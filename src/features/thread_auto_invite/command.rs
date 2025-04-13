use poise::{ApplicationContext, say_reply};

use crate::{
    CommandData, PError,
    config::get_config,
    features::thread_auto_invite::handler::invite_thread_by_roles,
    utils::{has_authed_role, is_in_public_thread},
};

use super::{handler::Handler, member_cache::MemberCache};

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
pub async fn invite_thread(ctx: ApplicationContext<'_, CommandData, PError>) -> Result<(), PError> {
    let config = &get_config(ctx.serenity_context()).await.thread_auto_invite;
    ctx.defer_ephemeral().await?;
    invite_thread_by_roles(ctx.serenity_context(), ctx.channel_id(), &config.role_ids).await;
    say_reply(ctx.into(), "スレッドに招待しました。").await?;
    Ok(())
}

/// 表示用のロールを持ったメンバーに呼び出し用のロールを付与します
#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_ROLES")]
pub async fn add_invite_role(ctx: ApplicationContext<'_, CommandData, PError>) -> Result<(), PError> {
    let members = MemberCache::get_all_members(ctx.serenity_context(), ctx.guild_id().unwrap()).await;

    let config = &get_config(ctx.serenity_context()).await.thread_auto_invite;

    ctx.defer().await?;

    let mut role_count = 0;

    for member in members {
        if config.role_ids.iter().any(|r| member.roles.contains(r)) {
            Handler::role_removed(ctx.serenity_context(), &member, config).await;
        }

        if member.roles.contains(&config.display_role_id) {
            Handler::role_added(ctx.serenity_context(), &member, config).await;
            role_count += 1;
            continue;
        }
    }

    say_reply(ctx.into(), format!("{} 人に招待用ロールを付与しました。", role_count)).await?;
    Ok(())
}

/// 表示用のロールを持ったメンバーに呼び出し用のロールを削除
#[poise::command(slash_command, guild_only, default_member_permissions = "MANAGE_ROLES")]
pub async fn remove_invite_role(ctx: ApplicationContext<'_, CommandData, PError>) -> Result<(), PError> {
    let members = MemberCache::get_all_members(ctx.serenity_context(), ctx.guild_id().unwrap()).await;

    let config = &get_config(ctx.serenity_context()).await.thread_auto_invite;

    ctx.defer().await?;

    let mut role_count = 0;

    for member in members {
        if !config.role_ids.iter().any(|r| member.roles.contains(r)) {
            continue;
        }

        Handler::role_removed(ctx.serenity_context(), &member, config).await;
        role_count += 1;
    }

    say_reply(
        ctx.into(),
        format!("{} 人から招待用ロールを全て削除しました。", role_count),
    )
    .await?;
    Ok(())
}
