use serenity::model::Color;

pub(in crate::features::message_logging) enum LogType {
    Edit { new_content: String },
    Delete,
}

impl LogType {
    pub fn name(&self) -> &'static str {
        match self {
            LogType::Edit { .. } => "編集",
            LogType::Delete => "削除",
        }
    }

    pub fn title(&self) -> String {
        format!("メッセージ{}ログ", self.name())
    }

    pub fn color(&self) -> Color {
        match self {
            LogType::Edit { .. } => Color::ORANGE,
            LogType::Delete => Color::RED,
        }
    }

    pub fn new_content(&self) -> Option<&str> {
        match self {
            LogType::Edit { new_content } => Some(new_content),
            LogType::Delete => None,
        }
    }
}
