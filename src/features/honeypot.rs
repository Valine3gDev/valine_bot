use std::collections::HashMap;

use chrono::Duration;
use serenity::{
    all::prelude::{Context, EventHandler},
    async_trait,
    builder::CreateEmbed,
    model::{
        Timestamp,
        channel::Message,
        id::{ChannelId, GuildId, MessageId, UserId},
    },
    utils::MessageBuilder,
};
use tracing::error;

use crate::{
    config::get_config,
    utils::{create_message, create_safe_message, send_message},
};

struct MessageFingerprint {
    author_id: UserId,
    content: String,
    attachments: Vec<String>,
}

impl MessageFingerprint {
    fn matches(&self, other: &MessageFingerprint) -> bool {
        if self.author_id != other.author_id {
            return false;
        }

        if self.content != other.content {
            return false;
        }

        let mut self_attachments = self.attachments.to_vec();
        let mut other_attachments = other.attachments.to_vec();
        self_attachments.sort();
        other_attachments.sort();

        self_attachments == other_attachments
    }

    fn matches_message(&self, message: &Message) -> bool {
        self.matches(&message.into())
    }
}

impl From<Message> for MessageFingerprint {
    fn from(message: Message) -> Self {
        (&message).into()
    }
}

impl From<&Message> for MessageFingerprint {
    fn from(message: &Message) -> Self {
        Self {
            author_id: message.author.id,
            content: message.content.clone(),
            attachments: message.attachments.iter().map(|a| a.filename.clone()).collect(),
        }
    }
}

pub struct Handler;

impl Handler {
    /**
    指定されたメッセージと同一の内容を持ち、指定された期間内に送信されたメッセージのID一覧を、 Serenity のキャッシュ内から収集する

    スパム対策の性質上 Bot起動以前のメッセージが必要になる可能性が低いため、Serenityのキャッシュからのみ収集する実装とした

    また、同様にスレッドにメッセージが送信されないというスパムの傾向を踏まえ、スレッド内のメッセージは収集対象から除外する
    */
    fn collect_message_ids(
        ctx: &Context,
        guild_id: GuildId,
        target_message: impl Into<MessageFingerprint>,
        message_lookback: Duration,
    ) -> HashMap<ChannelId, Vec<MessageId>> {
        let Some(guild) = guild_id.to_guild_cached(&ctx) else {
            error!("guild {} not found in cache", guild_id);
            return HashMap::new();
        };

        let mut ids = HashMap::new();
        let cutoff = Timestamp::now().unix_timestamp() - message_lookback.num_seconds();
        let target_message = target_message.into();

        for channel_id in guild.channels.keys() {
            if let Some(messages) = ctx.cache.channel_messages(channel_id) {
                let message_ids = messages
                    .iter()
                    .filter(|(_, message)| {
                        message.timestamp.unix_timestamp() >= cutoff && target_message.matches_message(message)
                    })
                    .map(|(message_id, _)| *message_id)
                    .collect::<Vec<_>>();

                if !message_ids.is_empty() {
                    ids.insert(*channel_id, message_ids);
                }
            }
        }

        ids
    }

    async fn delete_messages(ctx: &Context, messages: &HashMap<ChannelId, Vec<MessageId>>) {
        for (channel_id, message_ids) in messages {
            if message_ids.len() > 2 {
                for chunk in message_ids.chunks(100) {
                    if let Err(e) = channel_id.delete_messages(&ctx.http, chunk).await {
                        error!("Failed to delete messages in channel {}: {:?}", channel_id, e);
                    }
                }
                continue;
            }

            if let Some(id) = message_ids.first()
                && let Err(e) = channel_id.delete_message(&ctx.http, *id).await
            {
                error!("Failed to delete message {} in channel {}: {:?}", id, channel_id, e);
            }
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let author = &msg.author;

        if author.bot {
            return;
        }

        let config = &get_config(&ctx).await;

        if config.honeypot.channel_id != msg.channel_id {
            return;
        }

        let dm_message = author
            .direct_message(&ctx, create_message(&config.honeypot.kick_message))
            .await;

        let Ok(member) = msg.member(&ctx).await else {
            error!("user {} not found", author.id);
            return;
        };

        let _ = member
            .kick_with_reason(&ctx, "ハニーポットにメッセージを送信したため")
            .await;

        let delete_message_ids =
            Self::collect_message_ids(&ctx, msg.guild_id.unwrap(), &msg, config.honeypot.message_lookback);
        Self::delete_messages(&ctx, &delete_message_ids).await;

        let mut log_builder = MessageBuilder::new();
        log_builder
            .push_bold("ユーザー: ")
            .push_safe(member.display_name())
            .push(" ")
            .push_mono_line(author.id.to_string())
            .push_line(dm_message.map_or("DMの送信に失敗しました。", |_| ""));

        log_builder.push_bold_line("削除したメッセージID:");
        for (channel_id, message_ids) in &delete_message_ids {
            for message_id in message_ids {
                log_builder
                    .push("- ")
                    .push(message_id.link(*channel_id, msg.guild_id))
                    .push(" ")
                    .push_mono_line(message_id.to_string());
            }
        }

        let embed = CreateEmbed::new()
            .title("ハニーポット検知")
            .description(log_builder.build())
            .color(0xf00000)
            .thumbnail(
                author
                    .avatar_url()
                    .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            );

        let _ = send_message(
            &ctx,
            &config.honeypot.log_channel_id,
            create_safe_message().add_embed(embed),
        )
        .await;
    }
}
