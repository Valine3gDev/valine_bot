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
use tracing::error;

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

async fn wait_shutdown_signal() -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigterm = signal(SignalKind::terminate())?;

        tokio::select! {
            result = tokio::signal::ctrl_c() => result,
            _ = sigterm.recv() => Ok(()),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await
    }
}

pub fn install_signal_handler(client: &Client) {
    let shutdown = client.shard_manager.get_shutdown_trigger();

    tokio::spawn(async move {
        if let Err(error) = wait_shutdown_signal().await {
            error!("Could not register shutdown signal handler: {error}");
            return;
        }

        shutdown();
    });
}
