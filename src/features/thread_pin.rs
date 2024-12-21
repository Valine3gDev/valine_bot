use std::time::Duration;

use poise::say_reply;
use serenity::{
    all::{Message, MessageType},
    futures::StreamExt,
};

use crate::utils::has_authed_role;
use crate::{config::get_config, PContext, PError};

/// スレッド主限定でメッセージをピン留めします。
#[poise::command(
    context_menu_command = "ピン留め",
    slash_command,
    ephemeral,
    guild_only,
    aliases("ピン留め"),
    required_bot_permissions = "MANAGE_MESSAGES",
    check = "has_authed_role"
)]
pub async fn pin(
    ctx: PContext<'_>,
    #[description = "ピン留めするメッセージ (リンクかID)"] msg: Message,
) -> Result<(), PError> {
    let channel = ctx.guild_channel().await.unwrap();
    let Some(owner) = channel.owner_id else {
        say_reply(ctx, "スレッド以外のチャンネルでは使用出来ません。").await?;
        return Ok(());
    };

    if ctx.author().id != owner {
        say_reply(ctx, "スレッド主のみがピン留めできます。").await?;
        return Ok(());
    }

    let bot_id = get_config(ctx.serenity_context()).await.bot.application_id;
    let mut stream = channel
        .await_reply(&ctx.serenity_context().shard)
        .timeout(Duration::from_secs(5))
        .channel_id(channel.id)
        .author_id(bot_id)
        .filter(|r| r.kind == MessageType::PinsAdd)
        .stream();

    if msg.pinned {
        msg.unpin(ctx).await?;
        say_reply(ctx, "ピン留めを解除しました。").await?;
    } else {
        msg.pin(ctx).await?;
        say_reply(ctx, "ピン留めしました。").await?;
    }

    if let Some(msg) = stream.next().await {
        let _ = msg.delete(ctx.http()).await;
    }

    Ok(())
}
