use std::borrow::Cow;

use serenity::all::{Mentionable, MessageBuilder};
use serenity::builder::{CreateEmbed, CreateMessage};
use serenity::model::Color;
use serenity::model::guild::Member;
use serenity::utils::EmbedMessageBuilding;

use crate::utils::create_safe_message;

pub(in crate::features::auth) fn create_auth_log_message<'a>(
    title: impl Into<Cow<'a, str>>,
    color: impl Into<Color>,
    member: &Member,
    dm_delivery_succeeded: Option<bool>,
) -> CreateMessage<'a> {
    let mut description = MessageBuilder::new()
        .push("- ")
        .push_bold_line("ユーザー")
        .push("  - ")
        .push_bold("表示名: ")
        .push_line_safe(member.display_name())
        .push("  - ")
        .push_bold("メンション: ")
        .push_safe(&*member.mention().to_string())
        .push(" ")
        .push_named_link("リンク", &*format!("<https://discord.com/users/{}>", member.user.id))
        .push_line("")
        .push("  - ")
        .push_bold("ユーザー名: ")
        .push_line_safe(&*member.user.name)
        .push("  - ")
        .push_bold("ID: ")
        .push_line_safe(&*member.user.id.to_string());

    if let Some(delivery_succeeded) = dm_delivery_succeeded {
        description = description
            .push("- ")
            .push_bold("DM送信可否: ")
            .push_line(if delivery_succeeded { "YES" } else { "NO" });
    }

    let embed = CreateEmbed::new()
        .title(title)
        .description(description.build())
        .color(color)
        .thumbnail(
            member
                .user
                .avatar_url()
                .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
            Some("ユーザーアイコン".into()),
        );

    create_safe_message().embed(embed)
}
