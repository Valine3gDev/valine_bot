use serenity::{
    all::{Context, EventHandler, GuildChannel},
    async_trait,
};
use tracing::error;

use crate::{
    config::get_config,
    utils::{await_initial_message, create_message, send_message},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        if await_initial_message(&ctx, &thread).await {
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

            let log = create_message(&thread_config.startup_message);
            let _ = send_message(&ctx, &thread.id, log).await;
        }
    }
}
