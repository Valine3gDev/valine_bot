use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::Duration as StdDuration,
};

use anyhow::Context as _;
use chrono::Duration;
use duration_str::{deserialize_duration, deserialize_duration_chrono};
use regex::Regex;
use serde::{Deserialize, Deserializer};
use serde_with::{DisplayFromStr, serde_as};
use serenity::{
    all::{ChannelId, ForumTagId, GuildId, RoleId, Token, UserId},
    model::id::GenericChannelId,
};
use tokio::fs::read_to_string;

use crate::app::AppError;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub bot: BotConfig,
    pub auth: AuthConfig,
    pub auto_kick: AutoKickConfig,
    pub honeypot: HoneypotConfig,
    pub message_logging: MessageLoggingConfig,
    pub message_cache: MessageCacheConfig,
    pub pin: PinConfig,
    pub thread_auto_invite: ThreadAutoInviteConfig,
    pub question: QuestionConfig,
}

impl AppConfig {
    pub async fn from_file(path: &str) -> Result<Self, AppError> {
        let text = read_to_string(path)
            .await
            .with_context(|| format!("Failed to read config file: {path}"))?;
        toml::from_str(&text).with_context(|| format!("Failed to parse config file: {path}"))
    }
}

#[derive(Debug, Deserialize)]
pub struct BotConfig {
    pub token: Token,
    pub owners: HashSet<UserId>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct AuthConfig {
    pub log_channel_id: ChannelId,
    pub role_id: RoleId,
    pub keyword: String,
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
pub struct HoneypotConfig {
    pub channel_id: ChannelId,
    #[serde(deserialize_with = "deserialize_duration_chrono")]
    pub message_lookback: Duration,
    pub kick_message: String,
    pub log_channel_id: ChannelId,
}

#[derive(Debug, Deserialize)]
pub struct MessageLoggingConfig {
    pub channel_id: ChannelId,
    pub snapshot_channel_id: GenericChannelId,
}

#[derive(Debug, Deserialize)]
pub struct MessageCacheConfig {
    pub disabled: bool,
    pub target_guild_ids: Vec<GuildId>,
    #[serde(deserialize_with = "deserialize_duration")]
    pub request_window: StdDuration,
    pub requests_per_window: u8,
    pub concurrent_channels: u8,
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
pub struct ThreadAutoInviteConfig {
    pub display_role_id: RoleId,
    pub role_ids: Vec<RoleId>,
    pub min_member_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct QuestionConfig {
    pub forum_id: ChannelId,
    pub solved_tag: ForumTagId,
    pub solved_name_prefix: String,
}
