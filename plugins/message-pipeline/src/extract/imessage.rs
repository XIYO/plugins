use std::{ffi::OsString, sync::Arc, time::Instant};

use anyhow::{Context, Result};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Map, Value};
use tracing::{debug, error, info};

use crate::{
    command::CommandRunner,
    model::{AttachmentMeta, MessageKind, NormalizedMessage, SourceKind},
};

use super::{ExtractRequest, Extractor};

pub struct IMessageExtractor {
    runner: Arc<dyn CommandRunner>,
}

impl IMessageExtractor {
    pub fn new(runner: Arc<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

impl Extractor for IMessageExtractor {
    fn extract(&self, request: &ExtractRequest) -> Result<Vec<NormalizedMessage>> {
        info!("[extract:imessage:start] extracting read-only message range");
        let started = Instant::now();
        let chat_limit = request.chat_limit.to_string();
        let chat_args = [
            OsString::from("chats"),
            OsString::from("--limit"),
            OsString::from(chat_limit),
            OsString::from("--json"),
            OsString::from("--log-level"),
            OsString::from("error"),
        ];
        let chat_output = self
            .runner
            .run("imessage.chats", &request.binary, &chat_args);
        let chat_output = match chat_output {
            Ok(output) => output,
            Err(error_value) => {
                error!(
                    error = ?error_value,
                    "[extract:imessage:failure] chat listing failed"
                );
                return Err(error_value);
            }
        };
        let chats =
            parse_chats(&chat_output.stdout, request).context("invalid iMessage chat listing")?;

        let mut messages = Vec::new();
        for (index, chat) in chats.iter().enumerate() {
            debug!(
                chat_index = index,
                chat_count = chats.len(),
                "[extract:imessage:history] reading candidate chat"
            );
            let start = request
                .range
                .start
                .to_rfc3339_opts(SecondsFormat::Secs, true);
            let end = request.range.end.to_rfc3339_opts(SecondsFormat::Secs, true);
            let per_chat_limit = request.message_limit_per_chat.to_string();
            let history_args = [
                OsString::from("history"),
                OsString::from("--chat-id"),
                OsString::from(&chat.id),
                OsString::from("--start"),
                OsString::from(start),
                OsString::from("--end"),
                OsString::from(end),
                OsString::from("--limit"),
                OsString::from(per_chat_limit),
                OsString::from("--attachments"),
                OsString::from("--json"),
                OsString::from("--log-level"),
                OsString::from("error"),
            ];
            let history_output =
                self.runner
                    .run("imessage.history", &request.binary, &history_args)?;
            messages.extend(
                parse_history(&history_output.stdout, chat)
                    .with_context(|| format!("invalid iMessage history at chat index {index}"))?,
            );
        }

        info!(
            chat_count = chats.len(),
            row_count = messages.len(),
            duration_ms = started.elapsed().as_millis(),
            "[extract:imessage:success] extraction completed"
        );
        Ok(messages)
    }
}

#[derive(Debug)]
struct ChatRef {
    id: String,
    display_name: String,
}

fn parse_chats(bytes: &[u8], request: &ExtractRequest) -> Result<Vec<ChatRef>> {
    let values = parse_json_values(bytes)?;
    let mut chats = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        let object = value
            .as_object()
            .with_context(|| format!("chat row {index} must be an object"))?;
        if let Some(last_message_at) = optional_text(object, &["last_message_at"])
            && let Ok(timestamp) = parse_timestamp(&last_message_at)
            && timestamp < request.range.start
        {
            continue;
        }
        let id = required_scalar(object, &["id", "chat_id"], index, "chat id")?;
        let display_name = optional_text(object, &["display_name", "name"])
            .unwrap_or_else(|| "(unknown)".to_string());
        chats.push(ChatRef { id, display_name });
    }
    Ok(chats)
}

fn parse_history(bytes: &[u8], chat: &ChatRef) -> Result<Vec<NormalizedMessage>> {
    parse_json_values(bytes)?
        .into_iter()
        .enumerate()
        .map(|(index, value)| parse_message(index, value, chat))
        .collect()
}

fn parse_message(index: usize, value: Value, chat: &ChatRef) -> Result<NormalizedMessage> {
    let object = value
        .as_object()
        .with_context(|| format!("message row {index} must be an object"))?;
    let is_from_me = optional_bool(object, &["is_from_me"]).unwrap_or(false);
    let source_message_id = required_scalar(
        object,
        &["guid", "id", "rowid", "message_id"],
        index,
        "message id",
    )?;
    let source_thread_id =
        optional_scalar(object, &["chat_id", "chat_guid"]).unwrap_or_else(|| chat.id.clone());
    let created_at = required_text(object, &["created_at", "timestamp"], index, "timestamp")?;
    let timestamp_utc = parse_timestamp(&created_at)
        .with_context(|| format!("message row {index} timestamp is invalid"))?;
    let sender = optional_text(object, &["sender", "sender_id", "handle"])
        .unwrap_or_else(|| "unknown".to_string());
    let author_name = if is_from_me {
        "Me".to_string()
    } else {
        optional_text(object, &["sender_name", "sender", "handle"])
            .unwrap_or_else(|| "(unknown)".to_string())
    };
    let attachments = parse_attachments(object.get("attachments"));
    let content = object
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let kind = if !content.trim().is_empty() {
        MessageKind::Text
    } else {
        attachments
            .first()
            .map(|attachment| attachment.kind)
            .unwrap_or(MessageKind::Unknown)
    };
    let message = NormalizedMessage {
        source: SourceKind::IMessage,
        source_message_id,
        source_thread_id,
        thread_name: chat.display_name.clone(),
        source_author_id: if is_from_me {
            "self".to_string()
        } else {
            sender
        },
        author_name,
        is_from_me,
        timestamp_utc,
        kind,
        content,
        attachments,
    };
    message
        .validate()
        .with_context(|| format!("message row {index} is invalid"))?;
    Ok(message)
}

fn parse_json_values(bytes: &[u8]) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    let stream = serde_json::Deserializer::from_slice(bytes).into_iter::<Value>();
    for value in stream {
        let value = value.context("invalid JSON/NDJSON")?;
        match value {
            Value::Array(items) => values.extend(items),
            value => values.push(value),
        }
    }
    Ok(values)
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|value| value.with_timezone(&Utc))
        .context("expected an RFC 3339 timestamp")
}

