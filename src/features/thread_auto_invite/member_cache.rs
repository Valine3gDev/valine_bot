use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use dashmap::DashMap;
use futures::StreamExt;
use itertools::Itertools;
use serenity::{
    all::{Context, EventHandler, GuildId, Member, UserId},
    async_trait,
    prelude::TypeMapKey,
};
use tracing::{error, info};

pub struct Handler {
    collected: AtomicBool,
}

impl Handler {
    pub fn new() -> Self {
        Self {
            collected: AtomicBool::new(false),
        }
    }
}

impl Default for Handler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, guild_ids: Vec<GuildId>) {
        if self.collected.swap(true, Ordering::Relaxed) {
            return;
        }

        let mut total_members = 0;

        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MemberCacheType>().unwrap();

        for guild_id in guild_ids {
            let members = guild_id
                .members_iter(&ctx)
                .filter_map(async move |m| m.map_err(|e| error!("Error fetching member: {:?}", e)).ok())
                .collect::<Vec<_>>()
                .await;
            total_members += members.len();
            cache.extend_members(members);
        }
        info!("Member cache is ready. Total members cached: {}", total_members);
    }
}

pub struct MemberCache {
    cache: DashMap<GuildId, HashMap<UserId, Member>>,
}

impl MemberCache {
    pub fn new() -> Self {
        Self { cache: DashMap::new() }
    }

    pub fn insert(&self, member: &Member) {
        self.cache
            .entry(member.guild_id)
            .or_default()
            .insert(member.user.id, member.clone());
    }

    pub fn extend(&self, iter: impl IntoIterator<Item = (GuildId, UserId, Member)>) {
        iter.into_iter()
            .into_group_map_by(|(guild_id, _, _)| *guild_id)
            .into_iter()
            .map(|(guild_id, members)| {
                (
                    guild_id,
                    members
                        .into_iter()
                        .map(|(_, user_id, member)| (user_id, member))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .for_each(|(guild_id, members)| {
                let mut map = self.cache.entry(guild_id).or_default();
                map.extend(members);
            });
    }

    pub fn extend_members(&self, iter: impl IntoIterator<Item = Member>) {
        self.extend(iter.into_iter().map(|member| (member.guild_id, member.user.id, member)));
    }

    pub fn remove(&self, guild_id: GuildId, user_id: UserId) {
        if let Some(mut map) = self.cache.get_mut(&guild_id) {
            map.remove(&user_id);
        }
    }

    pub fn get(&self, guild_id: GuildId, user_id: UserId) -> Option<Member> {
        self.cache.get(&guild_id)?.get(&user_id).cloned()
    }

    pub fn get_all(&self, guild_id: GuildId) -> Vec<Member> {
        self.cache.entry(guild_id).or_default().values().cloned().collect()
    }

    pub async fn get_member(ctx: &Context, guild_id: GuildId, user_id: UserId) -> Option<Member> {
        let data = ctx.data.read().await;
        let cache = data.get::<MemberCacheType>().unwrap();
        cache.get(guild_id, user_id)
    }

    pub async fn get_all_members(ctx: &Context, guild_id: GuildId) -> Vec<Member> {
        let data = ctx.data.read().await;
        let cache = data.get::<MemberCacheType>().unwrap();
        cache.get_all(guild_id)
    }

    pub async fn insert_member(ctx: &Context, member: &Member) {
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MemberCacheType>().unwrap();
        cache.insert(member);
    }

    pub async fn remove_member(ctx: &Context, guild_id: GuildId, user_id: UserId) {
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MemberCacheType>().unwrap();
        cache.remove(guild_id, user_id);
    }
}

impl Default for MemberCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MemberCacheType;

impl TypeMapKey for MemberCacheType {
    type Value = Arc<MemberCache>;
}
