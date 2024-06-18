use std::env;

use once_cell::sync::Lazy;
use regex::Regex;
use serenity::all::{
    ChannelId, CreateAllowedMentions, CreateMessage, GuildId, MessageUpdateEvent, Ready, RoleId
};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use tracing::{error, info};

static TRIGGER_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(&env::var("TRIGGER_REGEX").unwrap()).unwrap());
static CHANNEL_ID: Lazy<ChannelId> =
    Lazy::new(|| ChannelId::new(env::var("CHANNEL_ID").unwrap().parse().unwrap()));
static LOG_CHANNEL_ID: Lazy<ChannelId> =
    Lazy::new(|| ChannelId::new(env::var("LOG_CHANNEL_ID").unwrap().parse().unwrap()));
static ROLE_ID: Lazy<RoleId> =
    Lazy::new(|| RoleId::new(env::var("ROLE_ID").unwrap().parse().unwrap()));

fn create_message(content: String) -> CreateMessage {
    CreateMessage::new()
        .content(content)
        .allowed_mentions(CreateAllowedMentions::new().all_users(false))
}

struct Handler;

impl Handler {
    async fn handle_message(&self, ctx: &Context, guild_id: GuildId, msg: Message) {
        if !TRIGGER_REGEX.is_match(&msg.content) {
            return;
        }

        if msg.channel_id != *CHANNEL_ID {
            return;
        }

        let member = match guild_id.member(&ctx.http, msg.author.id).await {
            Ok(member) => member,
            Err(why) => return error!("Failed to get member: {:?}", why),
        };

        if member.roles.contains(&ROLE_ID) {
            return;
        }

        if let Err(why) = member.add_role(&ctx.http, *ROLE_ID).await {
            let log = create_message(format!(
                "{} にロールを追加できませんでした。\n```\n{}```",
                member.mention(),
                why
            ));
            if let Err(why) = LOG_CHANNEL_ID.send_message(&ctx.http, log).await {
                error!("Error sending message: {:?}", why)
            }
            return error!("Failed to add role: {:?}", why);
        }

        let log = create_message(format!("{} にロールを追加しました。", member.mention()));
        if let Err(why) = LOG_CHANNEL_ID.send_message(&ctx.http, log).await {
            error!("Error sending message: {:?}", why)
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let guild_id = match msg.guild_id {
            Some(guild_id) => guild_id,
            None => return error!("Failed to get guild id: {:?}", msg),
        };
        self.handle_message(&ctx, guild_id, msg).await;
    }

    async fn message_update(
        &self,
        ctx: Context,
        _: Option<Message>,
        _: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        let guild_id = match event.guild_id {
            Some(guild_id) => guild_id,
            None => return error!("Failed to get guild id: {:?}", event),
        };
        match event.channel_id.message(&ctx.http, event.id).await {
            Ok(msg) => {
                self.handle_message(&ctx, guild_id, msg).await
            }
            Err(why) => error!("Failed to get message: {:?}", why),
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let _ = dotenvy::dotenv();

    let token = env::var("TOKEN").expect("Expected a TOKEN in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
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
