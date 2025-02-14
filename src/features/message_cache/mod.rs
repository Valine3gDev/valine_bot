mod cache;

use std::sync::atomic::{AtomicBool, Ordering};

pub use cache::{MessageCache, MessageCacheType};

use async_stream::stream;
use futures::StreamExt;
use serenity::{
    all::{Context, EventHandler, Guild, GuildChannel, GuildId, Member},
    async_trait,
};
use tracing::{error, info};

use crate::{
    config::{get_config, Config},
    utils::fetch_all_archived_public_thread,
};

pub struct Handler {
    disabled: bool,
    collected: AtomicBool,
}

impl Handler {
    pub fn new(disabled: bool) -> Self {
        Self {
            disabled,
            collected: AtomicBool::new(false),
        }
    }

    async fn cache_channel_message(
        &self,
        ctx: &Context,
        config: &Config,
        channel: GuildChannel,
        guild: &Guild,
        bot_member: &Member,
    ) {
        if !channel.is_text_based() {
            return;
        }

        let is_ignored = std::iter::once(channel.id)
            .chain(channel.parent_id.into_iter())
            .any(|id| config.message_cache.ignore_channel_ids.contains(&id));
        if is_ignored {
            return;
        }

        if !guild.user_permissions_in(&channel, bot_member).read_message_history() {
            return;
        }

        let messages = channel
            .id
            .messages_iter(&ctx)
            .take(config.message_cache.limit)
            .filter_map(|x| async { x.ok() })
            .collect::<Vec<_>>()
            .await;

        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MessageCacheType>().unwrap();
        let len = messages.len();
        cache.extend_messages(messages);
        info!("Cached {} messages for channel: {} ({})", len, channel.name, channel.id);
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        if self.disabled || self.collected.swap(true, Ordering::Relaxed) {
            return;
        }

        let ctx_ref = &ctx;
        let config = get_config(ctx_ref).await;

        for guild_id in &config.message_cache.target_guild_ids {
            let guild = match guild_id.to_guild_cached(ctx_ref) {
                Some(guild) => guild.clone(),
                None => {
                    error!("Failed to get guild: {:?}", guild_id);
                    continue;
                }
            };

            let bot_id = ctx_ref.cache.current_user().id;
            let Ok(bot_member) = guild.member(ctx_ref, bot_id).await else {
                error!("Failed to get bot member for guild: {:?}", guild_id);
                continue;
            };

            let Ok(channels) = guild_id.channels(ctx_ref).await else {
                error!("Failed to get channels for guild: {:?}", guild_id);
                continue;
            };

            let active_threads = guild.threads.clone();
            let _ = stream! {
                for thread in active_threads {
                    yield thread;
                }

                for (id, channel) in channels {
                    yield channel;

                    for await thread in fetch_all_archived_public_thread(ctx_ref, id, None).await {
                        yield thread;
                    }
                }
            }
            .then(|c| self.cache_channel_message(ctx_ref, &config, c, &guild, &bot_member))
            .collect::<Vec<_>>()
            .await;
        }
        info!("Cache ready!");
    }
}
