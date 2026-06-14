use serenity::{all::prelude::Context, async_trait, http::RatelimitInfo, model::event::FullEvent};
use tracing::error;

use crate::core::BoxError;

#[async_trait]
pub trait BotEventHandler: Send + Sync {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) -> Result<(), BoxError>;

    #[allow(unused_variables)]
    async fn ratelimit(&self, data: &RatelimitInfo) {}
}

#[async_trait]
pub trait BotEventErrorHandler: Send + Sync {
    async fn handle(&self, ctx: &Context, event: &FullEvent, error: &BoxError);
}

pub struct BotEventHandlers {
    handlers: Vec<Box<dyn BotEventHandler>>,
    error_handler: Option<Box<dyn BotEventErrorHandler>>,
}

impl BotEventHandlers {
    pub fn new() -> Self {
        Self {
            handlers: vec![],
            error_handler: None,
        }
    }

    pub async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        for handler in &self.handlers {
            if let Err(error) = handler.dispatch(ctx, event).await {
                match &self.error_handler {
                    Some(error_handler) => error_handler.handle(ctx, event, &error).await,
                    None => error!("Unhandled event handler error: {error:#?}"),
                }
            }
        }
    }

    pub async fn ratelimit(&self, data: &RatelimitInfo) {
        for handler in &self.handlers {
            handler.ratelimit(data).await
        }
    }

    pub fn add<B: BotEventHandler + 'static>(mut self, handler: B) -> Self {
        self.handlers.push(Box::new(handler));
        self
    }

    pub fn on_error<E: BotEventErrorHandler + 'static>(mut self, error_handler: E) -> Self {
        self.error_handler = Some(Box::new(error_handler));
        self
    }
}
