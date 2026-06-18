use itertools::Itertools;
use serenity::{
    model::{Color, channel::Attachment, id::AttachmentId},
    small_fixed_array::{FixedArray, FixedString},
};

pub(in crate::features::message_logging) enum MessageLogKind<'a> {
    Edit {
        content_after: &'a FixedString<u16>,
        attachments_after: &'a FixedArray<Attachment>,
    },
    Delete,
}

impl MessageLogKind<'_> {
    pub fn name(&self) -> &'static str {
        match self {
            MessageLogKind::Edit { .. } => "編集",
            MessageLogKind::Delete => "削除",
        }
    }

    pub fn title(&self) -> String {
        format!("メッセージ{}ログ", self.name())
    }

    pub fn color(&self) -> Color {
        match self {
            MessageLogKind::Edit { .. } => Color::ORANGE,
            MessageLogKind::Delete => Color::RED,
        }
    }

    pub fn content_after(&self) -> &str {
        match self {
            MessageLogKind::Edit { content_after, .. } => content_after,
            MessageLogKind::Delete => Default::default(),
        }
    }

    pub fn attachment_ids_after(&self) -> Vec<AttachmentId> {
        match self {
            MessageLogKind::Edit { attachments_after, .. } => attachments_after.iter().map(|a| a.id).collect_vec(),
            MessageLogKind::Delete => Default::default(),
        }
    }
}
