use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use serenity::{
    all::{Context, Message, MessageId},
    async_trait,
    model::{event::FullEvent, id::GenericChannelId},
};
use tracing::error;

use crate::{
    app::{AppError, BotError},
    core::BotEventHandler,
    features::message_logging::{
        log_sender::MessageLogSender, log_type::MessageLogKind, snapshot_store::MessageSnapshotStore,
    },
};

fn has_removed_logged_attachments(message: &Message, new_message: &Message) -> bool {
    let attachment_ids_after: Vec<_> = new_message.attachments.iter().map(|attachment| attachment.id).collect();

    message
        .attachments
        .iter()
        .any(|attachment| !attachment_ids_after.contains(&attachment.id))
}

/**
 * メッセージの編集可能な内容が変化したかどうか
 */
fn message_update_log_content_changed(message: &Message, new_message: &Message) -> bool {
    message.content != new_message.content || has_removed_logged_attachments(message, new_message)
}

pub struct MessageLoggingEventHandler {
    rebuilt_snapshot_store: AtomicBool,
    snapshot_store: Arc<MessageSnapshotStore>,
    log_sender: MessageLogSender,
}

impl MessageLoggingEventHandler {
    pub fn new() -> Self {
        let snapshot_store = Arc::new(MessageSnapshotStore::new());
        Self {
            rebuilt_snapshot_store: AtomicBool::new(false),
            log_sender: MessageLogSender::new(Arc::clone(&snapshot_store)),
            snapshot_store,
        }
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        if self.rebuilt_snapshot_store.swap(true, Ordering::Relaxed) {
            return;
        }

        let ctx = ctx.clone();
        let snapshot_store = self.snapshot_store.clone();
        tokio::spawn(async move {
            snapshot_store.rebuild(ctx).await;
        });
    }

    async fn handle_message_update(
        &self,
        ctx: &Context,
        old_if_available: &Option<Message>,
        new_message: &Message,
    ) -> Result<(), AppError> {
        let message = old_if_available.as_ref().ok_or_else(|| BotError::CacheMiss {
            resource: "message",
            id: new_message.id.to_string(),
        })?;

        if !message_update_log_content_changed(message, new_message) {
            return Ok(());
        }

        self.log_sender
            .send(
                ctx,
                message,
                MessageLogKind::Edit {
                    content_after: &new_message.content,
                    attachments_after: &new_message.attachments,
                },
            )
            .await?;

        self.snapshot_store.sync(ctx, new_message).await?;

        Ok(())
    }

    async fn handle_message_delete(
        &self,
        ctx: &Context,
        channel_id: &GenericChannelId,
        deleted_message_id: &MessageId,
    ) -> Result<(), AppError> {
        let message = ctx
            .cache
            .message(*channel_id, *deleted_message_id)
            .ok_or_else(|| BotError::CacheMiss {
                resource: "message",
                id: deleted_message_id.to_string(),
            })?
            .clone();

        self.log_sender.send(ctx, &message, MessageLogKind::Delete).await?;

        self.snapshot_store
            .delete(ctx, *channel_id, *deleted_message_id)
            .await?;

        Ok(())
    }

    async fn handle_message_delete_bulk(
        &self,
        ctx: &Context,
        channel_id: &GenericChannelId,
        deleted_message_ids: &[MessageId],
    ) -> Result<(), AppError> {
        for message_id in deleted_message_ids {
            let message = match ctx.cache.message(*channel_id, *message_id) {
                Some(message) => message.clone(),
                None => {
                    error!("Failed to get message: {message_id}");
                    continue;
                }
            };

            self.log_sender.send(ctx, &message, MessageLogKind::Delete).await?;
            self.snapshot_store.delete(ctx, *channel_id, *message_id).await?;
        }

        Ok(())
    }

    async fn handle_message_create(&self, ctx: &Context, new_message: &Message) -> Result<(), AppError> {
        self.snapshot_store.sync(ctx, new_message).await?;

        Ok(())
    }
}

#[async_trait]
impl BotEventHandler for MessageLoggingEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
        match event {
            FullEvent::CacheReady { .. } => self.handle_cache_ready(ctx).await,

            FullEvent::MessageUpdate {
                old_if_available: old,
                event,
                ..
            } => self.handle_message_update(ctx, old, &event.message).await?,

            FullEvent::MessageDelete {
                channel_id,
                deleted_message_id,
                ..
            } => self.handle_message_delete(ctx, channel_id, deleted_message_id).await?,

            FullEvent::MessageDeleteBulk {
                channel_id,
                multiple_deleted_messages_ids: messages_ids,
                ..
            } => self.handle_message_delete_bulk(ctx, channel_id, messages_ids).await?,

            FullEvent::Message { new_message, .. } => self.handle_message_create(ctx, new_message).await?,

            _ => {}
        }

        Ok(())
    }
}
