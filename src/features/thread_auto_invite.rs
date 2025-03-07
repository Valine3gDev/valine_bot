use poise::{ApplicationContext, say_reply};
use serenity::{
    all::{ChannelId, ChannelType, Context, EditMessage, EventHandler, GuildChannel, Mentionable, RoleId},
    async_trait,
};
use tracing::error;

use crate::{
    CommandData, PError,
    config::get_config,
    utils::{await_initial_message, create_message, has_authed_role, is_in_public_thread},
};

async fn invite_thread_by_role(ctx: &Context, thread_id: ChannelId, role_id: RoleId) {
    let mut message = {
        let msg = create_message("スレッド自動招待用メッセージ");
        match thread_id.send_message(&ctx, msg).await {
            Ok(m) => m,
            Err(why) => return error!("Error sending message: {:?}", why),
        }
    };

    let _ = message
        .edit(&ctx, EditMessage::new().content(role_id.mention().to_string()))
        .await;

    let _ = message.delete(&ctx).await;
}

pub struct Handler;

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
        invite_thread_by_role(&ctx, thread.id, config.role_id).await;
    }
}

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
    invite_thread_by_role(ctx.serenity_context(), ctx.channel_id(), config.role_id).await;
    say_reply(ctx.into(), "スレッドに招待しました。").await?;
    Ok(())
}
