use itertools::Itertools;
use serenity::{
    all::{ChannelId, Context, CreateAllowedMentions, CreateMessage, Message, MessageId},
    Result,
};
use similar::{Algorithm, ChangeTag, TextDiff};

use crate::MessageCacheType;

pub fn create_safe_message() -> CreateMessage {
    CreateMessage::new().allowed_mentions(CreateAllowedMentions::new().all_users(false))
}

pub fn create_message(content: String) -> CreateMessage {
    create_safe_message().content(content)
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

pub fn create_diff_lines_text(old: &str, new: &str) -> String {
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_lines(old, new);
    diff.iter_all_changes()
        .map(|c| match c.tag() {
            ChangeTag::Delete => format!("- {}", c),
            ChangeTag::Insert => format!("+ {}", c),
            ChangeTag::Equal => c.to_string(),
        })
        .join("")
}
