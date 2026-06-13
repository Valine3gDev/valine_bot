use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures::{StreamExt, stream};
use serenity::{
    all::{Context, MessageBuilder, prelude::Mentionable},
    async_trait,
    builder::CreateEmbed,
    model::{Color, event::FullEvent},
};
use tokio::pin;
use tracing::error;

use crate::{
    app::BotDataGetter,
    core::BotEventHandler,
    utils::{create_message, create_safe_message, send_message},
};

pub struct AutoKickEventHandler {
    task_started: AtomicBool,
}

impl AutoKickEventHandler {
    pub fn new() -> Self {
        Self {
            task_started: AtomicBool::new(false),
        }
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        if self.task_started.swap(true, Ordering::Relaxed) {
            return;
        }

        let ctx = ctx.clone();
        tokio::spawn(async move {
            let config = ctx.read_app_config().await;

            loop {
                let cached_members = ctx
                    .cache
                    .guild(config.auto_kick.guild_id)
                    // メンバーキャッシュが構築されていない場合は API から取ってくるため
                    .filter(|guild| guild.members.len() as u32 >= guild.member_count.get())
                    .map(|guild| guild.members.clone());
                let members = if let Some(members) = cached_members {
                    stream::iter(members).left_stream()
                } else {
                    config
                        .auto_kick
                        .guild_id
                        .members_iter(&ctx.http)
                        .filter_map(|r| async { r.ok() })
                        .right_stream()
                };
                pin!(members);

                while let Some(member) = members.next().await {
                    if member.user.bot() {
                        continue;
                    }

                    if member.roles.contains(&config.auth.role_id) {
                        continue;
                    }

                    let joined = match member.joined_at {
                        Some(time) => *time,
                        None => continue,
                    };

                    if chrono::Utc::now().signed_duration_since(joined) < config.auto_kick.grace_period {
                        continue;
                    }

                    let dm_message = member
                        .user
                        .id
                        .direct_message(&ctx, create_message(&config.auto_kick.kick_message))
                        .await;

                    if let Err(e) = member
                        .kick(&ctx.http, Some("一定期間のうちに認証ロールが付与されていないため"))
                        .await
                    {
                        error!("Failed to kick user: {:?}", e);
                        continue;
                    };

                    let log = MessageBuilder::new()
                        .push_bold("ユーザー: ")
                        .push_safe(member.display_name())
                        .push(" ")
                        .push_mono_line(&*member.user.id.to_string())
                        .push_bold("メンション: ")
                        .push_line_safe(&*member.mention().to_string())
                        .push_bold("リンク")
                        .push_line_safe(&*format!("<https://discord.com/users/{}>", member.user.id))
                        .push_line(dm_message.map_or("DMの送信に失敗しました。", |_| ""))
                        .build();

                    let embed = CreateEmbed::new()
                        .title("自動 Kick")
                        .description(log)
                        .color(Color::ORANGE)
                        .thumbnail(
                            member
                                .user
                                .avatar_url()
                                .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
                            Some("ユーザーアイコン".into()),
                        );

                    let _ = send_message(&ctx, &config.auth.log_channel_id, create_safe_message().embed(embed)).await;
                }

                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }
}

#[async_trait]
impl BotEventHandler for AutoKickEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        if let FullEvent::CacheReady { guilds, .. } = event {
            self.handle_cache_ready(ctx).await;
        }
    }
}
