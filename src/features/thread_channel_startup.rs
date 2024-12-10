use serenity::{
    all::{Context, EventHandler, GuildChannel},
    async_trait,
};
use tracing::error;

use crate::{
    config::get_config,
    utils::{create_message, send_message},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        // スレッドの作成時とスレッドの初期メッセージ送信後にイベントが発火するので、スレッド作成時は無視する
        if thread.last_message_id.is_none() {
            return;
        }

        let config = &get_config(&ctx).await.thread_channel_startup;
        let Some(parent_id) = thread.parent_id else {
            return error!("Failed to get parent id: {:?}", thread);
        };
        for thread_config in &config.threads {
            if parent_id != thread_config.channel_id {
                continue;
            }

            let log = create_message(thread_config.startup_message.clone());
            let _ = send_message(&ctx, &thread.id, log).await;
        }
    }
}
