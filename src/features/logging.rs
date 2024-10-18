use std::{env, sync::LazyLock};

use itertools::enumerate;
use serenity::{
    all::{
        ChannelId, Color, Context, CreateEmbed, EmbedMessageBuilding, EventHandler, FormattedTimestamp,
        FormattedTimestampStyle, GuildId, Mentionable, Message, MessageBuilder, MessageId, MessageUpdateEvent,
        Timestamp,
    },
    async_trait,
};
use tracing::error;

use crate::{
    features::MessageCacheType,
    utils::{create_diff_lines_text, create_safe_message, get_cached_message},
};

#[rustfmt::skip]
static LOG_CHANNEL_ID: LazyLock<ChannelId> = LazyLock::new(|| ChannelId::new(env::var("MESSAGE_LOG_CHANNEL_ID").unwrap().parse().unwrap()));

pub struct Handler;

impl Handler {
    pub fn build_embed(&self, message: &Message, new_content: String, mut embed: CreateEmbed) -> CreateEmbed {
        if let Some(message_reference) = &message.message_reference {
            let id = message_reference.message_id.unwrap_or(MessageId::default());

            let mut builder = MessageBuilder::new();
            builder
                .push_bold_safe("元メッセージ: ")
                .push_safe(id.link(message_reference.channel_id, message_reference.guild_id))
                .push_safe(" ")
                .push_mono_line_safe(id.to_string());

            embed = embed.field("__**転送**__", builder.build(), false);
        }

        if let Some(poll) = &message.poll {
            let mut builder = MessageBuilder::new();
            builder
                .push_bold_safe("タイトル: ")
                .push_line_safe(poll.question.text.clone().unwrap_or("<不明なタイトル>".to_string()))
                .push_bold_line_safe("回答:");

            let results = &poll.results;

            for (i, answer) in enumerate(&poll.answers) {
                builder.push_safe(&format!(
                    "- {}",
                    answer.poll_media.text.clone().unwrap_or("<不明な回答>".to_string())
                ));

                match results {
                    Some(results) => builder.push_line_safe(&format!(": {}票", results.answer_counts[i].count)),
                    None => builder.push_safe("\n"),
                };
            }

            if let Some(expiry) = poll.expiry {
                let formatted = FormattedTimestamp::new(expiry, Some(FormattedTimestampStyle::LongDateTime));
                builder
                    .push_bold_safe("有効期限:")
                    .push_line_safe(formatted.to_string());
            }

            embed = embed.field("__**投票**__", builder.build(), false);
        }

        if !message.content.is_empty() {
            let mut changed = MessageBuilder::new();
            changed.push_codeblock_safe(create_diff_lines_text(&message.content, &new_content), Some("diff"));

            embed = embed.field("__**テキスト差分**__", changed.build(), false);
        }

        if !message.attachments.is_empty() {
            let mut changed = MessageBuilder::new();
            for attachment in &message.attachments {
                changed
                    .push_safe("- ")
                    .push_named_link_safe(&attachment.filename, &attachment.url)
                    .push_safe("\n");
            }
            embed = embed.field("__**添付ファイル**__", changed.build(), false);
        }
        embed
    }

    async fn update_cache(&self, ctx: &Context, message: &Message) {
        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<MessageCacheType>().unwrap();
        cache.insert(message.clone());
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message_update(&self, ctx: Context, old: Option<Message>, _: Option<Message>, event: MessageUpdateEvent) {
        let Some(message) = get_cached_message(&ctx, event.channel_id, event.id).await else {
            return error!("Failed to get message: {:?}", event.id);
        };
        let message = old.unwrap_or(message);

        let Ok(new_message) = event.channel_id.message(&ctx.http, event.id).await else {
            return error!("Failed to get new message: {:?}", event.id);
        };

        if message.content == new_message.content {
            return;
        }

        self.update_cache(&ctx, &new_message).await;

        let timestamp = FormattedTimestamp::new(Timestamp::now(), Some(FormattedTimestampStyle::LongDateTime));
        let description = MessageBuilder::new()
            .push_bold_safe("メンバー: ")
            .mention(&message.author.mention())
            .push_safe(" ")
            .push_mono_line_safe(message.author.id.to_string())
            .push_bold_safe("メッセージ: ")
            .push_safe(message.id.link(message.channel_id, message.guild_id))
            .push_safe(" ")
            .push_mono_line_safe(message.id.to_string())
            .push_bold_safe("時刻: ")
            .push_line_safe(timestamp.to_string())
            .build();

        let mut embed = CreateEmbed::new()
            .title("メッセージ編集ログ")
            .description(description)
            .color(Color::new(0xff8800))
            .thumbnail(
                message
                    .author
                    .avatar_url()
                    .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            );

        embed = self.build_embed(&message, new_message.content, embed);
        let log = create_safe_message().add_embed(embed);
        if let Err(why) = LOG_CHANNEL_ID.send_message(&ctx.http, log).await {
            error!("Error sending message: {:?}", why)
        }
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        deleted_message_id: MessageId,
        _: Option<GuildId>,
    ) {
        let Some(message) = get_cached_message(&ctx, channel_id, deleted_message_id).await else {
            return error!("Failed to get message: {:?}", deleted_message_id);
        };

        let timestamp = FormattedTimestamp::new(Timestamp::now(), Some(FormattedTimestampStyle::LongDateTime));
        let description = MessageBuilder::new()
            .push_bold_safe("メンバー: ")
            .mention(&message.author.mention())
            .push_safe(" ")
            .push_mono_line_safe(message.author.id.to_string())
            .push_bold_safe("チャンネル: ")
            .mention(&message.channel_id.mention())
            .push_safe(" ")
            .push_mono_line_safe(message.id.to_string())
            .push_bold_safe("時刻: ")
            .push_line_safe(timestamp.to_string())
            .build();

        let mut embed = CreateEmbed::new()
            .title("メッセージ削除ログ")
            .description(description)
            .color(Color::new(0xf00000))
            .thumbnail(
                message
                    .author
                    .avatar_url()
                    .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            );

        embed = self.build_embed(&message, "".to_string(), embed);
        let log = create_safe_message().add_embed(embed);
        if let Err(why) = LOG_CHANNEL_ID.send_message(&ctx.http, log).await {
            error!("Error sending message: {:?}", why)
        }
    }
}
