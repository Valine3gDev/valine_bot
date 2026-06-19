use std::{borrow::Cow, iter};

use crate::{
    app::utils::components::{
        create_container_section, create_container_text, create_section_thumbnail, create_separator,
    },
    extensions::{AttachmentExt, MessageBuilderTimestampExt},
    features::message_logging::log_type::MessageLogKind,
};
use itertools::{Either, Itertools, enumerate};
use serenity::{
    all::{Message, MessageBuilder, MessageReferenceKind},
    builder::{
        CreateContainerComponent, CreateFile, CreateMediaGallery, CreateMediaGalleryItem, CreateSectionComponent,
        CreateUnfurledMediaItem,
    },
    model::{
        channel::{Attachment, MessageFlags, MessageReference, MessageType},
        id::AttachmentId,
    },
    utils::{Content, ContentModifier},
};
use similar::{Algorithm, Change, ChangeTag, TextDiff};

pub fn bold_underline<'a>(content: &'a str) -> Content<'a> {
    content + ContentModifier::Bold + ContentModifier::Underline
}

fn format_message_link_line(name: &str, message_reference: &MessageReference) -> String {
    let id_str = message_reference
        .message_id
        .map(|id| id.to_string())
        .unwrap_or("<Unknown Message>".to_string());
    let id_link = message_reference
        .message_id
        .map(|id| id.link(message_reference.channel_id, message_reference.guild_id))
        .map(|link| link.to_string())
        .unwrap_or("<Unknown Message>".to_string());

    MessageBuilder::new()
        .push_bold_safe(format!("{name}: ").as_str())
        .push_safe(id_link.as_str())
        .push_safe(" ")
        .push_mono_line_safe(id_str.as_str())
        .build()
}

fn format_channel_link_line(name: &str, message_reference: &MessageReference) -> String {
    let channel_link_str = message_reference
        .guild_id
        .map(|id| format!("https://discord.com/channels/{id}/{}", message_reference.channel_id))
        .unwrap_or("<Unknown Channel>".to_string());

    MessageBuilder::new()
        .push_bold_safe(format!("{name}: ").as_str())
        .push_safe(channel_link_str.as_str())
        .build()
}

fn build_reference_title_and_body(message: &Message, message_reference: &MessageReference) -> (&'static str, String) {
    match message.kind {
        MessageType::ThreadCreated => return ("スレッド作成", format_channel_link_line("スレッド", message_reference)),
        MessageType::PinsAdd => return ("ピン留め", format_message_link_line("対象", message_reference)),
        MessageType::InlineReply => return ("返信", format_message_link_line("返信元", message_reference)),
        MessageType::PollResult => return ("投票結果", format_message_link_line("投票", message_reference)),
        MessageType::ChannelFollowAdd => {
            return (
                "フォロー",
                format_channel_link_line("対象チャンネル", message_reference),
            );
        }
        MessageType::ThreadStarterMessage => {
            return (
                "スレッド作成",
                format_message_link_line("元メッセージ", message_reference),
            );
        }
        _ => {}
    };

    if let Some(flags) = message.flags
        && flags.contains(MessageFlags::IS_CROSSPOST)
    {
        return ("購読", format_message_link_line("元チャンネル", message_reference));
    }

    ("参照", format_message_link_line("参照元", message_reference))
}

pub(in crate::features::message_logging) fn build_message_reference_container_component<'a>(
    message: &Message,
) -> Option<CreateContainerComponent<'a>> {
    let message_reference = message.message_reference.as_ref()?;

    let (name, content) = match message_reference.kind {
        MessageReferenceKind::Default => build_reference_title_and_body(message, message_reference),
        MessageReferenceKind::Forward => ("転送", format_message_link_line("転送元", message_reference)),
        _ => ("不明", "不明なメッセージ参照: ".to_string()),
    };

    Some(create_container_text(
        MessageBuilder::new()
            .push("### ")
            .push_line(bold_underline(name))
            .push_line(content.as_str())
            .build(),
    ))
}

pub(in crate::features::message_logging) fn build_poll_container_component<'a>(
    message: &Message,
) -> Option<CreateContainerComponent<'a>> {
    let poll = message.poll.as_ref()?;

    let mut builder = MessageBuilder::new()
        .push("### ")
        .push_line(bold_underline("投票"))
        .push_bold_safe("タイトル: ")
        .push_line_safe(poll.question.text.as_deref().unwrap_or("<不明なタイトル>"))
        .push_bold_line_safe("回答:");

    for (i, answer) in enumerate(&poll.answers) {
        let i = i.try_into().ok()?;

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
            .push_bold_safe("投票期限: ")
            .push_short_date_medium_timestamp(expiry);
    }

    Some(create_container_text(builder.build()))
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

