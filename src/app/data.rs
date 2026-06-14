use std::sync::Arc;

use serenity::all::prelude::Context;
use tokio::sync::RwLock;

use crate::{
    app::{AppContext, config::AppConfig},
    core::MessageCache,
};

pub struct BotData {
    config: RwLock<Arc<AppConfig>>,
    message_cache: Arc<MessageCache>,
}

impl BotData {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
            message_cache: Arc::new(MessageCache::new()),
        }
    }
}

pub trait BotDataGetter {
    fn bot_data(&self) -> Arc<BotData>;

    async fn read_app_config(&self) -> Arc<AppConfig> {
        self.bot_data().config.read().await.clone()
    }

    async fn replace_app_config(&self, config: AppConfig) {
        let data = self.bot_data();
        *data.config.write().await = Arc::new(config);
    }

    fn message_cache(&self) -> Arc<MessageCache> {
        Arc::clone(&self.bot_data().message_cache)
    }
}

impl BotDataGetter for Context {
    fn bot_data(&self) -> Arc<BotData> {
        self.data()
    }
}

impl<'a> BotDataGetter for AppContext<'a> {
    fn bot_data(&self) -> Arc<BotData> {
        self.data()
    }
}
