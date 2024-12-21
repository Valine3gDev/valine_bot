use poise::say_reply;
use serenity::all::Mentionable;
use serenity::futures::{self, Stream, StreamExt};
use tracing::error;

use crate::config::get_config;
use crate::features::PError;
use crate::utils::{create_message, send_message};

use super::PContext;

async fn autocomplete_keyword<'a>(ctx: PContext<'_>, partial: &'a str) -> impl Stream<Item = String> + 'a {
    let config = &get_config(ctx.serenity_context()).await.auth;
    futures::stream::iter(config.dummy_keywords.clone())
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

/// 合言葉を入力してください。
#[poise::command(
    slash_command,
    ephemeral,
    guild_only,
    aliases("合言葉"),
    member_cooldown = 60,
    required_bot_permissions = "MANAGE_ROLES"
)]
pub async fn keyword(
    ctx: PContext<'_>,
    #[autocomplete = "autocomplete_keyword"]
    #[description = "合言葉"]
    keyword: String,
) -> Result<(), PError> {
    let config = &get_config(ctx.serenity_context()).await.auth;

    if !config.trigger_regex.is_match(&keyword) {
        say_reply(ctx, "合言葉が間違っています。").await?;
        return Ok(());
    }

    let member = ctx.author_member().await.unwrap();

    if member.roles.contains(&config.role_id) {
        error!("{} already has the role", member.user.name);
        say_reply(ctx, "すでにロールを持っています。").await?;
        return Ok(());
    }

    if let Err(why) = member.add_role(&ctx.http(), config.role_id).await {
        let log = create_message(format!(
            "{} にロールを追加できませんでした。\n```\n{}```",
            member.mention(),
            why
        ));
        let _ = send_message(ctx.serenity_context(), &config.log_channel_id, log).await;
        error!("Failed to add role: {:?}", why);
        return Ok(());
    }

    let log = create_message(format!("{} にロールを追加しました。", member.mention()));
    let _ = send_message(ctx.serenity_context(), &config.log_channel_id, log).await;
    say_reply(
        ctx,
        "合言葉を確認しました。\nチャンネルが表示されない場合、アプリの再起動や再読み込み(Ctrl + R)をお試しください。",
    )
    .await?;
    Ok(())
}
