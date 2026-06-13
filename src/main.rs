mod app;
mod core;
mod extensions;
mod features;
mod utils;

use std::sync::Arc;

use bpaf::Bpaf;
use poise::{Framework, FrameworkOptions};
use serenity::{cache::Settings as CacheSettings, prelude::*};
use tracing::error;

use crate::{
    app::{AppError, BotData, MainEventHandler, config::AppConfig, on_error},
    core::{BotEventHandlers, create_client},
    features::{AutoKickEventHandler, commands, handle_honeypot_event},
};

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
struct Options {
    #[bpaf(short, long)]
    check_config: bool,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_file("config.toml").await?;

    let options = options().run();

    if options.check_config {
        println!("Config is valid");
        return Ok(());
    }

    let framework = Framework::builder()
        .options(FrameworkOptions {
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
    settings.max_messages = 1_000_000;

    let mut client = create_client(
        config.bot.token.clone(),
        intents,
        BotEventHandlers::new()
            .add(MainEventHandler::new())
            .add(AutoKickEventHandler::new())
            .add(handle_honeypot_event),
    )
    .framework(Box::new(framework))
    // .event_handler(features::AuthHandler::new())
    // .event_handler(features::AutoKickHandler::new())
    // .event_handler(features::HoneypotHandler)
    // .event_handler(features::LoggingHandler)
    // .event_handler(features::ThreadAutoInviteHandler::new())
    // .event_handler(features::ThreadChannelStartupHandler)
    // .event_handler(features::QuestionHandler)
    // .event_handler(features::MessageCacheHandler::new(config.message_cache.disabled))
    .cache_settings(settings)
    .data(Arc::new(BotData::new(config)))
    // .type_map_insert::<MessageCacheType>(Arc::new(MessageCache::new()))
    .await
    .expect("Err creating client");

    let shutdown = client.shard_manager.get_shutdown_trigger();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shutdown()
    });

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    Ok(())
}
