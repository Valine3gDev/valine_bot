mod config;
mod error;
mod features;
mod utils;

use std::{
    fs::read_to_string,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use bpaf::Bpaf;
use config::Config;
use error::on_error;
use features::{commands, MessageCache, MessageCacheType};
use poise::{Framework, FrameworkOptions};
use serenity::{
    all::{ActivityData, GuildId, RatelimitInfo, Ready},
    async_trait,
    cache::Settings as CacheSettings,
    prelude::*,
};
use sysinfo::{Pid, System};
use tracing::{error, info};

pub type PError = Box<dyn std::error::Error + Send + Sync>;
pub struct CommandData {}
pub type PContext<'a> = poise::Context<'a, CommandData, PError>;
pub type PCommand = poise::Command<CommandData, PError>;

struct MainHandler {
    task_started: AtomicBool,
}

impl MainHandler {
    pub fn new() -> Self {
        Self {
            task_started: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl EventHandler for MainHandler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }

    async fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        if self.task_started.swap(true, Ordering::Relaxed) {
            return;
        }

        tokio::spawn(async move {
            let mut system = System::new_all();
            let pid = Pid::from_u32(std::process::id());

            loop {
                system.refresh_all();

                let Some(memory) = system.process(pid).map(|p| p.memory() as f64 / 1024.0 / 1024.0) else {
                    error!("Failed to get process info");
                    continue;
                };

                ctx.set_activity(Some(ActivityData::custom(format!("メモリ使用量: {:.1}MB", memory))));

                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    async fn ratelimit(&self, data: RatelimitInfo) {
        info!(
            "Ratelimited {} {}: {}s",
            data.method.reqwest_method(),
            data.path,
            data.timeout.as_secs()
        );
    }
}

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
    let config = match toml::from_str::<Config>(&config) {
        Ok(config) => config,
        Err(e) => {
            panic!("Failed to parse config.toml: {}", e);
        }
    };

    let options = options().run();

    if options.check_config {
        println!("Config is valid");
        return;
    }

    let framework = Framework::builder()
        .options(FrameworkOptions {
            commands: commands(),
            on_error: |error| Box::pin(on_error(error)),
            skip_checks_for_owners: true,
            owners: config.bot.owners.clone(),
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(CommandData {})
            })
        })
        .build();

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::GUILDS | GatewayIntents::MESSAGE_CONTENT;
    let mut settings = CacheSettings::default();
    settings.max_messages = 1_000_000;
    let mut client = Client::builder(&config.bot.token, intents)
        .framework(framework)
        .event_handler(MainHandler::new())
        .event_handler(features::AuthHandler::new())
        .event_handler(features::AutoKickHandler::new())
        .event_handler(features::LoggingHandler)
        .event_handler(features::ThreadAutoInviteHandler)
        .event_handler(features::ThreadChannelStartupHandler)
        .event_handler(features::QuestionHandler)
        .event_handler(features::MessageCacheHandler::new(config.message_cache.disabled))
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
