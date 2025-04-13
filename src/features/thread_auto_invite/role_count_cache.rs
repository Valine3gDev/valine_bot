use std::sync::Arc;

use dashmap::DashMap;
use serenity::{
    all::{Context, GuildId, Member, RoleId},
    prelude::TypeMapKey,
};
use tracing::info;

use crate::config::ThreadAutoInviteConfig;

use super::MemberCache;

#[derive(Debug, Clone)]
pub struct RoleCountCache {
    cache: DashMap<RoleId, usize>,
}

/**
 * 今のところ、スレッド招待用ロールのカウントしか更新されない
 */
impl RoleCountCache {
    pub fn new() -> Self {
        Self { cache: DashMap::new() }
    }

    pub fn init(&self, members: &[Member]) {
        for member in members {
            for role in &member.roles {
                info!("{} has role {}", member.user.name, role);
                self.increment(*role);
            }
        }
    }

    pub fn increment(&self, role_id: RoleId) {
        self.cache.entry(role_id).and_modify(|count| *count += 1).or_insert(1);
    }

    pub fn decrement(&self, role_id: RoleId) {
        self.cache.entry(role_id).and_modify(|count| *count -= 1).or_insert(0);
    }

    pub fn get(&self, role_id: RoleId) -> Option<usize> {
        self.cache.get(&role_id).map(|count| *count)
    }

    pub async fn is_empty(ctx: &Context) -> bool {
        let data = ctx.data.read().await;
        let cache = data.get::<RoleCountCacheType>().unwrap();
        cache.cache.is_empty()
    }

    pub async fn increment_count(ctx: &Context, role_id: RoleId) {
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<RoleCountCacheType>().unwrap();
        cache.increment(role_id);
    }
}

pub struct RoleCountCacheType;

impl TypeMapKey for RoleCountCacheType {
    type Value = Arc<RoleCountCache>;
}

pub async fn find_role(ctx: &Context, guild_id: GuildId, config: &ThreadAutoInviteConfig) -> Option<RoleId> {
    if RoleCountCache::is_empty(ctx).await {
        let members = MemberCache::get_all_members(ctx, guild_id).await;
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<RoleCountCacheType>().unwrap();
        cache.init(&members);
    }

    let data = ctx.data.read().await;
    let cache = data.get::<RoleCountCacheType>().unwrap();
    config
        .role_ids
        .iter()
        .find(|r| cache.get(**r).unwrap_or(0) < config.min_member_count)
        .cloned()
}
