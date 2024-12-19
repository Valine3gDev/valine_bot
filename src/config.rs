use std::sync::Arc;

use regex::Regex;
use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};
use serenity::{
    all::{ChannelId, Context, GuildId, RoleId, UserId},
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
}

impl TypeMapKey for Config {
    type Value = Arc<Config>;
}

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub token: String,
    pub application_id: UserId,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub log_channel_id: ChannelId,
    pub role_id: RoleId,
    #[serde_as(as = "DisplayFromStr")]
    pub trigger_regex: Regex,
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
pub struct ThreadStartupConfig {
    pub channel_id: ChannelId,
    pub startup_message: String,
}
