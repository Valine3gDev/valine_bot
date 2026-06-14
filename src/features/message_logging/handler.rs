use serenity::{
    all::{Context, CreateEmbed, Mentionable, Message, MessageBuilder, MessageId, Timestamp},
    model::{event::FullEvent, id::GenericChannelId},
};
use tracing::error;
use valine_bot_macros::event_handler;

use crate::{
    app::{AppError, BotDataExt},
    extensions::MessageBuilderTimestampExt,
    features::message_logging::{embed_builder::build_embed, log_type::LogType},
    utils::{create_safe_message, send_message},
};

async fn create_and_send_log(ctx: &Context, message: &Message, log_type: LogType) {
    if message.author.bot() {
        return;
    }

    let description = MessageBuilder::new()
        .push_bold_safe("メンバー: ")
        .mention(&message.author.mention())
        .push_safe(" ")
        .push_mono_line_safe(&*message.author.id.to_string())
        .push_bold_safe("メッセージ: ")
        .push_safe(&*message.id.link(message.channel_id, message.guild_id).to_string())
        .push_safe(" ")
        .push_mono_line_safe(&*message.id.to_string())
        .push_bold_safe("メッセージ送信日時: ")
        .push_short_date_medium_timestamp_line(message.timestamp)
        .push_bold_safe(format!("{}日時: ", log_type.name()).as_str())
        .push_short_date_medium_timestamp(Timestamp::now())
        .build();

    let mut embed = CreateEmbed::new()
        .title(log_type.title())
        .description(description)
        .color(log_type.color())
        .thumbnail(
            message
                .author
                .avatar_url()
                .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            Some("ユーザーアイコン".into()),
        );

    let new_content = log_type.new_content().unwrap_or("");
    embed = build_embed(message, new_content, embed);
    send_log(ctx, embed).await;
}

async fn send_log<'a>(ctx: &Context, embed: CreateEmbed<'a>) {
    let config = ctx.app_config().await;
    let log = create_safe_message().add_embed(embed);
    let _ = send_message(ctx, &config.message_logging.channel_id, log).await;
}

async fn handle_message_update(ctx: &Context, old_if_available: &Option<Message>, new_message: &Message) {
    let Some(message) = old_if_available else {
        return error!("Failed to get message: {}", new_message.id);
    };

    if message.content == new_message.content {
        return;
    }

    create_and_send_log(
        ctx,
        message,
        LogType::Edit {
            new_content: new_message.content.to_string(),
        },
    )
    .await;
}

async fn handle_message_delete(ctx: &Context, channel_id: &GenericChannelId, deleted_message_id: &MessageId) {
    let message = match ctx.cache.message(*channel_id, *deleted_message_id) {
        Some(message) => message.clone(),
        None => return error!("Failed to get message: {deleted_message_id}"),
    };

    create_and_send_log(ctx, &message, LogType::Delete).await;
}

async fn handle_message_delete_bulk(ctx: &Context, channel_id: &GenericChannelId, deleted_message_ids: &[MessageId]) {
    for message_id in deleted_message_ids {
        let message = match ctx.cache.message(*channel_id, *message_id) {
            Some(message) => message.clone(),
            None => {
                error!("Failed to get message: {message_id}");
                continue;
            }
        };
        create_and_send_log(ctx, &message, LogType::Delete).await;
    }
}

#[event_handler]
pub async fn handle_message_logging_event(ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
    match event {
        FullEvent::MessageUpdate {
            old_if_available,
            event,
            ..
        } => handle_message_update(ctx, old_if_available, &event.message).await,

        FullEvent::MessageDelete {
            channel_id,
            deleted_message_id,
            ..
        } => handle_message_delete(ctx, channel_id, deleted_message_id).await,

        FullEvent::MessageDeleteBulk {
            channel_id,
            multiple_deleted_messages_ids,
            ..
        } => handle_message_delete_bulk(ctx, channel_id, multiple_deleted_messages_ids).await,

        _ => {}
    }

    Ok(())
}
