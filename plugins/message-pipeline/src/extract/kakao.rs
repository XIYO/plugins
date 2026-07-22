use std::{ffi::OsString, sync::Arc, time::Instant};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use serde_json::Value;
use tracing::{error, info};

use crate::{
    command::CommandRunner,
    model::{AttachmentMeta, MessageKind, NormalizedMessage, SourceKind},
    time_range::DateRange,
};

use super::{ExtractRequest, Extractor};

pub struct KakaoExtractor {
    runner: Arc<dyn CommandRunner>,
}

impl KakaoExtractor {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

impl Extractor for KakaoExtractor {
    fn extract(&self, request: &ExtractRequest) -> Result<Vec<NormalizedMessage>> {
        info!("[extract:kakaotalk:start] extracting read-only message range");
        let started = Instant::now();
        let query = build_query(request.range);
        let args = [OsString::from("query"), OsString::from(query)];
        let output = self.runner.run("kakaotalk.query", &request.binary, &args);
        let output = match output {
            Ok(output) => output,
            Err(error_value) => {
                error!(
                    error = ?error_value,
                    "[extract:kakaotalk:failure] read-only extraction failed"
                );
                return Err(error_value);
            }
        };
        let messages = parse_rows(&output.stdout).context("invalid KakaoTalk query output")?;
        info!(
            row_count = messages.len(),
            duration_ms = started.elapsed().as_millis(),
            "[extract:kakaotalk:success] extraction completed"
        );
        Ok(messages)
    }
}

fn build_query(range: DateRange) -> String {
    let start = range.start.timestamp();
    let end = range.end.timestamp();
    format!(
        "WITH me AS (SELECT userId FROM NTChatContext LIMIT 1)\n\
         SELECT m.chatId, m.logId, m.sentAt,\n\
                COALESCE(NULLIF(r.chatName, ''), du.displayName, du.friendNickName, du.nickName, '(unknown)'),\n\
                m.authorId,\n\
                CASE WHEN m.authorId = (SELECT userId FROM me) THEN 'Me'\n\
                     ELSE COALESCE(au.displayName, au.friendNickName, au.nickName, '(unknown)') END,\n\
                m.type, COALESCE(m.message, ''), COALESCE(m.attachment, ''),\n\
                COALESCE(m.supplement, ''), COALESCE(m.extra, ''),\n\
                CASE WHEN m.authorId = (SELECT userId FROM me) THEN 1 ELSE 0 END\n\
         FROM NTChatMessage m\n\
         LEFT JOIN NTChatRoom r ON m.chatId = r.chatId\n\
         LEFT JOIN NTUser du ON r.directChatMemberUserId = du.userId AND du.linkId = 0\n\
         LEFT JOIN NTUser au ON m.authorId = au.userId AND au.linkId = 0\n\
         WHERE m.sentAt >= {start} AND m.sentAt < {end}\n\
         ORDER BY m.chatId ASC, m.sentAt ASC, m.logId ASC"
    )
}

fn parse_rows(bytes: &[u8]) -> Result<Vec<NormalizedMessage>> {
    let value: Value = serde_json::from_slice(bytes).context("expected a JSON array")?;
    let rows = value
        .as_array()
        .context("top-level value must be an array")?;
    rows.iter()
        .enumerate()
        .map(|(index, row)| parse_row(index, row))
        .collect()
}

fn parse_row(index: usize, value: &Value) -> Result<NormalizedMessage> {
    let row = value
        .as_array()
        .with_context(|| format!("row {index} must be an array"))?;
    if row.len() < 12 {
        bail!("row {index} has {} columns; expected 12", row.len())
    }

    let thread_id = scalar_string(&row[0]).with_context(|| format!("row {index} thread id"))?;
    let log_id = scalar_string(&row[1]).with_context(|| format!("row {index} log id"))?;
    let timestamp = integer(&row[2]).with_context(|| format!("row {index} timestamp"))?;
    let timestamp_utc = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .with_context(|| format!("row {index} timestamp is out of range"))?;
    let author_id = scalar_string(&row[4]).with_context(|| format!("row {index} author id"))?;
    let raw_type = integer(&row[6]).with_context(|| format!("row {index} message type"))?;
    let kind = kakao_kind(raw_type);
    let attachments = kakao_attachment_meta(kind, &row[8..=10]);
    let message = NormalizedMessage {
        source: SourceKind::KakaoTalk,
        source_message_id: format!("{thread_id}:{log_id}"),
        source_thread_id: thread_id,
        thread_name: text(&row[3]),
        source_author_id: author_id,
        author_name: text(&row[5]),
        is_from_me: boolean(&row[11]).with_context(|| format!("row {index} self flag"))?,
        timestamp_utc,
        kind,
        content: text(&row[7]),
        attachments,
    };
    message
        .validate()
        .with_context(|| format!("row {index} is invalid"))?;
    Ok(message)
}

fn scalar_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) if !value.is_empty() => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn integer(value: &Value) -> Option<i64> {
    match value {
        Value::Number(value) => value.as_i64().or_else(|| {
            value
                .as_f64()
                .filter(|value| value.is_finite())
                .map(|value| value.trunc() as i64)
        }),
        Value::String(value) => value.parse().ok().or_else(|| {
            value
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map(|value| value.trunc() as i64)
        }),
        _ => None,
    }
}

