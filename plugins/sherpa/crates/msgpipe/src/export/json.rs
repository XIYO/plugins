use anyhow::Result;
use chrono::SecondsFormat;
use chrono_tz::Tz;
use serde::Serialize;

use crate::model::AliasedMessage;

use super::sorted_messages;

#[derive(Serialize)]
struct CompactRow<'a>(&'a str, String, &'a str, &'a str, &'a str);

pub(super) fn render_json(messages: &[AliasedMessage], timezone: Tz) -> Result<String> {
    let rows: Vec<_> = sorted_messages(messages)
        .into_iter()
        .map(|item| {
            CompactRow(
                &item.thread_alias,
                item.message
                    .original
                    .timestamp_utc
                    .with_timezone(&timezone)
                    .to_rfc3339_opts(SecondsFormat::Secs, false),
                &item.speaker_alias,
                item.message.original.kind.marker(),
                &item.message.content,
            )
        })
        .collect();
    Ok(serde_json::to_string(&rows)?)
}
