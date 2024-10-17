use std::{env, sync::LazyLock};

use serenity::{
    all::{
        ChannelId, Context, EventHandler, GuildId, Mentionable, Message, MessageId, MessageUpdateEvent
    },
    async_trait,
};
use similar::TextDiff;
use tracing::{error, info};

use crate::utils::{create_message, get_cached_message};

use super::message_cache::MessageCacheType;

static LOG_CHANNEL_ID: LazyLock<ChannelId> =
    LazyLock::new(|| ChannelId::new(env::var("MESSAGE_LOG_CHANNEL_ID").unwrap().parse().unwrap()));

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // async fn message_update(
    //     &self,
    //     _: Context,
    //     old: Option<Message>,
    //     new: Option<Message>,
    //     event: MessageUpdateEvent,
    // ) {
    //     if old.is_none() || new.is_none() {
    //         return error!("Failed to get old or new message: {:?}", event);
    //     }

    //     let (old, new) = match (old, new) {
    //         (Some(old), Some(new)) => (old, new),
    //         _ => return error!("Failed to get old or new message: {:?}", event),
    //     };
    // }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_id: MessageId,
        _: Option<GuildId>,
    ) {
        {
            let data = ctx.data.read().await;
            let cache = data.get::<MessageCacheType>().unwrap();
            let c = ChannelId::new(1098136613059567737);
            let messages = match cache.get_messages(c) {
                Some(m) => m.clone(),
                None => Vec::new(),
            };
            messages.iter().for_each(|m| {
                info!("Message: {:?}", m);
            });
        }

        let message = match get_cached_message(&ctx, channel_id, deleted_message_id).await {
            Some(m) => m.clone(),
            None => return error!("Failed to get message: {:?}", deleted_message_id),
        };

        let log = create_message(format!(
            "{} はメッセージを削除しました。\n```diff\n{}```",
            message.author.mention(),
            message
                .content
                .lines()
                .map(|line| format!("- {}", line))
                .collect::<Vec<String>>()
                .join("\n")
        ));

        if let Err(why) = LOG_CHANNEL_ID.send_message(&ctx.http, log).await {
            error!("Error sending message: {:?}", why)
        }
    }
}
