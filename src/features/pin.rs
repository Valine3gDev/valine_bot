use std::time::Duration;

use futures::StreamExt;
use poise::say_reply;
use serenity::{
    all::{Message, MessageType},
    collector::CollectMessages,
    model::{channel::Channel, id::MessageId},
};

use crate::{
    app::{AppContext, AppError, BotDataGetter, config::AppConfig},
    utils::has_authed_role,
};

async fn check_owner(ctx: AppContext<'_>, config: &AppConfig, channel: &Channel) -> bool {
    let author_id = ctx.author().id;

    // コンフィグで設定されたオーナーかどうか
    if config.pin.channels.get(&channel.id().expect_channel()) == Some(&author_id) {
        return true;
    }

    let Channel::GuildThread(channel) = channel else {
        return false;
    };

    if channel.owner_id == author_id {
        return true;
    }

    // 質問フォーラムの場合、初期メッセージのメンションからスレッド主を取得
    if channel.parent_id == config.question.forum_id {
        // スレッドの初期メッセージのIDはスレッドのIDと同じ
        let Ok(msg) = channel.id.widen().message(ctx, MessageId::new(channel.id.get())).await else {
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
    ctx: AppContext<'_>,
    #[description = "ピン留めするメッセージ (リンクかID)"] msg: Message,
) -> Result<(), AppError> {
    let config = ctx.read_app_config().await;
    let channel = ctx.channel().await.unwrap();

    if !check_owner(ctx, &config, &channel).await {
        say_reply(ctx, "あなたはこのチャンネルでピン留めできません。").await?;
        return Ok(());
    }

    let mut stream = channel
        .id()
        .collect_messages(&ctx.serenity_context())
        .timeout(Duration::from_secs(5))
        .channel_id(msg.channel_id)
        .author_id(ctx.serenity_context().cache.current_user().id)
        .filter(|r| r.kind == MessageType::PinsAdd)
        .stream();

    static PIN_REASON: Option<&str> = Some("/pin コマンドによる操作");
    if msg.pinned() {
        msg.unpin(&ctx.http(), PIN_REASON).await?;
        say_reply(ctx, "ピン留めを解除しました。").await?;
    } else {
        msg.pin(&ctx.http(), PIN_REASON).await?;
        say_reply(ctx, "ピン留めしました。").await?;
    }

    if let Some(msg) = stream.next().await {
        let _ = msg.delete(&ctx.http(), None).await;
    }

    Ok(())
}
