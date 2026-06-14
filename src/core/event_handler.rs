use serenity::{all::prelude::Context, async_trait, http::RatelimitInfo, model::event::FullEvent};

#[async_trait]
pub trait BotEventHandler: Send + Sync {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent);

    #[allow(unused_variables)]
    async fn ratelimit(&self, data: &RatelimitInfo) {}
}

pub struct BotEventHandlers(Vec<Box<dyn BotEventHandler>>);

impl BotEventHandlers {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        for handler in &self.0 {
            handler.dispatch(ctx, event).await
        }
    }

    pub async fn ratelimit(&self, data: &RatelimitInfo) {
        for handler in &self.0 {
            handler.ratelimit(data).await
        }
    }

    pub fn add<B: BotEventHandler + 'static>(mut self, handler: B) -> Self {
        self.0.push(Box::new(handler));
        self
    }
}
