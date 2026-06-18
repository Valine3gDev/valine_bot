use serenity::model::channel::Attachment;

pub trait AttachmentExt {
    fn is_video(&self) -> bool;
    fn is_image(&self) -> bool;
}

impl AttachmentExt for Attachment {
    fn is_video(&self) -> bool {
        self.content_type.as_ref().is_some_and(|c| c.starts_with("video"))
    }

    fn is_image(&self) -> bool {
        self.content_type.as_ref().is_some_and(|c| c.starts_with("image"))
    }
}
