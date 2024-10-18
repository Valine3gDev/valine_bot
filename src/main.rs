mod features;
mod utils;

use std::{env, sync::Arc};

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

    let _ = dotenvy::dotenv();

    let token = env::var("TOKEN").expect("Expected a TOKEN in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILDS | GatewayIntents::MESSAGE_CONTENT;
    let mut settings = CacheSettings::default();
    settings.max_messages = 1_000_000;
    let mut client = Client::builder(&token, intents)
        .event_handler(MainHandler)
        .event_handler(features::AuthHandler)
        .event_handler(features::LoggingHandler)
        .event_handler(features::MessageCacheHandler { disabled: false })
        .cache_settings(settings)
        .type_map_insert::<MessageCacheType>(Arc::new(MessageCache::new()))
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
