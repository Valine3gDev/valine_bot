use std::time::Duration;

use futures::StreamExt;
use poise::say_reply;
use serenity::all::{GuildChannel, Message, MessageType};

use crate::config::Config;
use crate::utils::has_authed_role;
use crate::{config::get_config, PContext, PError};

async fn check_owner(ctx: PContext<'_>, config: &Config, channel: &GuildChannel) -> bool {
    let author_id = ctx.author().id;

    if channel.owner_id == Some(author_id) {
        return true;
    }

    // コンフィグで設定されたオーナーかどうか
    if config.pin.channels.get(&channel.id) == Some(&author_id) {
        return true;
    }

    // 質問フォーラムの場合、初期メッセージのメンションからスレッド主を取得
    // スレッドの初期メッセージのIDはスレッドのIDと同じ
    if channel.parent_id == Some(config.question.forum_id) {
        let Ok(msg) = channel.message(ctx, channel.id.get()).await else {
            // メッセージが取得できない場合はスレッドオーナーではない判定
            return false;
        };

        if msg.mentions.iter().any(|m| m.id == author_id) {
            return true;
        }
    }

    false
}

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
    let config = get_config(ctx.serenity_context()).await;
    let channel = ctx.guild_channel().await.unwrap();

    if !check_owner(ctx, &config, &channel).await {
        say_reply(ctx, "あなたはこのチャンネルでピン留めできません。").await?;
        return Ok(());
    }

    // ストリームを取得することでイベントの受信を開始させる
    let mut stream = channel
        .await_reply(&ctx.serenity_context().shard)
        .timeout(Duration::from_secs(5))
        .channel_id(msg.channel_id)
        .author_id(ctx.serenity_context().cache.current_user().id)
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
