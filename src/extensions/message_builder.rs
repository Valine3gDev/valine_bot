use serenity::{
    model::Timestamp,
    utils::{FormattedTimestamp, FormattedTimestampStyle, MessageBuilder},
};

#[allow(dead_code)]
pub trait MessageBuilderTimestampExt {
    fn _push_linebreak(&mut self) -> &mut Self;

    fn push_timestamp(&mut self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> &mut Self;

    fn push_timestamp_line(&mut self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> &mut Self {
        self.push_timestamp(timestamp, style)._push_linebreak()
    }

    fn push_timestamp_short(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortTime))
    }
    fn push_timestamp_short_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_short(timestamp)._push_linebreak()
    }

    fn push_timestamp_long(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::LongTime))
    }
    fn push_timestamp_long_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_long(timestamp)._push_linebreak()
    }

    fn push_timestamp_short_date(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortDate))
    }
    fn push_timestamp_short_date_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_short_date(timestamp)._push_linebreak()
    }

    fn push_timestamp_long_date(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::LongDate))
    }
    fn push_timestamp_long_date_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_long_date(timestamp)._push_linebreak()
    }

    fn push_timestamp_short_date_time(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::ShortDateTime))
    }
    fn push_timestamp_short_date_time_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_short_date_time(timestamp)._push_linebreak()
    }

    fn push_timestamp_long_date_time(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::LongDateTime))
    }
    fn push_timestamp_long_date_time_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_long_date_time(timestamp)._push_linebreak()
    }

    fn push_timestamp_relative_time(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp(timestamp, Some(FormattedTimestampStyle::RelativeTime))
    }
    fn push_timestamp_relative_time_line(&mut self, timestamp: Timestamp) -> &mut Self {
        self.push_timestamp_relative_time(timestamp)._push_linebreak()
    }
}

impl MessageBuilderTimestampExt for MessageBuilder {
    fn push_timestamp(&mut self, timestamp: Timestamp, style: Option<FormattedTimestampStyle>) -> &mut Self {
        let formatted = FormattedTimestamp::new(timestamp, style).to_string();
        self.push(formatted)
    }

    fn _push_linebreak(&mut self) -> &mut Self {
        self.push_line("")
    }
}
