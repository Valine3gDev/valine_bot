use itertools::Itertools;
use serenity::{
    all::{ChannelType, Context, EditMessage, GuildId, Member, Mentionable, RoleId, prelude::CacheHttp},
    model::{channel::GuildThread, event::FullEvent, id::GenericChannelId},
};
use tracing::{error, info};
use valine_bot_macros::event_handler;

use crate::{
    app::{BotDataExt, config::ThreadAutoInviteConfig},
    utils::create_message,
};

async fn find_role(ctx: &Context, guild_id: GuildId, config: &ThreadAutoInviteConfig) -> Option<RoleId> {
    let role_member_counts = ctx
        .http
        .get_guild_role_member_counts(guild_id)
        .await
        .map_err(|e| error!("Failed to get guild role member counts: {e:#?}"))
        .ok()?;

    config.role_ids.iter().find_map(|&role_id| {
        let count = *role_member_counts.get(&role_id)?;
        (count < config.min_member_count).then_some(role_id)
    })
}

pub(in crate::features::thread_auto_invite) async fn assign_role(
    ctx: &Context,
    new: &Member,
    config: &ThreadAutoInviteConfig,
) {
    let Some(role) = find_role(ctx, new.guild_id, config).await else {
        error!("No role found with count less than {}", config.min_member_count);
        return;
    };

    if let Err(e) = new.add_role(ctx.http(), role, None).await {
        error!("Failed to add role {role} to member {}: {e:#?}", new.user.id);
    } else {
        info!("Added role {role} to member {}", new.user.id);
    }
}

pub(in crate::features::thread_auto_invite) async fn remove_role(
    ctx: &Context,
    old: &Member,
    config: &ThreadAutoInviteConfig,
) {
    let roles = old
        .roles
        .iter()
        .filter(|r| config.role_ids.contains(r))
        .collect::<Vec<_>>();

    for role_id in roles {
        if let Err(e) = old.remove_role(ctx.http(), *role_id, None).await {
            error!("Failed to remove role {role_id} from member {}: {e:#?}", old.user.id);
        } else {
            info!("Removed role {role_id} from member {}", old.user.id);
            break;
        }
    }
}

pub(in crate::features::thread_auto_invite) async fn invite_thread_by_roles(
    ctx: &Context,
    thread_id: GenericChannelId,
    role_ids: &[RoleId],
) {
    let mut message = {
        let msg = create_message("スレッド自動招待用メッセージ");
        match thread_id.send_message(ctx.http(), msg).await {
            Ok(m) => m,
            Err(why) => return error!("Error sending message: {why:#?}"),
        }
    };

    let content = role_ids.iter().map(|r| r.mention().to_string()).join(" ");
    let _ = message.edit(&ctx, EditMessage::new().content(content)).await;

    let _ = message.delete(ctx.http(), None).await;
}

async fn handle_thread_create(ctx: &Context, thread: &GuildThread, newly_created: &Option<bool>) {
    if !newly_created.unwrap_or(false) {
        return;
    }

    if thread.base.kind == ChannelType::PrivateThread {
        return;
    }

    let config = &ctx.app_config().await.thread_auto_invite;
    invite_thread_by_roles(ctx, thread.id.widen(), &config.role_ids).await;
}

async fn handle_guild_member_update(ctx: &Context, old: &Option<Member>, new: &Option<Member>) {
    let Some(new) = new else {
        error!("Member update event with no new member");
        return;
    };
    let Some(old) = old else {
        error!("Member update event with no old member");
        return;
    };

    let config = &ctx.app_config().await.thread_auto_invite;

    let has_new_role = new.roles.contains(&config.display_role_id);
    let has_old_role = old.roles.contains(&config.display_role_id);

    if has_new_role && !has_old_role {
        assign_role(ctx, new, config).await;
    } else if has_old_role && !has_new_role {
        remove_role(ctx, old, config).await;
    }
}

#[event_handler]
pub async fn handle_thread_auto_invite_event(ctx: &Context, event: &FullEvent) {
    match event {
        FullEvent::ThreadCreate {
            thread, newly_created, ..
        } => handle_thread_create(ctx, thread, newly_created).await,

        FullEvent::GuildMemberUpdate {
            old_if_available, new, ..
        } => handle_guild_member_update(ctx, old_if_available, new).await,

        _ => {}
    }
}
