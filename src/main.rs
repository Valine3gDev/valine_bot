mod app;
mod core;
mod extensions;
mod features;
mod utils;

use std::sync::Arc;

use anyhow::Context as _;
use bpaf::Bpaf;
use poise::{Framework, FrameworkOptions, PrefixFrameworkOptions};
use serenity::{cache::Settings as CacheSettings, prelude::*};
use tracing::error;
use tracing_subscriber::EnvFilter;

use crate::{
    app::{AppError, BotData, MainEventHandler, config::AppConfig, handle_event_error, on_error},
    core::create_client,
    features::{commands, event_handlers},
};

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
struct Options {
    #[bpaf(short, long)]
    check_config: bool,
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    init_tracing();

    let config = AppConfig::from_file("config.toml").await?;

    let options = options().run();

    if options.check_config {
        println!("Config is valid");
        return Ok(());
    }

    let framework = Framework::builder()
        .options(FrameworkOptions {
            prefix_options: PrefixFrameworkOptions {
                prefix: None,
                mention_as_prefix: false,
                ..Default::default()
            },
            commands: commands(),
            on_error: |error| Box::pin(on_error(error)),
            skip_checks_for_owners: false,
            owners: config.bot.owners.clone(),
            ..Default::default()
        })
        .build();

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::MESSAGE_CONTENT;

    let mut settings = CacheSettings::default();
    settings.max_messages = usize::MAX;

    let mut client = create_client(
        config.bot.token.clone(),
        intents,
        event_handlers(&config)
            .add(MainEventHandler::new())
            .on_error(handle_event_error),
    )
    .framework(Box::new(framework))
    .cache_settings(settings)
    .data(Arc::new(BotData::new(config)))
    .await
    .context("Failed to create Discord client")?;

    let shutdown = client.shard_manager.get_shutdown_trigger();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shutdown()
    });

    if let Err(error) = client.start().await.context("Discord client stopped with an error") {
        error!("Client error: {error:#}");
    }

    Ok(())
}
