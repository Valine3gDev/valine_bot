use crate::{app::BotData, core::BoxError};

pub type AppError = BoxError;
pub type AppContext<'a> = poise::Context<'a, BotData, AppError>;
pub type AppApplicationContext<'a> = poise::ApplicationContext<'a, BotData, AppError>;
pub type AppCommand = poise::Command<BotData, AppError>;
