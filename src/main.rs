mod config;
mod error;
mod features;
mod utils;

use std::{fs::read_to_string, sync::Arc};

use bpaf::Bpaf;
use config::Config;
use features::{commands, MessageCache, MessageCacheType};
use poise::{say_reply, Framework, FrameworkError, FrameworkOptions};
use serenity::{
    all::{MessageParseError, RatelimitInfo, Ready},
    async_trait,
    cache::Settings as CacheSettings,
    prelude::*,
};
use tracing::{error, info};

pub type PError = Box<dyn std::error::Error + Send + Sync>;
pub struct CommandData {}
pub type PContext<'a> = poise::Context<'a, CommandData, PError>;
pub type PCommand = poise::Command<CommandData, PError>;

struct MainHandler;

#[async_trait]
impl EventHandler for MainHandler {
    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }

    async fn ratelimit(&self, data: RatelimitInfo) {
        info!("Ratelimited {}: {}s", data.path, data.timeout.as_secs());
    }
}

#[derive(Clone, Debug, Bpaf)]
#[bpaf(options, version)]
struct Options {
    #[bpaf(short, long)]
    check_config: bool,
}

pub async fn on_error(error: FrameworkError<'_, CommandData, PError>) {
    match error {
        FrameworkError::Command { error, ctx, .. } => {
            let _ = say_reply(ctx, "コマンド実行中にエラーが発生しました。").await;
            error!("Command error: Command: {:?}, Error: {:?}", ctx.command(), error);
        }
        FrameworkError::ArgumentParse { ctx, input, error, .. } => {
            let Some(input) = input else {
                return error!("Error parsing input: {:?}", error);
            };

            let error = match error.downcast_ref::<MessageParseError>() {
                Some(MessageParseError::Malformed) => {
                    "メッセージとして解析できませんでした。\nメッセージID、メッセージURL形式で入力してください。"
                }
                Some(MessageParseError::Http(_)) => "メッセージを取得できませんでした。",
                _ => &error.to_string(),
            };

            let _ = say_reply(ctx, format!("入力 `{}` の解析に失敗しました。\n{}", input, error)).await;
        }
        FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let msg = format!(
                "ボットに権限が無いためコマンドを実行できません: {}",
                missing_permissions,
            );
            let _ = say_reply(ctx, msg).await;
        }
        FrameworkError::NotAnOwner { ctx, .. } => {
            let _ = say_reply(ctx, "このコマンドはボットのオーナーのみ実行できます。").await;
        }
        FrameworkError::CooldownHit {
            remaining_cooldown,
            ctx,
            ..
        } => {
            let _ = say_reply(
                ctx,
                format!(
                    "このコマンドはクールダウン中です。残り時間: {}秒",
                    remaining_cooldown.as_secs()
                ),
            )
            .await;
        }
        FrameworkError::CommandCheckFailed { ctx, error, .. } => {
            let error = match error {
                Some(error) => error.to_string(),
                None => "コマンドの実行条件を満たしていません。".to_string(),
            };
            let _ = say_reply(ctx, error).await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
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
        .event_handler(MainHandler)
        .event_handler(features::LoggingHandler)
        .event_handler(features::ThreadAutoInviteHandler)
        .event_handler(features::ThreadChannelStartupHandler)
        .event_handler(features::QuestionHandler)
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
