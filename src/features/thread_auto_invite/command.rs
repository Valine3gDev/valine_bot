use poise::say_reply;

use crate::{
    app::{AppContext, AppError, BotDataExt},
    features::thread_auto_invite::handler::{handle_role_assignment, handle_role_removal, invite_thread_by_roles},
    utils::{get_guild_members, has_authed_role, is_in_public_thread},
};

/// 招待用ロールを持ったメンバーを実行したスレッドに招待します。
#[poise::command(
    slash_command,
    ephemeral,
    guild_only,
    aliases("スレッドに招待"),
    channel_cooldown = 86400, // 24 時間
    check = "has_authed_role",
    check = "is_in_public_thread"
)]
pub async fn invite_thread(ctx: AppContext<'_>) -> Result<(), AppError> {
    let config = &ctx.app_config().await.thread_auto_invite;
    ctx.defer_ephemeral().await?;
    invite_thread_by_roles(ctx.serenity_context(), ctx.channel_id(), &config.role_ids).await;
    say_reply(ctx, "スレッドに招待しました。").await?;
    Ok(())
}

/// 表示用のロールを持ったメンバーに呼び出し用のロールを付与します
#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_ROLES")]
pub async fn add_invite_role(ctx: AppContext<'_>) -> Result<(), AppError> {
    let members = get_guild_members(ctx.serenity_context(), ctx.guild_id().unwrap());

    let config = &ctx.app_config().await.thread_auto_invite;

    ctx.defer().await?;

    let mut added_count = 0;

    for member in members {
        if config.role_ids.iter().any(|r| member.roles.contains(r)) {
            handle_role_removal(ctx.serenity_context(), &member, config).await;
        }

        if member.roles.contains(&config.display_role_id) {
            handle_role_assignment(ctx.serenity_context(), &member, config).await;
            added_count += 1;
            continue;
        }
    }

    say_reply(ctx, format!("{added_count} 人に招待用ロールを付与しました。")).await?;
    Ok(())
}

/// 表示用のロールを持ったメンバーに呼び出し用のロールを削除
#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_ROLES")]
pub async fn remove_invite_role(ctx: AppContext<'_>) -> Result<(), AppError> {
    let members = get_guild_members(ctx.serenity_context(), ctx.guild_id().unwrap());

    let config = &ctx.app_config().await.thread_auto_invite;

    ctx.defer().await?;

    let mut role_count = 0;

    for member in members {
        if !config.role_ids.iter().any(|r| member.roles.contains(r)) {
            continue;
        }

        handle_role_removal(ctx.serenity_context(), &member, config).await;
        role_count += 1;
    }

    say_reply(ctx, format!("{role_count} 人から招待用ロールを全て削除しました。")).await?;
    Ok(())
}
