use std::ops::Not;

use itertools::{Itertools, enumerate};
use serenity::{
    all::{
        ChannelId, Context, CreateEmbed, EmbedMessageBuilding, EventHandler, GuildId, Mentionable, Message,
        MessageBuilder, MessageId, MessageReferenceKind, MessageUpdateEvent, Timestamp,
    },
    async_trait,
};
use tracing::error;

use crate::{
    config::get_config,
    extensions::MessageBuilderTimestampExt,
    features::MessageCacheType,
    utils::{create_diff_lines_text, create_safe_message, get_cached_message, send_message},
};

enum LogType {
    Edit { new_content: String },
    Delete,
}

impl LogType {
    fn name(&self) -> &'static str {
        match self {
            LogType::Edit { .. } => "編集",
            LogType::Delete => "削除",
        }
    }

    fn title(&self) -> String {
        format!("メッセージ{}ログ", self.name())
    }

    fn color(&self) -> i32 {
        match self {
            LogType::Edit { .. } => 0xff8800,
            LogType::Delete => 0xf00000,
        }
    }

    fn new_content(&self) -> Option<&str> {
        match self {
            LogType::Edit { new_content } => Some(new_content),
            LogType::Delete => None,
        }
    }
}

pub struct Handler;

impl Handler {
    fn build_reply_field(embed: CreateEmbed, message: &Message) -> CreateEmbed {
        let Some(m_ref) = &message.message_reference else {
            return embed;
        };
        let id = m_ref.message_id.unwrap_or(MessageId::default());
        let (name, content) = match m_ref.kind {
            MessageReferenceKind::Default => ("__**返信**__", "返信先: "),
            MessageReferenceKind::Forward => ("__**転送**__", "転送元: "),
            _ => ("__**不明**__", "不明な対象メッセージ: "),
        };
        embed.field(
            name,
            MessageBuilder::new()
                .push_bold_safe(content)
                .push_safe(id.link(m_ref.channel_id, m_ref.guild_id))
                .push_safe(" ")
                .push_mono_line_safe(id.to_string())
                .build(),
            false,
        )
    }

    fn build_poll_field(embed: CreateEmbed, message: &Message) -> CreateEmbed {
        let Some(poll) = &message.poll else {
            return embed;
        };

        let mut builder = MessageBuilder::new();
        builder
            .push_bold_safe("タイトル: ")
            .push_line_safe(poll.question.text.clone().unwrap_or("<不明なタイトル>".to_string()))
            .push_bold_line_safe("回答:");

        for (i, answer) in enumerate(&poll.answers) {
            let answer_text = answer.poll_media.text.clone().unwrap_or("<不明な回答>".to_string());
            builder.push_safe(format!("- {}", answer_text));
            if let Some(results) = &poll.results {
                builder.push_line_safe(format!(": {}票", results.answer_counts[i].count));
            } else {
                builder.push_safe("\n");
            }
        }

        if let Some(expiry) = poll.expiry {
            builder
                .push_bold_safe("有効期限: ")
                .push_timestamp_long_date_time_line(expiry);
        }

        embed.field("__**投票**__", builder.build(), false)
    }

    fn build_diff_field(mut embed: CreateEmbed, old_content: &str, new_content: &str) -> CreateEmbed {
        if old_content.is_empty() {
            return embed;
        }

        let diff = create_diff_lines_text(old_content, new_content);
        let chunks = diff.lines().peekable().batching(|lines| {
            let mut str = String::new();
            while let Some(line) = lines.next_if(|&l| str.len() + l.len() <= 1000) {
                str.push_str(line);
                str.push('\n');
            }
            str.is_empty().not().then_some(str)
        });

        for (i, chunk) in enumerate(chunks) {
            let changed = MessageBuilder::new().push_codeblock_safe(chunk, Some("diff")).build();
            embed = embed.field(if i == 0 { "__**テキスト差分**__" } else { "" }, changed, false)
        }
        embed
    }

    fn build_attachments_field(embed: CreateEmbed, message: &Message) -> CreateEmbed {
        if message.attachments.is_empty() {
            return embed;
        }
        let mut builder = MessageBuilder::new();
        for attachment in &message.attachments {
            builder
                .push_safe("- ")
                .push_named_link_safe(&attachment.filename, &attachment.url)
                .push_safe("\n");
        }
        embed.field("__**添付ファイル**__", builder.build(), false)
    }

    fn build_embed(&self, message: &Message, new_content: &str, mut embed: CreateEmbed) -> CreateEmbed {
        embed = Self::build_reply_field(embed, message);
        embed = Self::build_poll_field(embed, message);
        embed = Self::build_diff_field(embed, &message.content, new_content);
        Self::build_attachments_field(embed, message)
    }

    async fn create_and_send_log(&self, ctx: &Context, message: &Message, log_type: LogType) {
        if message.author.bot {
            return;
        }

        let description = MessageBuilder::new()
            .push_bold_safe("メンバー: ")
            .mention(&message.author.mention())
            .push_safe(" ")
            .push_mono_line_safe(message.author.id.to_string())
            .push_bold_safe("メッセージ: ")
            .push_safe(message.id.link(message.channel_id, message.guild_id))
            .push_safe(" ")
            .push_mono_line_safe(message.id.to_string())
            .push_bold_safe("メッセージ送信日時: ")
            .push_timestamp_long_date_time_line(message.timestamp)
            .push_bold_safe(format!("{}日時: ", log_type.name()))
            .push_timestamp_long_date_time_line(Timestamp::now())
            .build();

        let mut embed = CreateEmbed::new()
            .title(log_type.title())
            .description(description)
            .color(log_type.color())
            .thumbnail(
                message
                    .author
                    .avatar_url()
                    .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            );

        let new_content = log_type.new_content().unwrap_or("");
        embed = self.build_embed(message, new_content, embed);
        self.send_log(ctx, embed).await;
    }

    async fn update_cache(&self, ctx: &Context, message: &Message) {
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MessageCacheType>().unwrap();
        cache.insert(message.clone());
    }

    async fn send_log(&self, ctx: &Context, embed: CreateEmbed) {
        let config = get_config(ctx).await;
        let log = create_safe_message().add_embed(embed);
        let _ = send_message(ctx, &config.message_logging.channel_id, log).await;
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message_update(&self, ctx: Context, old: Option<Message>, _: Option<Message>, event: MessageUpdateEvent) {
        let Some(message) = get_cached_message(&ctx, event.channel_id, event.id).await else {
            return error!("Failed to get message: {}", event.id);
        };
        let message = old.unwrap_or(message);

        let Ok(new_message) = event.channel_id.message(&ctx.http, event.id).await else {
            return error!("Failed to get new message: {}", event.id);
        };

        if message.content == new_message.content {
            return;
        }

        self.update_cache(&ctx, &new_message).await;
        self.create_and_send_log(
            &ctx,
            &message,
            LogType::Edit {
                new_content: new_message.content,
            },
        )
        .await;
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_id: MessageId,
        _: Option<GuildId>,
    ) {
        let Some(message) = get_cached_message(&ctx, channel_id, deleted_message_id).await else {
            return error!("Failed to get message: {}", deleted_message_id);
        };

        self.create_and_send_log(&ctx, &message, LogType::Delete).await;
    }

    async fn message_delete_bulk(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_ids: Vec<MessageId>,
        _: Option<GuildId>,
    ) {
        for message_id in deleted_message_ids {
            let Some(message) = get_cached_message(&ctx, channel_id, message_id).await else {
                error!("Failed to get message: {}", message_id);
                continue;
            };
            self.create_and_send_log(&ctx, &message, LogType::Delete).await;
        }
    }
}
