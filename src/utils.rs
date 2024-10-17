use serenity::{all::{ChannelId, Context, CreateAllowedMentions, CreateMessage, Message, MessageId}, Result};

use crate::MessageCacheType;

pub fn create_message(content: String) -> CreateMessage {
    CreateMessage::new()
        .content(content)
        .allowed_mentions(CreateAllowedMentions::new().all_users(false))
}

pub async fn get_cached_message(ctx: &Context, channel_id: ChannelId, message_id: MessageId) -> Option<Message> {
    if let Some(m) = ctx.cache.message(channel_id, message_id) {
        return Some(m.clone());
    }

    let data = ctx.data.read().await;
    let cache = data.get::<MessageCacheType>().unwrap();
    if let Some(m) = cache.get(channel_id, message_id) {
        return Some(m);
    }

    None
}

pub async fn get_message(ctx: &Context, channel_id: ChannelId, message_id: MessageId) -> Result<Message> {
    if let Some(cached) = get_cached_message(ctx, channel_id, message_id).await {
        return Ok(cached);
    }

    channel_id.message(&ctx.http, message_id).await
}
