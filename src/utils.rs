use serenity::all::{CreateAllowedMentions, CreateMessage};

pub fn create_message(content: String) -> CreateMessage {
    CreateMessage::new()
        .content(content)
        .allowed_mentions(CreateAllowedMentions::new().all_users(false))
}
