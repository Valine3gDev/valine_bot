use anyhow::Context as _;
use itertools::Itertools;
use serenity::{
    all::{ChannelType, Context, EditMessage, GuildId, Member, Mentionable, RoleId, prelude::CacheHttp},
    model::{channel::GuildThread, event::FullEvent, id::GenericChannelId},
};
use tracing::info;
use valine_bot_macros::event_handler;

use crate::{
    app::{AppError, BotDataExt, BotError, config::ThreadAutoInviteConfig},
    utils::create_message,
};

async fn find_role(ctx: &Context, guild_id: GuildId, config: &ThreadAutoInviteConfig) -> Result<RoleId, AppError> {
    let role_member_counts = ctx
        .http
        .get_guild_role_member_counts(guild_id)
        .await
        .context("Failed to get invite role member counts")?;

    config
        .role_ids
        .iter()
        .find(|role_id| {
            role_member_counts
                .get(role_id)
                .is_some_and(|&count| count < config.min_member_count)
        })
        .copied()
        .ok_or_else(|| {
            BotError::NoAvailableInviteRole {
                member_limit: config.min_member_count,
            }
            .into()
        })
}

pub(in crate::features::thread_auto_invite) async fn assign_role(
    ctx: &Context,
    new: &Member,
    config: &ThreadAutoInviteConfig,
) -> Result<(), AppError> {
    let role = find_role(ctx, new.guild_id, config).await?;

    new.add_role(ctx.http(), role, None)
        .await
        .context("Failed to assign invite role")?;
    info!("Added role {role} to member {}", new.user.id);
    Ok(())
}

pub(in crate::features::thread_auto_invite) async fn remove_role(
    ctx: &Context,
    old: &Member,
    config: &ThreadAutoInviteConfig,
) -> Result<(), AppError> {
    if let Some(&role_id) = old.roles.iter().find(|role_id| config.role_ids.contains(role_id)) {
        old.remove_role(ctx.http(), role_id, None)
            .await
            .context("Failed to remove invite role")?;
        info!("Removed role {role_id} from member {}", old.user.id);
    }

    Ok(())
}

pub(in crate::features::thread_auto_invite) async fn invite_thread_by_roles(
    ctx: &Context,
    thread_id: GenericChannelId,
    role_ids: &[RoleId],
) -> Result<(), AppError> {
    let msg = create_message("スレッド自動招待用メッセージ");
    let mut message = thread_id
        .send_message(ctx.http(), msg)
        .await
        .context("Failed to send thread invite message")?;

    let content = role_ids.iter().map(|r| r.mention().to_string()).join(" ");
    message
        .edit(&ctx, EditMessage::new().content(content))
        .await
        .context("Failed to mention invite roles")?;

    message
        .delete(ctx.http(), None)
        .await
        .context("Failed to delete thread invite message")?;
    Ok(())
}

async fn handle_thread_create(
    ctx: &Context,
    thread: &GuildThread,
    newly_created: &Option<bool>,
) -> Result<(), AppError> {
    if !newly_created.unwrap_or(false) {
        return Ok(());
    }

    if thread.base.kind == ChannelType::PrivateThread {
        return Ok(());
    }

    let config = &ctx.app_config().await.thread_auto_invite;
    invite_thread_by_roles(ctx, thread.id.widen(), &config.role_ids).await
}

async fn handle_guild_member_update(ctx: &Context, old: &Option<Member>, new: &Option<Member>) -> Result<(), AppError> {
    let new = new.as_ref().ok_or(BotError::MissingEventData("updated member"))?;
    let old = old.as_ref().ok_or(BotError::MissingEventData("previous member"))?;

    let config = &ctx.app_config().await.thread_auto_invite;

    let has_new_role = new.roles.contains(&config.display_role_id);
    let has_old_role = old.roles.contains(&config.display_role_id);

    if has_new_role && !has_old_role {
        assign_role(ctx, new, config).await?;
    } else if has_old_role && !has_new_role {
        remove_role(ctx, old, config).await?;
    }

    Ok(())
}

#[event_handler]
pub async fn handle_thread_auto_invite_event(ctx: &Context, event: &FullEvent) -> Result<(), AppError> {
    match event {
        FullEvent::ThreadCreate {
            thread, newly_created, ..
        } => handle_thread_create(ctx, thread, newly_created).await?,

        FullEvent::GuildMemberUpdate {
            old_if_available, new, ..
        } => handle_guild_member_update(ctx, old_if_available, new).await?,

        _ => {}
    }

    Ok(())
}
