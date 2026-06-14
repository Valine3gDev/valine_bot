use crate::app::BotData;

pub type AppError = Box<dyn std::error::Error + Send + Sync>;
pub type AppContext<'a> = poise::Context<'a, BotData, AppError>;
pub type AppApplicationContext<'a> = poise::ApplicationContext<'a, BotData, AppError>;
pub type AppCommand = poise::Command<BotData, AppError>;