fn parse_attachments(value: Option<&Value>) -> Vec<AttachmentMeta> {
    let values: Vec<&Value> = match value {
        Some(Value::Array(values)) => values.iter().collect(),
        Some(Value::Object(_)) => vec![value.expect("value is present")],
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter_map(Value::as_object)
        .map(|object| {
            let mime_type = optional_text(object, &["mime_type", "mimeType"]);
            let uti = optional_text(object, &["uti"]);
            let is_sticker = optional_bool(object, &["is_sticker", "sticker"]).unwrap_or(false);
            let kind = attachment_kind(mime_type.as_deref(), uti.as_deref(), is_sticker);
            AttachmentMeta {
                kind,
                mime_type,
                byte_count: optional_u64(
                    object,
                    &[
                        "byte_size",
                        "byte_count",
                        "total_bytes",
                        "file_size",
                        "size",
                    ],
                ),
                is_sticker,
                is_missing: optional_bool(object, &["is_missing", "missing"]).unwrap_or(false),
            }
        })
        .collect()
}

fn attachment_kind(mime: Option<&str>, uti: Option<&str>, is_sticker: bool) -> MessageKind {
    if is_sticker {
        return MessageKind::Sticker;
    }
    let hint = format!("{} {}", mime.unwrap_or_default(), uti.unwrap_or_default()).to_lowercase();
    if hint.contains("image") {
        MessageKind::Image
    } else if hint.contains("video") || hint.contains("movie") {
        MessageKind::Video
    } else if hint.contains("audio") {
        MessageKind::Audio
    } else if hint.contains("url") {
        MessageKind::Link
    } else {
        MessageKind::File
    }
}

fn required_text(
    object: &Map<String, Value>,
    keys: &[&str],
    index: usize,
    field: &str,
) -> Result<String> {
    optional_text(object, keys).with_context(|| format!("message row {index} lacks {field}"))
}

fn required_scalar(
    object: &Map<String, Value>,
    keys: &[&str],
    index: usize,
    field: &str,
) -> Result<String> {
    optional_scalar(object, keys).with_context(|| format!("row {index} lacks {field}"))
}

fn optional_text(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match object.get(*key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        _ => None,
    })
}

