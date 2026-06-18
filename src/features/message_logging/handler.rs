use std::{borrow::Cow, iter};

use anyhow::Context as _;
use futures::future;
use itertools::Itertools;
use serenity::{
    all::{Context, Mentionable, Message, MessageBuilder, MessageId, Timestamp, prelude::CacheHttp},
    builder::{CreateAttachment, CreateContainerComponent, CreateSectionComponent, EditAttachments, EditMessage},
    model::{event::FullEvent, id::GenericChannelId},
};
use tracing::error;
use valine_bot_macros::event_handler;

use crate::{
    app::{
        AppError, BotDataExt, BotError,
        utils::components::{
            create_container, create_container_section, create_container_text, create_section_text,
            create_section_thumbnail, create_separator,
        },
    },
    extensions::MessageBuilderTimestampExt,
    features::message_logging::{
        component_builder::{
            bold_underline, build_diff_container_component, build_linked_removed_attachment_components,
            build_message_reference_container_component, build_poll_container_component,
            build_uploaded_removed_attachment_components,
        },
        log_type::MessageLogKind,
    },
    utils::{create_components_v2_message, create_safe_allowed_mentions, send_message},
};

fn build_log_container_components<'a>(
    message: &Message,
    log_kind: &MessageLogKind,
    basic_info_section_component: impl Into<Cow<'a, [CreateSectionComponent<'a>]>>,
    attachment_components: Vec<CreateContainerComponent<'a>>,
) -> Vec<CreateContainerComponent<'a>> {
    iter::once(create_container_text(format!("### **{}**", log_kind.title())))
        .chain(
            [
                Some(create_container_section(
                    basic_info_section_component.into(),
                    create_section_thumbnail(
                        message
                            .author
                            .avatar_url()
                            .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
                        Some("ユーザーアイコン"),
                        false,
                    ),
                )),
                build_message_reference_container_component(message),
                build_poll_container_component(message),
                build_diff_container_component(&message.content, log_kind.content_after()),
            ]
            .into_iter()
            .filter_map(|c| c.map(|c| [create_separator(false), c].into_iter()))
            .flatten(),
        )
        .chain(
            attachment_components
                .into_iter()
                .flat_map(|c| [create_separator(false), c]),
        )
        .collect_vec()
}

async fn send_message_log<'a>(ctx: &Context, message: &Message, log_kind: MessageLogKind<'a>) -> Result<(), AppError> {
    if message.author.bot() {
        return Ok(());
    }

    let attachment_ids_after = log_kind.attachment_ids_after();
    let message_basic_info = [create_section_text(
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
    )];

    let mut log_message = send_message(
        ctx,
        &ctx.app_config().await.message_logging.channel_id,
        create_components_v2_message(&[create_container(
            build_log_container_components(
                message,
                &log_kind,
                &message_basic_info,
                build_linked_removed_attachment_components(message, &attachment_ids_after, "ダウンロード中: \n"),
            ),
            Some(log_kind.color()),
            false,
        )]),
    )
    .await
    .context("Failed to send message log")?;

    if !message
        .attachments
        .iter()
        .any(|attachment| !attachment_ids_after.contains(&attachment.id))
    {
        return Ok(());
    }

    let edit_message = EditMessage::new().allowed_mentions(create_safe_allowed_mentions());

    let attachments = future::try_join_all(
        message
            .attachments
            .iter()
            .filter(|attachment| !attachment_ids_after.contains(&attachment.id))
            .map(|a| CreateAttachment::url(ctx.http(), a.url.to_string(), a.filename.to_string())),
    )
    .await;
    if let Ok(attachments) = attachments {
        let mut edit_attachments = EditAttachments::new();
        for attachment in attachments {
            edit_attachments = edit_attachments.add(attachment);
        }

        log_message
            .edit(
                &ctx,
                edit_message
                    .components(&[create_container(
                        build_log_container_components(
                            message,
                            &log_kind,
                            &message_basic_info,
                            build_uploaded_removed_attachment_components(message, &attachment_ids_after),
                        ),
                        Some(log_kind.color()),
                        false,
                    )])
                    .attachments(edit_attachments),
            )
            .await?;
    } else {
        log_message
            .edit(
                &ctx,
                edit_message.components(&[create_container(
                    build_log_container_components(
                        message,
                        &log_kind,
                        &message_basic_info,
                        build_linked_removed_attachment_components(message, &attachment_ids_after, ""),
                    ),
                    Some(log_kind.color()),
                    false,
                )]),
            )
            .await?;
    }

    Ok(())
}

async fn handle_message_update(
    ctx: &Context,
    old_if_available: &Option<Message>,
    new_message: &Message,
) -> Result<(), AppError> {
    let message = old_if_available.as_ref().ok_or_else(|| BotError::CacheMiss {
        resource: "message",
        id: new_message.id.to_string(),
    })?;

    send_message_log(
        ctx,
        message,
        MessageLogKind::Edit {
            content_after: &new_message.content,
            attachments_after: &new_message.attachments,
        },
    )
    .await
}

async fn handle_message_delete(
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

    send_message_log(ctx, &message, MessageLogKind::Delete).await
}

async fn handle_message_delete_bulk(
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
        send_message_log(ctx, &message, MessageLogKind::Delete).await?;
    }

    Ok(())
}

#[event_handler]
pub async fn handle_message_logging_event(ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
    match event {
        FullEvent::MessageUpdate {
            old_if_available,
            event,
            ..
        } => handle_message_update(ctx, old_if_available, &event.message).await?,

        FullEvent::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => handle_message_delete(ctx, channel_id, deleted_message_id).await?,

        FullEvent::MessageDeleteBulk {
            channel_id,
            multiple_deleted_messages_ids,
            ..
        } => handle_message_delete_bulk(ctx, channel_id, multiple_deleted_messages_ids).await?,

        _ => {}
    }

    Ok(())
}
