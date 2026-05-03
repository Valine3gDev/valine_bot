use itertools::Itertools;
use serenity::{
    all::{
        ChannelId, ChannelType, Context, EditMessage, EventHandler, GuildChannel, GuildId, GuildMemberUpdateEvent,
        Member, Mentionable, RoleId,
    },
    async_trait,
};
use tracing::{error, info};

use crate::{
    config::{ThreadAutoInviteConfig, get_config},
    utils::{await_initial_message, create_message},
};

pub struct Handler;

impl Handler {
    pub fn new() -> Self {
        Self
    }

    async fn find_role(ctx: &Context, guild_id: GuildId, config: &ThreadAutoInviteConfig) -> Option<RoleId> {
        let role_member_counts = ctx
            .http
            .get_guild_role_member_counts(guild_id)
            .await
            .map_err(|e| error!("Failed to get guild role member counts: {}", e))
            .ok()?;

        config.role_ids.iter().find_map(|&role_id| {
            let count = *role_member_counts.get(&role_id)?;
            (count < config.min_member_count).then_some(role_id)
        })
    }

    pub(super) async fn handle_role_assignment(ctx: &Context, new: &Member, config: &ThreadAutoInviteConfig) {
        let Some(role) = Self::find_role(ctx, new.guild_id, config).await else {
            error!("No role found with count less than {}", config.min_member_count);
            return;
        };

        if let Err(e) = new.add_role(ctx, role).await {
            error!("Failed to add role {} to member {}: {}", role, new.user.id, e);
        } else {
            info!("Added role {} to member {}", role, new.user.id);
        }
    }

    pub(super) async fn handle_role_removal(ctx: &Context, old: &Member, config: &ThreadAutoInviteConfig) {
        let roles = old
            .roles
            .iter()
            .filter(|r| config.role_ids.contains(r))
            .collect::<Vec<_>>();

        for role_id in roles {
            if let Err(e) = old.remove_role(&ctx, role_id).await {
                error!("Failed to remove role {} from member {}: {}", role_id, old.user.id, e);
            } else {
                info!("Removed role {} from member {}", role_id, old.user.id);
                break;
            }
        }
    }
}

pub async fn invite_thread_by_roles(ctx: &Context, thread_id: ChannelId, role_ids: &[RoleId]) {
    let mut message = {
        let msg = create_message("スレッド自動招待用メッセージ");
        match thread_id.send_message(&ctx, msg).await {
            Ok(m) => m,
            Err(why) => return error!("Error sending message: {:?}", why),
        }
    };

    let content = role_ids.iter().map(|r| r.mention().to_string()).join(" ");
    let _ = message.edit(&ctx, EditMessage::new().content(content)).await;

    let _ = message.delete(&ctx).await;
}

#[async_trait]
impl EventHandler for Handler {
    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        if thread.kind == ChannelType::PrivateThread {
            return;
        }

        if await_initial_message(&ctx, &thread).await {
            return;
        }

        let config = &get_config(&ctx).await.thread_auto_invite;
        invite_thread_by_roles(&ctx, thread.id, &config.role_ids).await;
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        old: Option<Member>,
        new: Option<Member>,
        _: GuildMemberUpdateEvent,
    ) {
        let Some(new) = new else {
            error!("Member update event with no new member");
            return;
        };
        let Some(old) = old else {
            error!("Member update event with no old member");
            return;
        };

        let config = &get_config(&ctx).await.thread_auto_invite;

        let has_new_role = new.roles.contains(&config.display_role_id);
        let has_old_role = old.roles.contains(&config.display_role_id);

        if has_new_role && !has_old_role {
            Handler::handle_role_assignment(&ctx, &new, config).await;
        } else if has_old_role && !has_new_role {
            Handler::handle_role_removal(&ctx, &old, config).await;
        }
    }
}
