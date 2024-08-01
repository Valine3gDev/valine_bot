use std::env;
use std::sync::LazyLock;

use regex::Regex;
use serenity::all::{ChannelId, GuildId, MessageUpdateEvent, RoleId};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use tracing::error;

use crate::utils::create_message;

static TRIGGER_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&env::var("TRIGGER_REGEX").unwrap()).unwrap());
static CHANNEL_ID: LazyLock<ChannelId> =
    LazyLock::new(|| ChannelId::new(env::var("CHANNEL_ID").unwrap().parse().unwrap()));
static LOG_CHANNEL_ID: LazyLock<ChannelId> =
    LazyLock::new(|| ChannelId::new(env::var("LOG_CHANNEL_ID").unwrap().parse().unwrap()));
static ROLE_ID: LazyLock<RoleId> =
    LazyLock::new(|| RoleId::new(env::var("ROLE_ID").unwrap().parse().unwrap()));

pub struct Handler;

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
            error!("{} already has the role", member.user.name);
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
        let Some(guild_id) = msg.guild_id else {
            return error!("Failed to get guild id: {:?}", msg);
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
        let Some(guild_id) = event.guild_id else {
            return error!("Failed to get guild id: {:?}", event);
        };
        match event.channel_id.message(&ctx.http, event.id).await {
            Ok(msg) => self.handle_message(&ctx, guild_id, msg).await,
            Err(why) => error!("Failed to get message: {:?}", why),
        }
    }
}
