use serenity::{
    all::prelude::Context,
    model::{
        channel::Message,
        id::{GenericChannelId, MessageId},
    },
};

use crate::app::BotDataGetter;

pub async fn get_cached_message(ctx: &Context, channel_id: GenericChannelId, message_id: MessageId) -> Option<Message> {
    if let Some(m) = ctx.cache.message(channel_id, message_id) {
        return Some(m.clone());
    }

    let cache = ctx.message_cache();
    if let Some(m) = cache.get(channel_id, message_id) {
        return Some(m);
    }

    None
}
