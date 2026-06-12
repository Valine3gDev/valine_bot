use std::sync::Arc;

use serenity::all::prelude::Context;

use crate::app::{AppContext, config::AppConfig};

pub struct BotData {
    pub config: Arc<AppConfig>,
}

pub trait BotDataGetter {
    fn get_bot_data(&self) -> Arc<BotData>;

    fn get_app_config(&self) -> Arc<AppConfig>;
}

impl BotDataGetter for Context {
    fn get_bot_data(&self) -> Arc<BotData> {
        self.data::<BotData>()
    }

    fn get_app_config(&self) -> Arc<AppConfig> {
        Arc::clone(&self.get_bot_data().config)
    }
}

impl<'a> BotDataGetter for AppContext<'a> {
    fn get_bot_data(&self) -> Arc<BotData> {
        self.data()
    }

    fn get_app_config(&self) -> Arc<AppConfig> {
        Arc::clone(&self.data().config)
    }
}
