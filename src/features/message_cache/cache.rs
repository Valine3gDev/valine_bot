use std::{collections::HashMap, sync::Arc};

use dashmap::DashMap;
use itertools::Itertools;
use serenity::{
    all::{ChannelId, Message, MessageId},
    prelude::TypeMapKey,
};

pub struct MessageCache {
    cache: DashMap<ChannelId, HashMap<MessageId, Message>>,
}

impl MessageCache {
    pub fn new() -> Self {
        Self { cache: DashMap::new() }
    }

    pub fn insert(&self, message: Message) {
        let mut map = self.cache.entry(message.channel_id).or_insert(HashMap::new());
        map.insert(message.id, message);
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
