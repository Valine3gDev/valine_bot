mod admin;
mod auth;
mod auto_kick;
mod logging;
mod message_cache;
mod pin;
mod question;
mod thread_auto_invite;
mod thread_channel_startup;

pub use auth::Handler as AuthHandler;
pub use auto_kick::Handler as AutoKickHandler;
pub use logging::Handler as LoggingHandler;
pub use message_cache::Handler as MessageCacheHandler;
pub use question::Handler as QuestionHandler;
pub use thread_auto_invite::Handler as ThreadAutoInviteHandler;
pub use thread_channel_startup::Handler as ThreadChannelStartupHandler;

pub use message_cache::{MessageCache, MessageCacheType};

use crate::PCommand;

pub fn commands() -> Vec<PCommand> {
    build_commands(vec![
        auth::create_keyword_button,
        question::question,
        pin::pin,
        admin::reload_config,
        thread_auto_invite::invite_thread,
    ])
}

fn alias_command(base: fn() -> PCommand, name: String) -> PCommand {
    let mut command = base();
    command.name = name;
    command.aliases.clear();
    command.context_menu_action = None;
    command.context_menu_name = None;
    command
}

fn build_commands(commands: Vec<fn() -> PCommand>) -> Vec<PCommand> {
    commands
        .into_iter()
        .flat_map(|cmd| {
            let base = cmd();
            let aliases = base
                .aliases
                .clone()
                .into_iter()
                .map(move |alias| alias_command(cmd, alias));
            std::iter::once(base).chain(aliases).collect::<Vec<_>>()
        })
        .collect()
}
