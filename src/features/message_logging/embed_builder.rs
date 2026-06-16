use std::{
    iter,
    ops::{Deref, Not},
};

use itertools::{Either, Itertools, enumerate};
use serenity::all::{CreateEmbed, EmbedMessageBuilding, Message, MessageBuilder, MessageId, MessageReferenceKind};
use similar::{Algorithm, Change, ChangeTag, TextDiff};
use tracing::error;

use crate::extensions::MessageBuilderTimestampExt;

pub(in crate::features::message_logging) fn build_reply_field<'a>(
    embed: CreateEmbed<'a>,
    message: &Message,
) -> CreateEmbed<'a> {
    let Some(m_ref) = &message.message_reference else {
        return embed;
    };
    let id = m_ref.message_id.unwrap_or(MessageId::default());
    let (name, content) = match m_ref.kind {
        MessageReferenceKind::Default => ("__**返信**__", "返信先: "),
        MessageReferenceKind::Forward => ("__**転送**__", "転送元: "),
        _ => ("__**不明**__", "不明な対象メッセージ: "),
    };
    embed.field(
        name,
        MessageBuilder::new()
            .push_bold_safe(content)
            .push_safe(&*id.link(m_ref.channel_id, m_ref.guild_id).to_string())
            .push_safe(" ")
            .push_mono_line_safe(&*id.to_string())
            .build(),
        false,
    )
}

pub(in crate::features::message_logging) fn build_poll_field<'a>(
    embed: CreateEmbed<'a>,
    message: &Message,
) -> CreateEmbed<'a> {
    let Some(poll) = &message.poll else {
        return embed;
    };

    let mut builder = MessageBuilder::new();
    builder = builder
        .push_bold_safe("タイトル: ")
        .push_line_safe(poll.question.text.as_deref().unwrap_or("<不明なタイトル>"))
        .push_bold_line_safe("回答:");

    for (i, answer) in enumerate(&poll.answers) {
        let Ok(i) = i.try_into() else {
            error!("poll answer index must fit in u8");
            break;
        };

        let answer_text = answer.poll_media.text.as_deref().unwrap_or("<不明な回答>");
        builder = builder.push_safe(format!("- {answer_text}").as_str());

        builder = if let Some(results) = &poll.results {
            builder.push_line_safe(format!(": {}票", results.answer_counts[i].count).as_str())
        } else {
            builder.push_safe("\n")
        };
    }

    if let Some(expiry) = poll.expiry {
        builder = builder
            .push_bold_safe("有効期限: ")
            .push_short_date_medium_timestamp(expiry);
    }

    embed.field("__**投票**__", builder.build(), false)
}

enum DiffLine<'a> {
    OmittedEqualBlock {
        first_line: &'a str,
        last_line: &'a str,
        omitted_line_count: usize,
    },
    Change(Change<&'a str>),
}

fn create_diff_lines_text(old: &str, new: &str) -> String {
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_lines(old, new);
    diff.iter_all_changes()
        .chunk_by(|c| c.tag())
        .into_iter()
        .flat_map(|(tag, changes)| match tag {
            ChangeTag::Delete | ChangeTag::Insert => Either::Left(changes.map(DiffLine::Change)),
            ChangeTag::Equal => {
                let equal_changes = changes.collect_vec();
                if equal_changes.len() <= 3 {
                    Either::Right(Either::Left(equal_changes.into_iter().map(DiffLine::Change)))
                } else {
                    let first_line = equal_changes.first().unwrap().value();
                    let last_line = equal_changes.last().unwrap().value();
                    let omitted_line_count = equal_changes.len() - 2;

                    Either::Right(Either::Right(iter::once(DiffLine::OmittedEqualBlock {
                        first_line,
                        last_line,
                        omitted_line_count,
                    })))
                }
            }
        })
        .map(|line| match line {
            DiffLine::Change(change) => match change.tag() {
                ChangeTag::Delete => format!("- {change}"),
                ChangeTag::Insert => format!("+ {change}"),
                ChangeTag::Equal => format!("  {change}"),
            },
            DiffLine::OmittedEqualBlock {
                first_line,
                last_line,
                omitted_line_count,
            } => format!("  {first_line}  ... {omitted_line_count}行省略\n  {last_line}"),
        })
        .join("")
}

pub(in crate::features::message_logging) fn build_diff_field<'a>(
    mut embed: CreateEmbed<'a>,
    old_content: &str,
    new_content: &str,
) -> CreateEmbed<'a> {
    if old_content.is_empty() {
        return embed;
    }

    let diff = create_diff_lines_text(old_content, new_content);
    let chunks = diff.lines().peekable().batching(|lines| {
        let mut str = String::new();
        while let Some(line) = lines.next_if(|&l| str.len() + l.len() <= 1000) {
            str.push_str(line);
            str.push('\n');
        }
        str.is_empty().not().then_some(str)
    });

    for (i, chunk) in enumerate(chunks) {
        let changed = MessageBuilder::new()
            .push_codeblock_safe(chunk.as_str(), Some("diff"))
            .build();

        embed = embed.field(if i == 0 { "__**テキスト差分**__" } else { "" }, changed, false)
    }
    embed
}

pub(in crate::features::message_logging) fn build_attachments_field<'a>(
    embed: CreateEmbed<'a>,
    message: &Message,
) -> CreateEmbed<'a> {
    if message.attachments.is_empty() {
        return embed;
    }
    let mut builder = MessageBuilder::new();
    for attachment in &message.attachments {
        builder = builder
            .push_safe("- ")
            .push_named_link_safe(attachment.filename.deref(), attachment.url.deref())
            .push_safe("\n");
    }
    embed.field("__**添付ファイル**__", builder.build(), false)
}

pub(in crate::features::message_logging) fn build_embed<'a>(
    message: &Message,
    new_content: &str,
    mut embed: CreateEmbed<'a>,
) -> CreateEmbed<'a> {
    embed = build_reply_field(embed, message);
    embed = build_poll_field(embed, message);
    embed = build_diff_field(embed, &message.content, new_content);
    build_attachments_field(embed, message)
}