fn text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn boolean(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(value) => Some(*value),
        Value::Number(value) => value.as_i64().map(|value| value != 0),
        Value::String(value) => match value.as_str() {
            "1" | "true" => Some(true),
            "0" | "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn kakao_kind(raw: i64) -> MessageKind {
    let base = raw & 0x3fff;
    match base {
        0 => MessageKind::System,
        1 => MessageKind::Text,
        2 => MessageKind::Image,
        3 => MessageKind::Video,
        4 => MessageKind::File,
        5 => MessageKind::Audio,
        12 | 20 | 25 => MessageKind::Sticker,
        16 => MessageKind::Location,
        26 => MessageKind::Text,
        27 => MessageKind::Image,
        51 => MessageKind::System,
        _ => MessageKind::Unknown,
    }
}

fn kakao_attachment_meta(kind: MessageKind, raw_fields: &[Value]) -> Vec<AttachmentMeta> {
    // Type 1/26 text rows commonly carry mention, linkify, bot, or reply metadata in
    // `attachment`; that metadata is not a standalone attachment and must not produce
    // an @attachment marker for almost every ordinary message.
    if kind == MessageKind::Text {
        return Vec::new();
    }
    let has_metadata = raw_fields
        .iter()
        .any(|value| !text(value).trim().is_empty());
    if !has_metadata && matches!(kind, MessageKind::Text | MessageKind::System) {
        return Vec::new();
    }
    vec![AttachmentMeta {
        kind,
        mime_type: None,
        byte_count: None,
        is_sticker: kind == MessageKind::Sticker,
        is_missing: false,
    }]
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        path::Path,
        sync::{Arc, Mutex},
    };

    use anyhow::Result;
    use chrono::{TimeZone, Utc};

    use super::{KakaoExtractor, build_query, parse_rows};
    use crate::{
        command::{CommandRunner, ProcessOutput},
        extract::{ExtractRequest, Extractor},
        time_range::DateRange,
    };

    struct FakeRunner {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl CommandRunner for FakeRunner {
        fn run(
            &self,
            _boundary: &'static str,
            _program: &Path,
            args: &[OsString],
        ) -> Result<ProcessOutput> {
            self.calls.lock().unwrap().push(
                args.iter()
                    .map(|value| value.to_string_lossy().into_owned())
                    .collect(),
            );
            Ok(ProcessOutput {
                stdout: br#"[[12,34,1782864000,"Room",56,"Sender",1,"hello","","","",0]]"#.to_vec(),
            })
        }
    }

    #[test]
    fn query_is_a_fixed_read_only_statement() {
        let range = DateRange::new(
            Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap(),
        )
        .unwrap();
        let query = build_query(range);
        assert!(query.starts_with("WITH me AS"));
        assert!(!query.contains(';'));
        assert!(!query.to_ascii_uppercase().contains("DELETE"));
        assert!(!query.to_ascii_uppercase().contains("UPDATE"));
        assert!(!query.to_ascii_uppercase().contains("INSERT"));
    }

    #[test]
    fn parses_array_rows_without_losing_source_identity() {
        let input = br#"[[12,34,1782864000,"Room",56,"Sender",1,"hello","","","",0]]"#;
        let messages = parse_rows(input).unwrap();
        let message = &messages[0];
        assert_eq!(message.source_message_id, "12:34");
        assert_eq!(message.source_thread_id, "12");
        assert_eq!(message.source_author_id, "56");
        assert_eq!(message.content, "hello");
        assert!(!message.is_from_me);
    }

    #[test]
    fn accepts_sqlite_numeric_timestamps_serialized_as_float() {
        let input = br#"[[12,34,1782864000.75,"Room",56,"Sender",1,"hello","","","",0]]"#;
        let messages = parse_rows(input).unwrap();
        assert_eq!(messages[0].timestamp_utc.timestamp(), 1_782_864_000);
    }

    #[test]
    fn extractor_exposes_only_the_fixed_query_subcommand() {
        let runner = Arc::new(FakeRunner {
            calls: Mutex::new(Vec::new()),
        });
        let extractor = KakaoExtractor::new(runner.clone());
        let request = ExtractRequest::for_binary(
            DateRange::new(
                Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap(),
            )
            .unwrap(),
            "kakaocli".into(),
        );
        assert_eq!(extractor.extract(&request).unwrap().len(), 1);
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][0], "query");
        assert!(calls[0][1].starts_with("WITH me AS"));
    }
}
