use dashmap::{DashMap, mapref::entry::Entry};
use futures::StreamExt;
use serenity::{
    all::prelude::{CacheHttp, Context},
    builder::CreateAttachment,
    model::{
        channel::{Attachment, Message, MessageReference, MessageReferenceKind},
        id::{AttachmentId, GenericChannelId, MessageId, UserId},
    },
    small_fixed_array::FixedArray,
};
use tracing::{error, info};

use crate::{
    app::{AppError, BotDataExt},
    utils::create_safe_message,
};

pub(in crate::features::message_logging) struct MessageSnapshotStore {
    snapshot_ids: DashMap<GenericChannelId, DashMap<MessageId, MessageId>>,
}

impl MessageSnapshotStore {
    pub fn new() -> Self {
        Self {
            snapshot_ids: DashMap::default(),
        }
    }

    pub async fn delete(
        &self,
        ctx: &Context,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> Result<(), AppError> {
        let Some(entry) = self.snapshot_ids.get(&channel_id) else {
            return Ok(());
        };

        if let Some((_, snapshot_message_id)) = entry.remove(&message_id) {
            ctx.app_config()
                .await
                .message_logging
                .snapshot_channel_id
                .delete_message(ctx.http(), snapshot_message_id, None)
                .await?;
        }

        Ok(())
    }

    pub async fn update(&self, ctx: &Context, message: &Message) -> Result<(), AppError> {
        let config = &ctx.app_config().await.message_logging;
        let snapshot_channel_id = config.snapshot_channel_id;

        let snapshot_message = snapshot_channel_id
            .send_message(
                ctx.http(),
                create_safe_message().reference_message(
                    MessageReference::new(MessageReferenceKind::Forward, message.channel_id).message_id(message.id),
                ),
            )
            .await?;

        let old = {
            let entry = self.snapshot_ids.entry(message.channel_id).or_default();
            entry.insert(message.id, snapshot_message.id)
        };

        if let Some(old) = old {
            snapshot_channel_id.delete_message(ctx.http(), old, None).await?;
        }

        Ok(())
    }

    pub async fn sync(&self, ctx: &Context, message: &Message) -> Result<(), AppError> {
        if message.author.bot() {
            return Ok(());
        }

        if message.attachments.is_empty() {
            self.delete(ctx, message.channel_id, message.id).await
        } else {
            self.update(ctx, message).await
        }
    }

    pub async fn rebuild(&self, ctx: Context) {
        let snapshot_channel_id = ctx.app_config().await.message_logging.snapshot_channel_id;
        let bot_id = ctx.cache.current_user().id;
        let mut scanned_count = 0;
        let mut restored_count = 0;
        let mut messages = Box::pin(snapshot_channel_id.messages_iter(&ctx));

        while let Some(result) = messages.next().await {
            let message = match result {
                Ok(message) => message,
                Err(error) => {
                    error!("Failed to fetch snapshot messages: {error}");
                    break;
                }
            };
            scanned_count += 1;

            if self.restore_snapshot_id(&message, bot_id) {
                restored_count += 1;
            }
        }

        info!("Restored {restored_count} message attachment snapshots from {scanned_count} snapshot messages");
    }

    fn restore_snapshot_id(&self, snapshot_message: &Message, bot_id: UserId) -> bool {
        if snapshot_message.author.id != bot_id {
            return false;
        }

        let Some(message_reference) = snapshot_message.message_reference.as_ref() else {
            return false;
        };
        if message_reference.kind != MessageReferenceKind::Forward {
            return false;
        }
        let Some(message_id) = message_reference.message_id else {
            return false;
        };

        let entry = self.snapshot_ids.entry(message_reference.channel_id).or_default();
        match entry.entry(message_id) {
            Entry::Occupied(_) => false,
            Entry::Vacant(entry) => {
                entry.insert(snapshot_message.id);
                true
            }
        }
    }

    pub async fn attachments_for(&self, ctx: &Context, message: &Message) -> Result<FixedArray<Attachment>, AppError> {
        let attachments = self
            .get(ctx, message.channel_id, message.id)
            .await?
            .and_then(|snapshot| {
                snapshot
                    .message_snapshots
                    .first()
                    .map(|message| message.attachments.clone())
            })
            .unwrap_or_else(|| message.attachments.clone());

        Ok(attachments)
    }

    pub async fn upload_attachments(
        &self,
        ctx: &Context,
        message: &Message,
        keep_attachment_ids: &[AttachmentId],
    ) -> Result<Vec<CreateAttachment<'static>>, AppError> {
        let attachments = self.attachments_for(ctx, message).await?;

        Ok(futures::future::try_join_all(
            attachments
                .iter()
                .filter(|attachment| !keep_attachment_ids.contains(&attachment.id))
                .map(|attachment| {
                    CreateAttachment::url(ctx.http(), attachment.url.to_string(), attachment.filename.to_string())
                }),
        )
        .await?)
    }

    pub async fn get(
        &self,
        ctx: &Context,
        channel_id: GenericChannelId,
        message_id: MessageId,
    ) -> Result<Option<Message>, AppError> {
        let snapshot_message_id = {
            let entry = self.snapshot_ids.entry(channel_id).or_default();
            let Some(id) = entry.get(&message_id) else {
                return Ok(None);
            };
            *id
        };

        let snapshot_message = ctx
            .app_config()
            .await
            .message_logging
            .snapshot_channel_id
            .message(&ctx, snapshot_message_id)
            .await?;

        Ok(Some(snapshot_message))
    }
}
