use std::sync::Arc;

use serenity::{
    Client,
    all::prelude::{Context, EventHandler, GatewayIntents},
    async_trait,
    gateway::client::ClientBuilder,
    http::RatelimitInfo,
    model::event::FullEvent,
    secrets::Token,
};

use crate::core::event_handler::BotEventHandlers;

struct ClientEventHandler(BotEventHandlers);

#[async_trait]
impl EventHandler for ClientEventHandler {
    async fn dispatch(&self, ctx: &Context, event: &FullEvent) {
        self.0.dispatch(ctx, event).await;
    }

    async fn ratelimit(&self, data: RatelimitInfo) {
        self.0.ratelimit(&data).await;
    }
}

pub fn create_client(
    token: impl Into<Token>,
    intents: GatewayIntents,
    event_handlers: BotEventHandlers,
) -> ClientBuilder {
    Client::builder(token.into(), intents).event_handler(Arc::new(ClientEventHandler(event_handlers)))
}
