use std::collections::HashMap;

use dashmap::DashMap;
use itertools::Itertools;
use serenity::{
    all::{Message, MessageId},
    model::id::GenericChannelId,
};

pub struct MessageCache {
    cache: DashMap<GenericChannelId, HashMap<MessageId, Message>>,
}

impl MessageCache {
    pub fn new() -> Self {
        Self { cache: DashMap::new() }
    }

    pub fn insert(&self, message: Message) {
        let mut map = self.cache.entry(message.channel_id).or_default();
        map.insert(message.id, message);
    }

    pub fn remove(&self, channel_id: GenericChannelId, message_id: MessageId) {
        let mut map = self.cache.entry(channel_id).or_default();
        map.remove(&message_id);
    }

    pub fn remove_all(&self, channel_id: GenericChannelId, message_ids: &[MessageId]) {
        let mut map = self.cache.entry(channel_id).or_default();
        for id in message_ids {
            map.remove(id);
        }
    }

    pub fn extend(&self, iter: impl IntoIterator<Item = (GenericChannelId, MessageId, Message)>) {
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
                let mut map = self.cache.entry(channel_id).or_default();
                map.extend(messages);
            });
    }

    pub fn extend_messages(&self, iter: impl IntoIterator<Item = Message>) {
        self.extend(
            iter.into_iter()
                .map(|message| (message.channel_id, message.id, message)),
        );
    }

    pub fn get(&self, channel_id: GenericChannelId, message_id: MessageId) -> Option<Message> {
        let map = self.cache.get(&channel_id)?;
        map.get(&message_id).cloned()
    }
}

impl Default for MessageCache {
    fn default() -> Self {
        Self::new()
    }
}
