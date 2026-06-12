mod config;
mod core;
mod data;
mod error;
mod extensions;
mod main_event_handler;
mod types;
// mod features;
mod utils;

use std::{fs::read_to_string, sync::Arc};

use bpaf::Bpaf;
use config::Config;
// use error::on_error;
// use features::{MessageCache, MessageCacheType, commands};
use poise::{Framework, FrameworkOptions};
use serenity::{cache::Settings as CacheSettings, prelude::*};
use tracing::error;

use crate::{
    core::{BotEventHandlers, create_client},
    data::BotData,
    error::on_error,
    main_event_handler::MainEventHandler,
};

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
struct Options {
    #[bpaf(short, long)]
    check_config: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = read_to_string("config.toml").expect("Failed to read config.toml");
    let config = toml::from_str::<Config>(&config).unwrap_or_else(|e| panic!("Failed to parse config.toml: {}", e));

    let options = options().run();

    if options.check_config {
        println!("Config is valid");
        return;
    }

    let data = Arc::new(BotData {
        config: Arc::new(config),
    });

    let framework = Framework::builder()
        .options(FrameworkOptions {
            // commands: commands(),
            on_error: |error| Box::pin(on_error(error)),
            skip_checks_for_owners: false,
            owners: data.config.bot.owners.clone(),
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
        data.config.bot.token.clone(),
        intents,
        BotEventHandlers::new().add(MainEventHandler::new()),
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
    .data(Arc::new(data))
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
}
