use std::sync::Arc;

use anyhow::Context as _;
use serenity::{
    all::{Context, Mentionable, Message, MessageBuilder, Timestamp},
    builder::{CreateContainerComponent, CreateSectionComponent, EditAttachments, EditMessage},
};

use crate::{
    app::{
        AppError, BotDataExt,
        utils::components::{create_container, create_section_text},
    },
    extensions::MessageBuilderTimestampExt,
    features::message_logging::{
        component_builder::{
            bold_underline, build_linked_removed_attachment_components, build_log_container_components,
            build_uploaded_removed_attachment_components,
        },
        log_type::MessageLogKind,
        snapshot_store::MessageSnapshotStore,
    },
    utils::{create_components_v2_message, create_safe_allowed_mentions, send_message},
};

pub struct MessageLogSender {
    snapshot_store: Arc<MessageSnapshotStore>,
}

impl MessageLogSender {
    pub fn new(snapshot_store: Arc<MessageSnapshotStore>) -> Self {
        Self { snapshot_store }
    }

    pub async fn send<'a>(
        &self,
        ctx: &Context,
        message: &Message,
        log_kind: MessageLogKind<'a>,
    ) -> Result<(), AppError> {
        if message.author.bot() {
            return Ok(());
        }

        let attachment_ids_after = log_kind.attachment_ids_after();
        let message_basic_info = build_message_basic_info(message, &log_kind);
        let log_container_components = build_log_container_components(
            message,
            &log_kind,
            &message_basic_info,
            build_linked_removed_attachment_components(message, &attachment_ids_after),
        );

        let mut log_message = self
            .send_initial_log_message(ctx, &log_kind, log_container_components)
            .await?;

        self.upload_removed_attachments(ctx, &mut log_message, message, &log_kind, &message_basic_info)
            .await?;

        Ok(())
    }

    async fn send_initial_log_message<'a>(
        &self,
        ctx: &Context,
        log_kind: &MessageLogKind<'a>,
        log_container_components: Vec<CreateContainerComponent<'a>>,
    ) -> Result<Message, AppError> {
        send_message(
            ctx,
            &ctx.app_config().await.message_logging.channel_id,
            create_components_v2_message(vec![create_container(
                log_container_components,
                Some(log_kind.color()),
                false,
            )]),
        )
        .await
        .context("Failed to send message log")
    }

    async fn upload_removed_attachments<'a>(
        &self,
        ctx: &Context,
        log_message: &mut Message,
        message: &'a Message,
        log_kind: &MessageLogKind<'a>,
        message_basic_info: &'a [CreateSectionComponent<'a>],
    ) -> Result<(), AppError> {
        if message.attachments.is_empty() {
            return Ok(());
        }

        let attachment_ids_after = log_kind.attachment_ids_after();
        let attachments = self
            .snapshot_store
            .upload_attachments(ctx, message, &attachment_ids_after)
            .await?;

        let mut edit_attachments = EditAttachments::new();
        for attachment in attachments {
            edit_attachments = edit_attachments.add(attachment);
        }

        log_message
            .edit(
                &ctx,
                EditMessage::new()
                    .allowed_mentions(create_safe_allowed_mentions())
                    .components(vec![create_container(
                        build_log_container_components(
                            message,
                            log_kind,
                            message_basic_info,
                            build_uploaded_removed_attachment_components(message, &attachment_ids_after),
                        ),
                        Some(log_kind.color()),
                        false,
                    )])
                    .attachments(edit_attachments),
            )
            .await?;

        Ok(())
    }
}

fn build_message_basic_info<'a>(message: &Message, log_kind: &MessageLogKind) -> [CreateSectionComponent<'a>; 1] {
    [create_section_text(
        MessageBuilder::new()
            .push("### ")
            .push_line(bold_underline("基本情報"))
            .push_bold_safe("送信者: ")
            .mention(&message.author.mention())
            .push_safe(" ")
            .push_mono_line_safe(&*message.author.id.to_string())
            .push_bold_safe("リンク: ")
            .push_safe(&*message.id.link(message.channel_id, message.guild_id).to_string())
            .push_safe(" ")
            .push_mono_line_safe(&*message.id.to_string())
            .push_bold_safe("送信日時: ")
            .push_short_date_medium_timestamp_line(message.timestamp)
            .push_bold_safe(format!("{}日時: ", log_kind.name()).as_str())
            .push_short_date_medium_timestamp(Timestamp::now())
            .build(),
    )]
}
