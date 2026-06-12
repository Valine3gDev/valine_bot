use std::collections::HashMap;

use chrono::Duration;
use serenity::{
    all::prelude::Context,
    builder::CreateEmbed,
    model::{
        Timestamp,
        channel::Message,
        event::FullEvent,
        id::{ChannelId, GuildId, MessageId, UserId},
    },
    utils::MessageBuilder,
};
use tracing::error;
use valine_bot_macros::event_handler;

use crate::{
    app::BotDataGetter,
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
            content: message.content.clone().into_string(),
            attachments: message
                .attachments
                .iter()
                .map(|a| a.filename.clone().into_string())
                .collect(),
        }
    }
}

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
    let Some(guild) = guild_id.to_guild_cached(&ctx.cache) else {
        error!("guild {} not found in cache", guild_id);
        return HashMap::new();
    };

    let mut ids = HashMap::new();
    let cutoff = Timestamp::now().unix_timestamp() - message_lookback.num_seconds();
    let target_message = target_message.into();

    for channel in &guild.channels {
        let id = channel.id;
        if let Some(messages) = ctx.cache.channel_messages(id.into()) {
            let message_ids = messages
                .iter()
                .filter(|m| m.timestamp.unix_timestamp() >= cutoff && target_message.matches_message(m))
                .map(|m| m.id)
                .collect::<Vec<_>>();

            if !message_ids.is_empty() {
                ids.insert(id, message_ids);
            }
        }
    }

    ids
}

async fn delete_messages(ctx: &Context, messages: &HashMap<ChannelId, Vec<MessageId>>) {
    static DELETE_REASON: Option<&str> = Some("ハニーポットに送信されたメッセージと同一のため");

    for (channel_id, message_ids) in messages {
        let channel_id = channel_id.widen();
        if message_ids.len() > 2 {
            for chunk in message_ids.chunks(100) {
                if let Err(e) = channel_id.delete_messages(&ctx.http, chunk, DELETE_REASON).await {
                    error!("Failed to delete messages in channel {}: {:?}", channel_id, e);
                }
            }
            continue;
        }

        if let Some(id) = message_ids.first()
            && let Err(e) = channel_id.delete_message(&ctx.http, *id, DELETE_REASON).await
        {
            error!("Failed to delete message {} in channel {}: {:?}", id, channel_id, e);
        }
    }
}

#[event_handler]
pub async fn handle_honeypot_event(ctx: &Context, event: &FullEvent) {
    if let FullEvent::Message { new_message, .. } = event {
        let author = &new_message.author;

        if author.bot() {
            return;
        }

        let config = ctx.read_app_config().await;

        if config.honeypot.channel_id != new_message.channel_id.expect_channel() {
            return;
        }

        let dm_message = author
            .id
            .direct_message(&ctx, create_message(&config.honeypot.kick_message))
            .await;

        let Ok(member) = new_message.member(&ctx).await else {
            error!("user {} not found", author.id);
            return;
        };

        let _ = member
            .kick(&ctx.http, Some("ハニーポットにメッセージを送信したため"))
            .await;

        let delete_message_ids = collect_message_ids(
            ctx,
            new_message.guild_id.unwrap(),
            new_message,
            config.honeypot.message_lookback,
        );
        delete_messages(ctx, &delete_message_ids).await;

        let mut log_builder = MessageBuilder::new()
            .push_bold("ユーザー: ")
            .push_safe(member.display_name())
            .push(" ")
            .push_mono_line(&*author.id.to_string())
            .push_line(dm_message.map_or("DMの送信に失敗しました。", |_| ""))
            .push_bold_line("削除したメッセージID:");

        for (channel_id, message_ids) in &delete_message_ids {
            for message_id in message_ids {
                log_builder = log_builder
                    .push("- ")
                    .push(&*message_id.link(channel_id.widen(), new_message.guild_id).to_string())
                    .push(" ")
                    .push_mono_line(&*message_id.to_string());
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
                Some("ユーザーアイコン".into()),
            );

        let _ = send_message(
            ctx,
            &config.honeypot.log_channel_id,
            create_safe_message().add_embed(embed),
        )
        .await;
    }
}
