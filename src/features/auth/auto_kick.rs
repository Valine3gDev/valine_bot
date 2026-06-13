use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use futures::{StreamExt, stream};
use serenity::{
    all::{Context, prelude::CacheHttp},
    async_trait,
    model::{Color, event::FullEvent},
};
use tokio::pin;
use tracing::error;

use crate::{
    app::BotDataGetter,
    core::BotEventHandler,
    features::auth::utils::create_auth_log_message,
    utils::{create_message, send_message},
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

    async fn run_kick_loop(ctx: Context) {
        loop {
            let config = ctx.read_app_config().await;

            let cached_members = ctx
                .cache
                .guild(config.auto_kick.guild_id)
                // メンバーキャッシュが構築されていない場合は API から取ってくるため
                .filter(|guild| guild.members.len() as u32 >= guild.member_count.get())
                .map(|guild| guild.members.clone());
            let member_stream = if let Some(members) = cached_members {
                stream::iter(members).left_stream()
            } else {
                config
                    .auto_kick
                    .guild_id
                    .members_iter(ctx.http())
                    .filter_map(|r| async { r.ok() })
                    .right_stream()
            };
            pin!(member_stream);

            while let Some(member) = member_stream.next().await {
                if member.user.bot() {
                    continue;
                }

                if member.roles.contains(&config.auth.role_id) {
                    continue;
                }

                let joined_at = match member.joined_at {
                    Some(time) => *time,
                    None => continue,
                };

                if chrono::Utc::now().signed_duration_since(joined_at) < config.auto_kick.grace_period {
                    continue;
                }

                let dm_result = member
                    .user
                    .id
                    .direct_message(&ctx, create_message(&config.auto_kick.kick_message))
                    .await;

                if let Err(error) = member
                    .kick(ctx.http(), Some("一定期間のうちに認証ロールが付与されていないため"))
                    .await
                {
                    error!("Failed to kick user: {error:#?}");
                    // continue;
                };

                let _ = send_message(
                    &ctx,
                    &config.auth.log_channel_id,
                    create_auth_log_message(
                        "認証期限切れのため Kick",
                        Color::ORANGE,
                        &member,
                        Some(dm_result.is_ok()),
                    ),
                )
                .await;
            }

            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        if self.task_started.swap(true, Ordering::Relaxed) {
            return;
        }

        let ctx = ctx.clone();
        tokio::spawn(Self::run_kick_loop(ctx));
    }
}

#[async_trait]
impl BotEventHandler for AutoKickEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        if let FullEvent::CacheReady { .. } = event {
            self.handle_cache_ready(ctx).await;
        }
    }
}
