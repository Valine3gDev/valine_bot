use std::sync::atomic::{AtomicBool, Ordering};

use async_stream::stream;
use futures::{StreamExt, future};
use serenity::{
    all::{Context, Guild, GuildChannel, Member, prelude::CacheHttp},
    async_trait,
    model::{
        Permissions,
        channel::{GenericGuildChannelRef, GuildThread, Message},
        event::FullEvent,
        id::{GenericChannelId, MessageId},
    },
    small_fixed_array::FixedString,
};
use tracing::{error, info};

use crate::{
    app::{BotDataGetter, config::AppConfig},
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

    fn parent_id(&self) -> Option<GenericChannelId> {
        match self {
            Self::Channel(c) => c.parent_id,
            Self::Thread(t) => Some(t.parent_id),
        }
        .map(|id| id.widen())
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
        &self,
        ctx: &Context,
        config: &AppConfig,
        channel: ChannelWrapper,
        guild: &Guild,
        bot_member: &Member,
    ) {
        if !channel.is_text_based() {
            return;
        }

        let is_ignored = [Some(channel.id()), channel.parent_id()]
            .iter()
            .filter_map(|id| *id)
            .any(|id| config.message_cache.ignore_channel_ids.contains(&id.expect_channel()));
        if is_ignored {
            return;
        }

        let has_read_message_history_permission = channel
            .user_permission(guild, bot_member)
            .map(|p| p.read_message_history())
            .unwrap_or(false);
        if !has_read_message_history_permission {
            return;
        }

        let messages = channel
            .id()
            .messages_iter(&ctx.http())
            .take(config.message_cache.limit)
            .take_while(|x| future::ready(x.is_ok()))
            .filter_map(|x| future::ready(x.ok()))
            .collect::<Vec<_>>()
            .await;

        let cache = ctx.message_cache();
        let len = messages.len();
        cache.extend_messages(messages);
        info!(
            "Cached {len} messages for channel: {} ({})",
            channel.name(),
            channel.id()
        );
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        if self.disabled || self.collected.swap(true, Ordering::Relaxed) {
            return;
        }

        let config = ctx.read_app_config().await;

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
            let _ = stream! {
                for thread in active_threads {
                    yield ChannelWrapper::Thread(thread);
                }

                for channel in channels {
                    let id = channel.id;
                    yield ChannelWrapper::Channel(channel);

                    for await thread in fetch_all_archived_public_thread(ctx, id, None).await {
                        yield ChannelWrapper::Thread(thread);
                    }
                }
            }
            .map(|c| self.cache_channel_message(ctx, &config, c, &guild, &bot_member))
            .buffer_unordered(20)
            .collect::<Vec<_>>()
            .await;
        }
        info!("Cache ready!");
    }

    async fn handle_message_create(&self, ctx: &Context, new_message: &Message) {
        let cache = ctx.message_cache();
        cache.insert(new_message.clone());
    }

    async fn handle_message_update(&self, ctx: &Context, new_message: &Message) {
        let cache = ctx.message_cache();
        cache.insert(new_message.clone());
    }

    async fn handle_message_delete(&self, ctx: &Context, channel_id: &GenericChannelId, message_id: &MessageId) {
        let cache = ctx.message_cache();
        cache.remove(*channel_id, *message_id);
    }

    async fn handle_message_delete_bulk(
        &self,
        ctx: &Context,
        channel_id: &GenericChannelId,
        message_ids: &[MessageId],
    ) {
        let cache = ctx.message_cache();
        cache.remove_all(*channel_id, message_ids);
    }
}

#[async_trait]
impl BotEventHandler for MessageCacheHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        match event {
            FullEvent::CacheReady { .. } => self.handle_cache_ready(ctx).await,

            FullEvent::Message { new_message, .. } => self.handle_message_create(ctx, new_message).await,

            FullEvent::MessageUpdate { event, .. } => self.handle_message_update(ctx, &event.message).await,

            FullEvent::MessageDelete {
                channel_id,
                deleted_message_id,
                ..
            } => self.handle_message_delete(ctx, channel_id, deleted_message_id).await,

            FullEvent::MessageDeleteBulk {
                channel_id,
                multiple_deleted_messages_ids,
                ..
            } => {
                self.handle_message_delete_bulk(ctx, channel_id, multiple_deleted_messages_ids)
                    .await
            }

            _ => {}
        }
    }
}
