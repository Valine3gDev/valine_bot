mod admin;
// mod auth;
mod auto_kick;
mod honeypot;
// mod logging;
// mod message_cache;
// mod pin;
// mod question;
// mod thread_auto_invite;
// mod thread_channel_startup;

// pub use auth::Handler as AuthHandler;
pub use auto_kick::AutoKickEventHandler;
pub use honeypot::handle_honeypot_event;
// pub use logging::Handler as LoggingHandler;
// pub use message_cache::Handler as MessageCacheHandler;
// pub use question::Handler as QuestionHandler;
// pub use thread_auto_invite::ThreadAutoInviteHandler;
// pub use thread_channel_startup::Handler as ThreadChannelStartupHandler;

// pub use message_cache::{MessageCache, MessageCacheType};

use std::borrow::Cow;

use crate::app::AppCommand;

pub fn commands() -> Vec<AppCommand> {
    build_commands(vec![
        // auth::create_keyword_button,
        // question::question,
        // pin::pin,
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
