use serenity::{
    all::{Context, EditMessage, EventHandler, GuildChannel, Mentionable},
    async_trait,
};
use tracing::error;

use crate::{
    config::get_config,
    utils::{await_initial_message, create_message},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        if await_initial_message(&ctx, &thread).await {
            return;
        }

        let mut message = {
            let msg = create_message("スレッド自動招待用メッセージ");
            match thread.send_message(&ctx, msg).await {
                Ok(m) => m,
                Err(why) => return error!("Error sending message: {:?}", why),
            }
        };

        let config = &get_config(&ctx).await.thread_auto_invite;
        let _ = message
            .edit(&ctx, EditMessage::new().content(config.role_id.mention().to_string()))
            .await;

        let _ = message.delete(&ctx).await;
    }
}
