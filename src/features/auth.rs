use poise::{say_reply, FrameworkError};
use serenity::all::Mentionable;
use tracing::error;

use crate::config::get_config;
use crate::features::PError;
use crate::on_error;
use crate::utils::{create_message, send_message};

use super::PContext;

async fn keyword_on_error(error: FrameworkError<'_, (), PError>) {
    match error {
        FrameworkError::Command { ctx, error, .. } => {
            let _ = say_reply(ctx, "合言葉の確認中にエラーが発生しました。").await;
            error!("Command error: {}", error);
        }
        error => {
            let _ = on_error(error).await;
        }
    }
}

/// 合言葉を入力してください。
#[poise::command(
    slash_command,
    ephemeral,
    guild_only,
    aliases("合言葉"),
    on_error = "keyword_on_error",
    required_bot_permissions = "MANAGE_ROLES"
)]
pub async fn keyword(ctx: PContext<'_>, #[description = "合言葉"] keyword: String) -> Result<(), PError> {
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