fn optional_scalar(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match object.get(*key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        _ => None,
    })
}

fn optional_bool(object: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|key| match object.get(*key) {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Number(value)) => value.as_i64().map(|value| value != 0),
        _ => None,
    })
}

fn optional_u64(object: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| match object.get(*key) {
        Some(Value::Number(value)) => value.as_u64(),
        Some(Value::String(value)) => value.parse().ok(),
        _ => None,
    })
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

    use super::{ChatRef, IMessageExtractor, parse_history, parse_json_values};
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
            let args: Vec<String> = args
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect();
            let output = match args.first().map(String::as_str) {
                Some("chats") => {
                    b"{\"id\":7,\"name\":\"Room\",\"last_message_at\":\"2026-07-01T01:02:03Z\"}\n".to_vec()
                }
                Some("history") => b"{\"id\":9,\"chat_id\":7,\"sender\":\"person@example.test\",\"is_from_me\":false,\"text\":\"hello\",\"created_at\":\"2026-07-01T01:02:03Z\",\"attachments\":[]}\n".to_vec(),
                _ => panic!("unexpected source subcommand"),
            };
            self.calls.lock().unwrap().push(args);
            Ok(ProcessOutput { stdout: output })
        }
    }

    #[test]
    fn accepts_ndjson_and_json_arrays() {
        assert_eq!(
            parse_json_values(b"{\"id\":1}\n{\"id\":2}\n")
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            parse_json_values(br#"[{"id":1},{"id":2}]"#).unwrap().len(),
            2
        );
    }

    #[test]
    fn parses_history_and_discards_attachment_paths() {
        let chat = ChatRef {
            id: "7".to_string(),
            display_name: "Room".to_string(),
        };
        let input = br#"{"id":9,"chat_id":7,"sender":"person@example.test","sender_name":"Person","is_from_me":false,"text":"","created_at":"2026-07-01T01:02:03Z","attachments":[{"mime_type":"image/png","total_bytes":10,"original_path":"/private/secret.png"}]}"#;
        let messages = parse_history(input, &chat).unwrap();
        let message = &messages[0];
        assert_eq!(
            message.timestamp_utc,
            Utc.with_ymd_and_hms(2026, 7, 1, 1, 2, 3).unwrap()
        );
        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].byte_count, Some(10));
        let serialized = serde_json::to_string(message).unwrap();
        assert!(!serialized.contains("secret.png"));
    }

    #[test]
    fn extractor_uses_only_read_only_batch_commands() {
        let runner = Arc::new(FakeRunner {
            calls: Mutex::new(Vec::new()),
        });
        let extractor = IMessageExtractor::new(runner.clone());
        let request = ExtractRequest::for_binary(
            DateRange::new(
                Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap(),
            )
            .unwrap(),
            "imsg".into(),
        );
        assert_eq!(extractor.extract(&request).unwrap().len(), 1);
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0][0], "chats");
        assert_eq!(calls[1][0], "history");
        assert!(calls[1].contains(&"--attachments".to_string()));
        assert!(!calls[1].contains(&"--convert-attachments".to_string()));
    }
}
