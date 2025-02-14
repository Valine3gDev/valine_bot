use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use chrono::Duration;
use duration_str::deserialize_duration_chrono;
use regex::Regex;
use serde::{Deserialize, Deserializer};
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
    pub auto_kick: AutoKickConfig,
    pub message_logging: MessageLoggingConfig,
    pub message_cache: MessageCacheConfig,
    pub pin: PinConfig,
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
pub struct AutoKickConfig {
    pub guild_id: GuildId,
    #[serde(deserialize_with = "deserialize_duration_chrono")]
    pub grace_period: Duration,
    pub kick_message: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageLoggingConfig {
    pub channel_id: ChannelId,
}

#[derive(Debug, Deserialize)]
pub struct MessageCacheConfig {
    pub disabled: bool,
    pub limit: usize,
    pub target_guild_ids: Vec<GuildId>,
    pub ignore_channel_ids: Vec<ChannelId>,
}

#[derive(Debug, Deserialize)]
pub struct PinConfig {
    #[serde(deserialize_with = "to_pin_channels")]
    pub channels: HashMap<ChannelId, UserId>,
}

fn to_pin_channels<'de, D>(deserializer: D) -> Result<HashMap<ChannelId, UserId>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Debug, Deserialize)]
    struct Temp {
        id: ChannelId,
        owner: UserId,
    }

    Ok(Vec::deserialize(deserializer)?
        .into_iter()
        .map(|i: Temp| (i.id, i.owner))
        .collect())
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

#[derive(Debug, Deserialize)]
pub struct ThreadAutoInviteConfig {
    pub role_id: RoleId,
}

#[derive(Debug, Deserialize)]
pub struct QuestionConfig {
    pub forum_id: ChannelId,
    pub exclude_tags: Vec<ForumTagId>,
    pub solved_tag: ForumTagId,
}
