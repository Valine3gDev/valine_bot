use std::sync::Arc;

use poise::say_reply;
use tokio::fs::read_to_string;

use crate::{PContext, PError, config::Config};

/// コンフィグを再読み込み
#[poise::command(slash_command, ephemeral, owners_only, dm_only)]
pub async fn reload_config(ctx: PContext<'_>) -> Result<(), PError> {
    let Ok(config) = read_to_string("config.toml").await else {
        return Err("Failed to read config.toml".into());
    };

    let Ok(config) = toml::from_str::<Config>(&config) else {
        return Err("Failed to parse config.toml".into());
    };

    let mut data = ctx.serenity_context().data.write().await;
    data.insert::<Config>(Arc::new(config));

    say_reply(ctx, "Config reloaded").await?;

    Ok(())
}
