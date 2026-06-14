use std::{borrow::Cow, time::Duration};

use async_stream::stream;
use futures::Stream;
use itertools::Itertools;
use serenity::{
    Result,
    all::{ChannelId, Context, CreateAllowedMentions, CreateMessage, Message, prelude::CacheHttp},
    builder::{
        CreateComponent, CreateInteractionResponse, CreateInteractionResponseMessage, CreateModal, CreateModalComponent,
    },
    model::{
        channel::{ChannelType, GuildThread},
        guild::Member,
        id::GuildId,
    },
};
use similar::{Algorithm, ChangeTag, TextDiff};
use tracing::error;

use crate::app::{AppContext, AppError, BotDataExt, BotError};

pub fn create_safe_message<'a>() -> CreateMessage<'a> {
    CreateMessage::new().allowed_mentions(CreateAllowedMentions::new().all_users(false))
}

pub fn create_message<'a>(content: impl Into<Cow<'a, str>>) -> CreateMessage<'a> {
    create_safe_message().content(content)
}

pub fn create_interaction_message<'a>(
    content: impl Into<Cow<'a, str>>,
    ephemeral: bool,
    components: Option<&'a [CreateComponent<'a>]>,
) -> CreateInteractionResponse<'a> {
    let mut msg = CreateInteractionResponseMessage::new()
        .content(content)
        .ephemeral(ephemeral);

    if let Some(components) = components {
        msg = msg.components(components);
    }

    CreateInteractionResponse::Message(msg)
}

pub fn create_ephemeral_message<'a>(
    content: impl Into<Cow<'a, str>>,
    components: Option<&'a [CreateComponent<'a>]>,
) -> CreateInteractionResponse<'a> {
    create_interaction_message(content, true, components)
}

pub fn create_model<'a>(
    custom_id: impl Into<Cow<'a, str>>,
    title: impl Into<Cow<'a, str>>,
    components: impl Into<Cow<'a, [CreateModalComponent<'a>]>>,
) -> CreateInteractionResponse<'a> {
    CreateInteractionResponse::Modal(CreateModal::new(custom_id, title).components(components))
}

/*
認証済みロールを持っているかどうかを確認します。
*/
pub async fn has_authed_role(ctx: AppContext<'_>) -> Result<bool, AppError> {
    let Some(member) = ctx.author_member().await else {
        return Ok(false);
    };

    let config = ctx.app_config().await;
    if !member.roles.contains(&config.auth.role_id) {
        Err(BotError::HasNoRole.into())
    } else {
        Ok(true)
    }
}

/*
実行した場所がパブリックスレッドであるかどうかを確認します。
*/
pub async fn is_in_public_thread(ctx: AppContext<'_>) -> Result<bool, AppError> {
    let thread = ctx
        .channel()
        .await
        .and_then(|t| t.thread())
        .ok_or(BotError::IsNotInThread)?;
    match thread.base.kind {
        ChannelType::PublicThread | ChannelType::NewsThread => Ok(true),
        ChannelType::PrivateThread => Err(BotError::IsPrivateThread.into()),
        _ => Err(BotError::IsNotInThread.into()),
    }
}

const UNITS: [(u64, &str); 4] = [(86400, "日"), (3600, "時間"), (60, "分"), (1, "秒")];

pub fn format_duration(duration: Duration, mut count: usize) -> String {
    let mut remaining = duration.as_secs();
    let mut parts = Vec::new();

    for (unit, label) in UNITS {
        if remaining >= unit && count > 0 {
            let value = remaining / unit;
            if value > 0 {
                parts.push(format!("{value}{label}"));
                remaining %= unit;
                count -= 1;
            }
        }
    }

    parts.join(" ")
}

pub async fn send_message<'a>(ctx: &Context, channel_id: &ChannelId, builder: CreateMessage<'a>) -> Result<Message> {
    match channel_id.widen().send_message(ctx.http(), builder).await {
        Ok(m) => Ok(m),
        Err(why) => {
            error!("Error sending message: {:#?}", why);
            Err(why)
        }
    }
}

/**
指定されたチャンネルのアーカイブされたパブリックスレッドをすべて取得します。
 */
pub async fn fetch_all_archived_public_thread(
    ctx: &Context,
    channel_id: ChannelId,
    max_retries: Option<usize>,
) -> impl Stream<Item = GuildThread> {
    let max_retries = max_retries.unwrap_or(5);
    Box::pin(stream! {
        let mut retries_left = max_retries;
        let mut before = None;
        loop {
            let thread_data = match ctx.http.get_channel_archived_public_threads(channel_id, before, Some(100)).await {
                Ok(data) => data,
                Err(_) => {
                    if retries_left == 0 {
                        break;
                    } else {
                        retries_left -= 1;
                        continue;
                    }
                }
            };

            before = thread_data.threads
                .last()
                .and_then(|last| last.thread_metadata.archive_timestamp);

            for channel in thread_data.threads {
                yield channel;
            }

            if !thread_data.has_more || before.is_none() {
                break;
            }
        }
    })
}

pub fn create_diff_lines_text(old: &str, new: &str) -> String {
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_lines(old, new);
    diff.iter_all_changes()
        .map(|c| match c.tag() {
            ChangeTag::Delete => format!("- {c}"),
            ChangeTag::Insert => format!("+ {c}"),
            ChangeTag::Equal => format!("  {c}"),
        })
        .join("")
}

pub fn get_guild_members(ctx: &Context, guild_id: GuildId) -> impl Iterator<Item = Member> {
    guild_id
        .to_guild_cached(&ctx.cache)
        .map(|guild| guild.members.clone())
        .into_iter()
        .flatten()
}
