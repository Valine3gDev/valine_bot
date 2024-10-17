use std::{
    collections::HashMap,
    env,
    sync::{Arc, LazyLock},
};

use dashmap::DashMap;
use itertools::Itertools;
use serenity::{
    all::{ChannelId, Context, EventHandler, GetMessages, GuildId, Message, MessageId},
    async_trait,
    prelude::TypeMapKey,
};
use tracing::info;

static MESSAGE_CACHE_GUILD_ID: LazyLock<GuildId> =
    LazyLock::new(|| GuildId::new(env::var("MESSAGE_CACHE_GUILD_ID").unwrap().parse().unwrap()));

pub struct MessageCache {
    cache: DashMap<ChannelId, HashMap<MessageId, Message>>,
}

impl MessageCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }

    pub fn extend(&self, iter: impl IntoIterator<Item = (ChannelId, MessageId, Message)>) {
        iter.into_iter()
            .into_group_map_by(|(channel_id, _, _)| *channel_id)
            .into_iter()
            .map(|(channel_id, messages)| {
                (
                    channel_id,
                    messages
                        .into_iter()
                        .map(|(_, message_id, message)| (message_id, message))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .for_each(|(channel_id, messages)| {
                let mut map = self.cache.entry(channel_id).or_insert(HashMap::new());
                map.extend(messages);
            });
    }

    pub fn extend_messages(&self, iter: impl IntoIterator<Item = Message>) {
        self.extend(
            iter.into_iter()
                .map(|message| (message.channel_id, message.id, message)),
        );
    }

    pub fn get(&self, channel_id: ChannelId, message_id: MessageId) -> Option<Message> {
        let map = self.cache.get(&channel_id)?;
        map.get(&message_id).cloned()
    }

    pub fn get_messages(&self, channel_id: ChannelId) -> Option<Vec<Message>> {
        self.cache
            .get(&channel_id)
            .map(|map| map.values().cloned().collect())
    }
}

impl Default for MessageCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MessageCacheType;

impl TypeMapKey for MessageCacheType {
    type Value = Arc<MessageCache>;
}

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        let channels = MESSAGE_CACHE_GUILD_ID.channels(&ctx.http).await.unwrap();
        for (id, channel) in channels.iter() {
            if !channel.is_text_based() {
                continue;
            }

            let Ok(messages) = channel
                .messages(&ctx.http, GetMessages::new().limit(100))
                .await
            else {
                info!("Failed to get messages for channel: {:?}", channel.name);
                continue;
            };

            let mut data = ctx.data.write().await;
            let cache = data.get_mut::<MessageCacheType>().unwrap();
            let len = messages.len();
            cache.extend_messages(messages);
            info!(
                "Cached {} messages for channel: {} ({})",
                len, channel.name, id
            );
        }
        info!("Cache ready!");
    }
}