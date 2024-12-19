use std::time::Duration;

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
        // Botがメッセージを送信すると二度イベントが発火するので、初期メッセージ送信後のイベントは無視する
        if thread.last_message_id.is_some() {
            return;
        }

        // 初期メッセージが送信されるか、5秒経つまで待機
        let _ = thread
            .await_reply(&ctx.shard)
            .channel_id(thread.id)
            .author_id(thread.owner_id.unwrap())
            .timeout(Duration::from_secs(5))
            .await;

        let config = &get_config(&ctx).await.thread_channel_startup;
        let Some(parent_id) = thread.parent_id else {
            return error!("Failed to get parent id: {:?}", thread);
        };

        for thread_config in &config.threads {
            if parent_id != thread_config.channel_id {
                continue;
            }

            let log = create_message(&thread_config.startup_message);
            let _ = send_message(&ctx, &thread.id, log).await;
        }
    }
}
