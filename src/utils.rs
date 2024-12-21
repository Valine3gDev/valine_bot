use std::time::Duration;

use itertools::Itertools;
use serenity::{
    all::{ChannelId, Context, CreateAllowedMentions, CreateMessage, GuildChannel, Message, MessageId},
    Result,
};
use similar::{Algorithm, ChangeTag, TextDiff};
use tracing::error;

use crate::{config::get_config, error::BotError, MessageCacheType, PContext, PError};

pub fn create_safe_message() -> CreateMessage {
    CreateMessage::new().allowed_mentions(CreateAllowedMentions::new().all_users(false))
}

pub fn create_message(content: impl Into<String>) -> CreateMessage {
    create_safe_message().content(content)
}

/**
thread_create イベントにおいて、初期メッセージが送信されるか5秒経過するまで待機する

初期メッセージが送信されると、falseを返し、既に初期メッセージが存在する場合、true を返す
```rs
pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        if await_thread_create(&ctx, &thread).await {
            return;
        }

        // 処理
    }
}
```
 */
pub async fn await_initial_message(ctx: &Context, thread: &GuildChannel) -> bool {
    // Botがメッセージを送信すると二度イベントが発火するので、初期メッセージ送信後のイベントは無視する
    if thread.last_message_id.is_some() {
        return true;
    }

    // 初期メッセージが送信されるか、5秒経つまで待機
    thread
        .await_reply(&ctx.shard)
        .channel_id(thread.id)
        .author_id(thread.owner_id.unwrap())
        .timeout(Duration::from_secs(5))
        .await;
    false
}

/*
認証済みロールを持っているかどうかを確認します。
*/
pub async fn has_authed_role(ctx: PContext<'_>) -> Result<bool, PError> {
    let Some(member) = ctx.author_member().await else {
        return Ok(false);
    };

    let config = &get_config(ctx.serenity_context()).await.auth;
    if !member.roles.contains(&config.role_id) {
        Err(BotError::HasNoRole.into())
    } else {
        Ok(true)
    }
}

pub async fn send_message(ctx: &Context, channel_id: &ChannelId, builder: CreateMessage) -> Result<Message> {
    match channel_id.send_message(&ctx.http, builder).await {
        Ok(m) => Ok(m),
        Err(why) => {
            error!("Error sending message: {:?}", why);
            Err(why)
        }
    }
}

pub async fn get_cached_message(ctx: &Context, channel_id: ChannelId, message_id: MessageId) -> Option<Message> {
    if let Some(m) = ctx.cache.message(channel_id, message_id) {
        return Some(m.clone());
    }

    let data = ctx.data.read().await;
    let cache = data.get::<MessageCacheType>().unwrap();
    if let Some(m) = cache.get(channel_id, message_id) {
        return Some(m);
    }

    None
}

pub fn create_diff_lines_text(old: &str, new: &str) -> String {
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_lines(old, new);
    diff.iter_all_changes()
        .map(|c| match c.tag() {
            ChangeTag::Delete => format!("- {}", c),
            ChangeTag::Insert => format!("+ {}", c),
            ChangeTag::Equal => format!("  {}", c),
        })
        .join("")
}
