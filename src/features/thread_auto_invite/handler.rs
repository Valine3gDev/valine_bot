use itertools::Itertools;
use serenity::{
    all::{
        ChannelId, ChannelType, Context, EditMessage, EventHandler, GuildChannel, GuildId, GuildMemberUpdateEvent,
        Member, Mentionable, RoleId, User,
    },
    async_trait,
};
use tracing::{error, info};

use crate::{
    config::{ThreadAutoInviteConfig, get_config},
    utils::{await_initial_message, create_message},
};

use super::{RoleCountCache, RoleCountCacheType, member_cache::MemberCache, role_count_cache::find_role};

pub struct Handler;

impl Handler {
    pub fn new() -> Self {
        Self
    }

    pub(super) async fn role_added(ctx: &Context, new: &Member, config: &ThreadAutoInviteConfig) {
        let Some(role) = find_role(ctx, new.guild_id, config).await else {
            error!("No role found with count less than {}", config.min_member_count);
            return;
        };

        let result = new
            .add_role(ctx, role)
            .await
            .map_err(|_| error!("Failed to add role {} to member {}", role, new.user.id))
            .is_ok();

        if result {
            RoleCountCache::increment_count(ctx, role).await;
        }
    }

    pub(super) async fn role_removed(ctx: &Context, old: &Member, config: &ThreadAutoInviteConfig) {
        let roles = old
            .roles
            .iter()
            .filter(|r| config.role_ids.contains(r))
            .collect::<Vec<_>>();

        let mut data = ctx.data.write().await;
        let cache = data.get_mut::<RoleCountCacheType>().unwrap();

        for role_id in roles {
            let result = old
                .remove_role(&ctx, role_id)
                .await
                .map_err(|_| error!("Failed to remove role {} from member {}", role_id, old.user.id))
                .is_ok();
            if result {
                cache.decrement(*role_id);
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

    async fn guild_member_addition(&self, ctx: Context, member: Member) {
        MemberCache::insert_member(&ctx, &member).await;
    }

    async fn guild_member_removal(&self, ctx: Context, guild_id: GuildId, user: User, _: Option<Member>) {
        MemberCache::remove_member(&ctx, guild_id, user.id).await;
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        _: Option<Member>,
        new: Option<Member>,
        event: GuildMemberUpdateEvent,
    ) {
        let Some(new) = new else {
            error!("Member update event with no new member");
            return;
        };

        let Some(old) = MemberCache::get_member(&ctx, event.guild_id, new.user.id).await else {
            error!("Member update event with no old member");
            return;
        };

        MemberCache::insert_member(&ctx, &new).await;

        let config = &get_config(&ctx).await.thread_auto_invite;

        let has_new_role = new.roles.contains(&config.display_role_id);
        let has_old_role = old.roles.contains(&config.display_role_id);

        if has_new_role && !has_old_role {
            Handler::role_added(&ctx, &new, config).await;
            info!("added role to {}", new.user.name);
        } else if has_old_role && !has_new_role {
            Handler::role_removed(&ctx, &old, config).await;
            info!("removed role from {}", new.user.name);
        }
    }
}
