use serenity::{
    model::Timestamp,
    utils::{FormattedTimestamp, FormattedTimestampStyle, MessageBuilder},
};

#[allow(dead_code)]
pub trait MessageBuilderTimestampExt {
    fn push_timestamp(self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> Self;
    fn push_timestamp_line(self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> Self;

    fn push_short_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_short_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_medium_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_medium_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_short_date_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_short_date_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_long_date_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_long_date_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_long_date_short_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_long_date_short_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_full_date_short_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_full_date_short_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_short_date_short_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_short_date_short_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_short_date_medium_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_short_date_medium_timestamp_line(self, timestamp: Timestamp) -> Self;

    fn push_relative_timestamp(self, timestamp: Timestamp) -> Self;
    fn push_timestamp_relative_time_line(self, timestamp: Timestamp) -> Self;
}

fn _push_linebreak(mut builder: MessageBuilder) -> MessageBuilder {
    builder.0.push('\n');
    builder
}

impl MessageBuilderTimestampExt for MessageBuilder {
    fn push_timestamp(mut self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> Self {
        let formatted = FormattedTimestamp::new(timestamp, style).to_string();
        self.0.push_str(&formatted);
        self
    }
    fn push_timestamp_line(self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> Self {
        _push_linebreak(self.push_timestamp(timestamp, style))
    }

    fn push_short_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortTime))
    }
    fn push_short_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_short_timestamp(timestamp))
    }

    fn push_medium_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::MediumTime))
    }
    fn push_medium_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_medium_timestamp(timestamp))
    }

    fn push_short_date_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortDate))
    }
    fn push_short_date_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_short_date_timestamp(timestamp))
    }

    fn push_long_date_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::LongDate))
    }
    fn push_long_date_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_long_date_timestamp(timestamp))
    }

    fn push_long_date_short_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::LongDateShortTime))
    }
    fn push_long_date_short_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_long_date_short_timestamp(timestamp))
    }

    fn push_full_date_short_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::FullDateShortTime))
    }
    fn push_full_date_short_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_full_date_short_timestamp(timestamp))
    }

    fn push_short_date_short_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortDateShortTime))
    }
    fn push_short_date_short_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_short_date_short_timestamp(timestamp))
    }

    fn push_short_date_medium_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortDateMediumTime))
    }
    fn push_short_date_medium_timestamp_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_short_date_medium_timestamp(timestamp))
    }

    fn push_relative_timestamp(self, timestamp: Timestamp) -> Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::RelativeTime))
    }
    fn push_timestamp_relative_time_line(self, timestamp: Timestamp) -> Self {
        _push_linebreak(self.push_relative_timestamp(timestamp))
    }
}
