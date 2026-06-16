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

fn format_omitted_equal(head: Option<&str>, tail: Option<&str>, count: usize) -> String {
    let head = head.map(|line| format!("  {line}")).unwrap_or_default();
    let tail = tail.map(|line| format!("  {line}")).unwrap_or_default();

    format!("{head}  ... {count}行省略\n{tail}")
}

fn format_change(change: Change<&'_ str>) -> String {
    match change.tag() {
        ChangeTag::Delete => format!("- {change}"),
        ChangeTag::Insert => format!("+ {change}"),
        ChangeTag::Equal => format!("  {change}"),
    }
}

fn create_diff_lines_text(old: &str, new: &str) -> String {
    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_lines(old, new);
    let grouped_changes = diff.iter_all_changes().chunk_by(|c| c.tag());
    let diff_chunks = grouped_changes.into_iter().collect_vec();
    let last_chunk_index = diff_chunks.len() - 1;
    diff_chunks
        .into_iter()
        .enumerate()
        .flat_map(|(chunk_index, (tag, changes))| match tag {
            ChangeTag::Delete | ChangeTag::Insert => Either::Left(changes.map(format_change)),
            ChangeTag::Equal => {
                let changes = changes.collect_vec();
                if changes.len() <= 3 {
                    Either::Right(Either::Left(changes.into_iter().map(format_change)))
                } else {
                    let has_head = chunk_index != 0;
                    let has_tail = chunk_index != last_chunk_index;

                    Either::Right(Either::Right(iter::once(format_omitted_equal(
                        has_head.then(|| changes.first().unwrap().value()),
                        has_tail.then(|| changes.last().unwrap().value()),
                        changes.len() - ((has_head as usize) + (has_tail as usize)),
                    ))))
                }
            }
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
