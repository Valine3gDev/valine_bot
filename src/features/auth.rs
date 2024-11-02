use std::env;
use std::sync::LazyLock;

use regex::Regex;
use serenity::all::{ChannelId, GuildId, MessageId, MessageUpdateEvent, ReactionType, RoleId, User};
use serenity::model::channel::Message;
use serenity::prelude::*;
use serenity::{async_trait, utils};
use tracing::error;

use crate::utils::{create_message, get_message, react_from_id};

#[rustfmt::skip]
static TRIGGER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(&env::var("TRIGGER_REGEX").unwrap()).unwrap());
#[rustfmt::skip]
static CHANNEL_ID: LazyLock<ChannelId> = LazyLock::new(|| ChannelId::new(env::var("CHANNEL_ID").unwrap().parse().unwrap()));
#[rustfmt::skip]
static LOG_CHANNEL_ID: LazyLock<ChannelId> = LazyLock::new(|| ChannelId::new(env::var("LOG_CHANNEL_ID").unwrap().parse().unwrap()));
#[rustfmt::skip]
static ROLE_ID: LazyLock<RoleId> = LazyLock::new(|| RoleId::new(env::var("ROLE_ID").unwrap().parse().unwrap()));
#[rustfmt::skip]
static AUTHENTICATED_REACTION: LazyLock<ReactionType> = LazyLock::new(|| utils::parse_emoji(env::var("AUTHENTICATED_REACTION").unwrap()).unwrap().into());

pub struct Handler;

impl Handler {
    async fn handle_message(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        channel_id: ChannelId,
        message_id: MessageId,
        author: User,
        content: String,
    ) {
        if !TRIGGER_REGEX.is_match(&content) {
            return;
        }

        if channel_id != *CHANNEL_ID {
            return;
        }

        let member = match guild_id.member(&ctx.http, author.id).await {
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

        if let Err(why) = react_from_id(ctx, channel_id, message_id, &AUTHENTICATED_REACTION).await {
            error!("Failed to react to message: {:?}", why);
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

        self.handle_message(&ctx, guild_id, msg.channel_id, msg.id, msg.author, msg.content)
            .await;
    }

    async fn message_update(&self, ctx: Context, _: Option<Message>, _: Option<Message>, event: MessageUpdateEvent) {
        let Some(guild_id) = event.guild_id else {
            return error!("Failed to get guild id: {:?}", event);
        };
        let Some(author) = event.author else {
            return error!("Failed to get author: {:?}", event);
        };
        if let Some(content) = event.content {
            self.handle_message(&ctx, guild_id, event.channel_id, event.id, author, content)
                .await;
            return;
        }

        match get_message(&ctx, event.channel_id, event.id).await {
            Ok(m) => {
                self.handle_message(&ctx, guild_id, event.channel_id, event.id, author, m.content)
                    .await
            }
            Err(why) => error!("Failed to get message: {:?}", why),
        }
    }
}
