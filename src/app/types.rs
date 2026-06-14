use crate::{app::BotData, core::AnyError};

pub type AppError = AnyError;
pub type AppContext<'a> = poise::Context<'a, BotData, AppError>;
pub type AppApplicationContext<'a> = poise::ApplicationContext<'a, BotData, AppError>;
pub type AppCommand = poise::Command<BotData, AppError>;
