use std::{collections::HashSet, sync::Arc};

use regex::Regex;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use serenity::{
    all::{ChannelId, Context, ForumTagId, GuildId, RoleId, UserId},
    prelude::TypeMapKey,
};

pub async fn get_config(ctx: &Context) -> Arc<Config> {
    let data = ctx.data.read().await;
    let config = data.get::<Config>().expect("Expected MessageCount in TypeMap.");
    config.clone()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub bot: BotConfig,
    pub auth: AuthConfig,
    pub message_logging: MessageLoggingConfig,
    pub message_cache: MessageCacheConfig,
    pub thread_channel_startup: ThreadChannelStartupConfig,
    pub thread_auto_invite: ThreadAutoInviteConfig,
    pub question: QuestionConfig,
}

impl TypeMapKey for Config {
    type Value = Arc<Config>;
}

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub token: String,
    pub owners: HashSet<UserId>,
    pub application_id: UserId,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub log_channel_id: ChannelId,
    pub role_id: RoleId,
    #[serde_as(as = "DisplayFromStr")]
    pub trigger_regex: Regex,
    pub dummy_keywords: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct MessageLoggingConfig {
    pub channel_id: ChannelId,
}

#[derive(Debug, Deserialize)]
pub struct MessageCacheConfig {
    pub disabled: bool,
    pub target_guild_ids: Vec<GuildId>,
}

#[derive(Debug, Deserialize)]
pub struct ThreadChannelStartupConfig {
    pub threads: Vec<ThreadStartupConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ThreadAutoInviteConfig {
    pub role_id: RoleId,
}

#[derive(Debug, Deserialize)]
pub struct ThreadStartupConfig {
    pub channel_id: ChannelId,
    pub startup_message: String,
}

#[derive(Debug, Deserialize)]
pub struct QuestionConfig {
    pub forum_id: ChannelId,
    pub exclude_tags: Vec<ForumTagId>,
    pub solved_tag: ForumTagId,
}
