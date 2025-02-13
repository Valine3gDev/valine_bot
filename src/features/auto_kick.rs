use std::time::Duration;

use futures::StreamExt;
use serenity::{
    all::{Context, EventHandler, GuildId, MessageBuilder},
    async_trait,
};
use tokio::pin;
use tracing::error;

use crate::{
    config::get_config,
    utils::{create_message, send_message},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn cache_ready(&self, ctx: Context, _: Vec<GuildId>) {
        tokio::spawn(async move {
            let config = get_config(&ctx).await;

            loop {
                let members = config.auto_kick.guild_id.members_iter(&ctx);
                pin!(members);

                while let Some(Ok(member)) = members.next().await {
                    if member.user.bot {
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

                    if let Err(e) = member.kick(&ctx).await {
                        error!("Failed to kick user: {:?}", e);
                        continue;
                    };

                    let dm_message = member
                        .user
                        .id
                        .direct_message(&ctx, create_message(&config.auto_kick.kick_message))
                        .await;

                    let log = create_message(
                        MessageBuilder::new()
                            .push_safe(member.display_name())
                            .push(" (")
                            .push_mono(member.user.id.to_string())
                            .push(") をキックしました。")
                            .push(dm_message.map(|_| "").unwrap_or_else(|_| "DMの送信に失敗しました。"))
                            .build(),
                    );
                    let _ = send_message(&ctx, &config.auth.log_channel_id, log).await;
                }

                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });
    }
}
