mod admin;
mod auth;
mod honeypot;
mod message_cache_handler;
mod message_logging;
mod pin;
// mod question;
// mod thread_auto_invite;

use std::borrow::Cow;

use crate::{
    app::{AppCommand, config::AppConfig},
    core::BotEventHandlers,
    features::{
        auth::{AutoKickEventHandler, KeywordAuthEventHandler},
        honeypot::handle_honeypot_event,
        message_cache_handler::MessageCacheHandler,
        message_logging::handle_message_logging_event,
    },
};

pub fn event_handlers(config: &AppConfig) -> BotEventHandlers {
    BotEventHandlers::new()
        .add(handle_honeypot_event)
        .add(handle_message_logging_event)
        .add(KeywordAuthEventHandler::new())
        .add(AutoKickEventHandler::new())
        .add(MessageCacheHandler::new(config.message_cache.disabled))
}

pub fn commands() -> Vec<AppCommand> {
    build_commands(vec![
        auth::create_keyword_button,
        // question::question,
        pin::pin,
        admin::reload_config,
        // thread_auto_invite::invite_thread,
        // thread_auto_invite::add_invite_role,
        // thread_auto_invite::remove_invite_role,
    ])
}

fn alias_command(base: fn() -> AppCommand, name: Cow<'static, str>) -> AppCommand {
    let mut command = base();
    command.name = name;
    command.aliases = (&[]).into();
    command.context_menu_action = None;
    command.context_menu_name = None;
    command
}

fn build_commands(commands: Vec<fn() -> AppCommand>) -> Vec<AppCommand> {
    commands
        .into_iter()
        .flat_map(|cmd| {
            let base = cmd();
            let aliases = base.aliases.clone();
            std::iter::once(base)
                .chain(aliases.iter().map(move |a| alias_command(cmd, a.clone())))
                .collect::<Vec<_>>()
        })
        .collect()
}
