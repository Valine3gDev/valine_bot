mod config;
mod features;
mod utils;

use std::{fs::read_to_string, sync::Arc};

use config::Config;
use features::{MessageCache, MessageCacheType};
use serenity::{all::Ready, async_trait, cache::Settings as CacheSettings, prelude::*};
use tracing::{error, info};

struct MainHandler;

#[async_trait]
impl EventHandler for MainHandler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let config = read_to_string("config.toml").expect("Failed to read config.toml");
    let config = match toml::from_str::<Config>(&config) {
        Ok(config) => config,
        Err(e) => {
            panic!("Failed to parse config.toml: {}", e);
        }
    };

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILDS | GatewayIntents::MESSAGE_CONTENT;
    let mut settings = CacheSettings::default();
    settings.max_messages = 1_000_000;
    let mut client = Client::builder(&config.bot.token, intents)
        .event_handler(MainHandler)
        .event_handler(features::AuthHandler)
        .event_handler(features::LoggingHandler)
        .event_handler(features::MessageCacheHandler {
            disabled: config.message_cache.disabled,
        })
        .cache_settings(settings)
        .type_map_insert::<MessageCacheType>(Arc::new(MessageCache::new()))
        .type_map_insert::<Config>(Arc::new(config))
        .await
        .expect("Err creating client");

    let shard_manager = client.shard_manager.clone();
    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Could not register ctrl+c handler");
        shard_manager.shutdown_all().await;
    });

    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