fn format_content_diff(old: &str, new: &str) -> String {
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

pub(in crate::features::message_logging) fn build_diff_container_component<'a>(
    old_content: &str,
    new_content: &str,
) -> Option<CreateContainerComponent<'a>> {
    if old_content.is_empty() {
        return None;
    }
    if old_content == new_content {
        return None;
    }

    let diff = format_content_diff(old_content, new_content);
    Some(create_container_text(
        MessageBuilder::new()
            .push("### ")
            .push_line(bold_underline("テキスト差分"))
            .push_codeblock_safe(diff.as_str(), Some("diff"))
            .build(),
    ))
}

fn removed_attachments<'a>(
    message: &'a Message,
    attachment_ids_after: &'a [AttachmentId],
) -> impl Iterator<Item = &'a Attachment> + use<'a> {
    message
        .attachments
        .iter()
        .filter(|attachment| !attachment_ids_after.contains(&attachment.id))
}

pub(in crate::features::message_logging) fn build_linked_removed_attachment_components<'a>(
    message: &'a Message,
    attachment_ids_after: &'a [AttachmentId],
) -> Vec<CreateContainerComponent<'a>> {
    if message.attachments.is_empty() {
        return vec![];
    }

    let (galleries, files): (Vec<_>, Vec<_>) =
        removed_attachments(message, attachment_ids_after).partition_map(|attachment| {
            let text = format!("- [{}](<{}>)", attachment.filename, attachment.url);
            if attachment.is_image() || attachment.is_video() {
                Either::Left(text)
            } else {
                Either::Right(text)
            }
        });

    let mut result = Vec::new();
    if !galleries.is_empty() {
        result.push(create_container_text(
            MessageBuilder::new()
                .push("### ")
                .push_line(bold_underline("削除された画像・動画"))
                .push(galleries.join("\n").as_str())
                .build(),
        ));
    }
    if !files.is_empty() {
        result.push(create_container_text(
            MessageBuilder::new()
                .push("### ")
                .push_line(bold_underline("削除されたファイル"))
                .push(files.join("\n").as_str())
                .build(),
        ));
    }

    result
}

pub(in crate::features::message_logging) fn build_uploaded_removed_attachment_components<'a>(
    message: &'a Message,
    attachment_ids_after: &'a [AttachmentId],
) -> Vec<CreateContainerComponent<'a>> {
    if message.attachments.is_empty() {
        return vec![];
    }

    let (galleries, files): (Vec<_>, Vec<_>) =
        removed_attachments(message, attachment_ids_after).partition_map(|attachment| {
            let item = CreateUnfurledMediaItem::new(format!("attachment://{}", attachment.filename));
            if attachment.is_image() || attachment.is_video() {
                Either::Left(item)
            } else {
                Either::Right(item)
            }
        });

    let mut result = Vec::new();
    if !galleries.is_empty() {
        result.extend([
            create_container_text(
                MessageBuilder::new()
                    .push("### ")
                    .push_line(bold_underline("削除された画像・動画"))
                    .build(),
            ),
            CreateContainerComponent::MediaGallery(CreateMediaGallery::new(
                galleries.into_iter().map(CreateMediaGalleryItem::new).collect_vec(),
            )),
        ]);
    }
    if !files.is_empty() {
        result.push(create_container_text(
            MessageBuilder::new()
                .push("### ")
                .push_line(bold_underline("削除されたファイル"))
                .build(),
        ));
        result.extend(
            files
                .into_iter()
                .map(|item| CreateContainerComponent::File(CreateFile::new(item))),
        );
    }

    result
}

pub(in crate::features::message_logging) fn build_log_container_components<'a>(
    message: &Message,
    log_kind: &MessageLogKind,
    basic_info_section_component: impl Into<Cow<'a, [CreateSectionComponent<'a>]>>,
    attachment_components: Vec<CreateContainerComponent<'a>>,
) -> Vec<CreateContainerComponent<'a>> {
    iter::once(create_container_text(format!("### **{}**", log_kind.title())))
        .chain(
            [
                Some(create_container_section(
                    basic_info_section_component.into(),
                    create_section_thumbnail(
                        message
                            .author
                            .avatar_url()
                            .unwrap_or("https://cdn.discordapp.com/embed/avatars/0.png".to_string()),
                        Some("ユーザーアイコン"),
                        false,
                    ),
                )),
                build_message_reference_container_component(message),
                build_poll_container_component(message),
                build_diff_container_component(&message.content, log_kind.content_after()),
            ]
            .into_iter()
            .filter_map(|c| c.map(|c| [create_separator(false), c].into_iter()))
            .flatten(),
        )
        .chain(
            attachment_components
                .into_iter()
                .flat_map(|c| [create_separator(false), c]),
        )
        .collect_vec()
}
