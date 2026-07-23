use chrono::SecondsFormat;
use chrono_tz::Tz;

use crate::model::AliasedMessage;

use super::sorted_messages;

pub(super) fn render_tsv(messages: &[AliasedMessage], timezone: Tz) -> String {
    let mut output = String::from("thread\ttime\tspeaker\tkind\tcontent\n");
    for item in sorted_messages(messages) {
        let timestamp = item
            .message
            .original
            .timestamp_utc
            .with_timezone(&timezone)
            .to_rfc3339_opts(SecondsFormat::Secs, false);
        let kind = item.message.original.kind.to_string();
        for (index, field) in [
            item.thread_alias.as_str(),
            timestamp.as_str(),
            item.speaker_alias.as_str(),
            kind.as_str(),
            item.message.content.as_str(),
        ]
        .into_iter()
        .enumerate()
        {
            if index > 0 {
                output.push('\t');
            }
            output.push_str(&escape(field));
        }
        output.push('\n');
    }
    output
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
