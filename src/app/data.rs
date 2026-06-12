use std::sync::Arc;

use serenity::all::prelude::Context;
use tokio::sync::RwLock;

use crate::app::{AppContext, config::AppConfig};

pub struct BotData {
    config: RwLock<Arc<AppConfig>>,
}

impl BotData {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config: RwLock::new(Arc::new(config)),
        }
    }
}

pub trait BotDataGetter {
    fn get_bot_data(&self) -> Arc<BotData>;

    async fn read_app_config(&self) -> Arc<AppConfig> {
        self.get_bot_data().config.read().await.clone()
    }

    async fn replace_app_config(&self, config: AppConfig) {
        let data = self.get_bot_data();
        *data.config.write().await = Arc::new(config);
    }
}

impl BotDataGetter for Context {
    fn get_bot_data(&self) -> Arc<BotData> {
        self.data()
    }
}

impl<'a> BotDataGetter for AppContext<'a> {
    fn get_bot_data(&self) -> Arc<BotData> {
        self.data()
    }
}
