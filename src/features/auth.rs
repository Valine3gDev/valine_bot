use serenity::all::{ChannelId, GuildId, MessageId, MessageUpdateEvent, User};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;
use tracing::error;

use crate::config::get_config;
use crate::utils::{create_message, get_message, react_from_id, send_message};

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
        let config = &get_config(ctx).await.auth;

        if !config.trigger_regex.is_match(&content) {
            return;
        }

        if channel_id != config.channel_id {
            return;
        }

        let member = match guild_id.member(&ctx.http, author.id).await {
            Ok(member) => member,
            Err(why) => return error!("Failed to get member: {:?}", why),
        };

        if member.roles.contains(&config.role_id) {
            error!("{} already has the role", member.user.name);
            return;
        }

        if let Err(why) = member.add_role(&ctx.http, config.role_id).await {
            let log = create_message(format!(
                "{} にロールを追加できませんでした。\n```\n{}```",
                member.mention(),
                why
            ));
            let _ = send_message(ctx, &config.log_channel_id, log).await;
            return error!("Failed to add role: {:?}", why);
        }

        react_from_id(ctx, channel_id, message_id, &config.authenticated_reaction).await;

        let log = create_message(format!("{} にロールを追加しました。", member.mention()));
        let _ = send_message(ctx, &config.log_channel_id, log).await;
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
