use crate::data::BotData;

pub type PError = Box<dyn std::error::Error + Send + Sync>;
pub type PContext<'a> = poise::Context<'a, BotData, PError>;
pub type PCommand = poise::Command<BotData, PError>;
