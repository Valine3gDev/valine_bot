use poise::say_reply;

use crate::app::{AppContext, AppError, BotDataGetter, config::AppConfig};

/// コンフィグを再読み込み
#[poise::command(slash_command, ephemeral, owners_only, dm_only)]
pub async fn reload_config(ctx: AppContext<'_>) -> Result<(), AppError> {
    let config = AppConfig::from_file("config.toml").await?;
    ctx.replace_app_config(config).await;
    say_reply(ctx, "Config reloaded").await?;
    Ok(())
}
