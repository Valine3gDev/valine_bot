use std::sync::Arc;

use serenity::all::prelude::Context;

use crate::config::Config;

pub struct BotData {
    pub config: Arc<Config>,
}

pub trait BotDataGetter {
    fn get_bot_data(&self) -> Arc<BotData>;
}

impl BotDataGetter for Context {
    fn get_bot_data(&self) -> Arc<BotData> {
        self.data::<BotData>()
    }
}
