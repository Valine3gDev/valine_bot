use std::time::Duration;

use futures::lock::Mutex;
use serenity::{
    all::prelude::Context,
    async_trait,
    gateway::{ActivityData, ChunkGuildFilter},
    http::RatelimitInfo,
    model::{event::FullEvent, gateway::Ready, guild::Guild},
};
use sysinfo::{Pid, System};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::core::BotEventHandler;

pub struct MainEventHandler {
    activity_task: Mutex<Option<JoinHandle<()>>>,
}

impl MainEventHandler {
    pub fn new() -> Self {
        Self {
            activity_task: Mutex::new(None),
        }
    }

    async fn handle_ready(&self, ready: &Ready) {
        info!("{} is connected!", ready.user.name);
    }

    async fn handle_cache_ready(&self, ctx: &Context) {
        let mut task = self.activity_task.lock().await;

        if let Some(handle) = task.take() {
            handle.abort();
        }

        let ctx = ctx.clone();

        *task = Some(tokio::spawn(async move {
            let mut system = System::new_all();
            let pid = Pid::from_u32(std::process::id());

            loop {
                system.refresh_all();

                if let Some(memory) = system.process(pid).map(|p| p.memory() as f64 / 1024.0 / 1024.0) {
                    ctx.set_activity(Some(ActivityData::custom(format!("メモリ使用量: {:.1}MB", memory))));
                } else {
                    error!("Failed to get process info");
                }

                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        }));
    }

    async fn handle_guild_create(&self, ctx: &Context, guild: &Guild) {
        // 全てのメンバーを取得する。結果は Serenity によって自動でキャッシュされる。
        ctx.chunk_guild(guild.id, Some(0), false, ChunkGuildFilter::None, None);
    }
}

#[async_trait]
impl BotEventHandler for MainEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        if let FullEvent::CacheReady { .. } = event {
            self.handle_cache_ready(ctx).await;
        }

        match event {
            FullEvent::CacheReady { .. } => self.handle_cache_ready(ctx).await,
            FullEvent::Ready { data_about_bot, .. } => self.handle_ready(data_about_bot).await,
            FullEvent::GuildCreate { guild, .. } => self.handle_guild_create(ctx, guild).await,
            _ => {}
        }
    }

    async fn ratelimit(&self, data: &RatelimitInfo) {
        warn!(
            "Ratelimited {} {}: {}s",
            data.method.reqwest_method(),
            data.path,
            data.timeout.as_secs()
        );
    }
}
