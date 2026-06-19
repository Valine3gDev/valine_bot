use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use async_stream::stream;
use futures::{StreamExt, future};
use serenity::{
    all::{Context, Guild, GuildChannel, Member, prelude::CacheHttp},
    async_trait,
    model::{
        Permissions,
        channel::{GenericGuildChannelRef, GuildThread},
        event::FullEvent,
        id::GenericChannelId,
    },
    small_fixed_array::FixedString,
};
use tokio::time::sleep;
use tracing::{error, info};

use crate::{
    app::{AppError, BotDataExt},
    core::BotEventHandler,
    utils::fetch_all_archived_public_thread,
};

enum ChannelWrapper {
    Channel(GuildChannel),
    Thread(GuildThread),
}

impl ChannelWrapper {
    fn is_text_based(&self) -> bool {
        match self {
            Self::Channel(c) => c.is_text_based(),
            Self::Thread(_) => true,
        }
    }

    fn id(&self) -> GenericChannelId {
        match self {
            Self::Channel(c) => c.id.widen(),
            Self::Thread(t) => t.id.widen(),
        }
    }

    fn name(&self) -> FixedString<u16> {
        (match self {
            Self::Channel(c) => &c.base,
            Self::Thread(t) => &t.base,
        })
        .clone()
        .name
    }

    fn user_permission(&self, guild: &Guild, member: &Member) -> Option<Permissions> {
        match self {
            Self::Channel(c) => Some(guild.user_permissions_in(c, member)),
            Self::Thread(t) => guild.channel(t.parent_id.into()).and_then(|c| match c {
                GenericGuildChannelRef::Channel(gc) => Some(guild.user_permissions_in(gc, member)),
                _ => None,
            }),
        }
    }
}

pub struct MessageCacheHandler {
    disabled: bool,
    collected: AtomicBool,
}

impl MessageCacheHandler {
    pub fn new(disabled: bool) -> Self {
        Self {
            disabled,
            collected: AtomicBool::new(false),
        }
    }

    async fn cache_channel_message(
        ctx: &Context,
        channel: ChannelWrapper,
        guild: &Guild,
        bot_member: &Member,
        request_window: Duration,
        requests_per_window: usize,
    ) {
        if !channel.is_text_based() {
            return;
        }

        let has_read_message_history_permission = channel
            .user_permission(guild, bot_member)
            .map(|p| p.read_message_history())
            .unwrap_or(false);
        if !has_read_message_history_permission {
            return;
        }

        let messages_per_window = requests_per_window.saturating_mul(100);

        // Context を渡すと Serenity 標準キャッシュに取得結果が載るようになる
        let collected_count = channel
            .id()
            .messages_iter(ctx)
            .enumerate()
            .map(|(index, message)| async move {
                if (index + 1) % messages_per_window == 0 {
                    sleep(request_window).await;
                }
                message
            })
            .buffered(1)
            .take_while(|x| future::ready(x.is_ok()))
            .filter_map(|x| future::ready(x.ok()))
            .count()
            .await;

        info!(
            "Cached {collected_count} messages for channel: {} ({})",
            channel.name(),
            channel.id()
        );
    }

    async fn collect_cache(ctx: Context) {
        let config = ctx.app_config().await;
        let ignore_channel_ids = [config.message_logging.snapshot_channel_id];
        let request_window = config.message_cache.request_window;
        let requests_per_window: usize = config.message_cache.requests_per_window.max(1).into();
        let concurrent_channels: usize = config.message_cache.concurrent_channels.max(1).into();

        for guild_id in &config.message_cache.target_guild_ids {
            let guild = match guild_id.to_guild_cached(&ctx.cache) {
                Some(guild) => guild.clone(),
                None => {
                    error!("Failed to get guild: {}", guild_id);
                    continue;
                }
            };

            let bot_id = ctx.cache.current_user().id;

            let Ok(bot_member) = guild.member(ctx.http(), bot_id).await else {
                error!("Failed to get bot member for guild: {}", guild_id);
                continue;
            };

            let Ok(channels) = guild_id.channels(ctx.http()).await else {
                error!("Failed to get channels for guild: {}", guild_id);
                continue;
            };

            let active_threads = guild.threads.clone();
            let ctx = &ctx;
            let _ = stream! {
                for thread in active_threads {
                    yield ChannelWrapper::Thread(thread);
                }

                for channel in channels {
                    let id = channel.id;

                    if ignore_channel_ids.contains(&id.widen()) {
                        continue;
                    }

                    yield ChannelWrapper::Channel(channel);

                    for await thread in fetch_all_archived_public_thread(ctx, id, None).await {
                        yield ChannelWrapper::Thread(thread);
                    }
                }
            }
            .map(|c| Self::cache_channel_message(ctx, c, &guild, &bot_member, request_window, requests_per_window))
            .buffer_unordered(concurrent_channels)
            .collect::<Vec<_>>()
            .await;
        }
        info!("Cache ready!");
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        if self.disabled || self.collected.swap(true, Ordering::Relaxed) {
            return;
        }

        tokio::spawn(Self::collect_cache(ctx.clone()));
    }
}

#[async_trait]
impl BotEventHandler for MessageCacheHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
        if let FullEvent::CacheReady { .. } = event {
            self.handle_cache_ready(ctx).await
        }

        Ok(())
    }
}
