use anyhow::{Result, bail};
use chrono::Timelike;

use crate::{model::AliasedMessage, optimizer::OptimizationProfile};

use super::{ExportConfig, sorted_messages};

pub fn render_cct(messages: &[AliasedMessage], config: ExportConfig) -> Result<String> {
    if config.session_gap_minutes == 0 {
        bail!("session gap must be greater than zero")
    }
    match config.profile {
        OptimizationProfile::Exact => Ok(render_cct2(messages, config)),
        OptimizationProfile::Schedule => Ok(render_cct3(messages, config)),
    }
}

fn render_cct2(messages: &[AliasedMessage], config: ExportConfig) -> String {
    let mut output = format!("!CCT2|z={}\n", config.timezone.name());
    let mut previous_thread = String::new();
    let mut previous_day = String::new();
    let mut previous_time = String::new();
    let mut previous_speaker = String::new();

    for item in sorted_messages(messages) {
        let local = item
            .message
            .original
            .timestamp_utc
            .with_timezone(&config.timezone);
        if item.thread_alias != previous_thread {
            output.push_str("T|");
            output.push_str(&escape_field(&item.thread_alias));
            output.push('\n');
            previous_thread.clone_from(&item.thread_alias);
            previous_day.clear();
            previous_time.clear();
            previous_speaker.clear();
        }
        let day = local.format("%y%m%d").to_string();
        if day != previous_day {
            output.push_str("D|");
            output.push_str(&day);
            output.push('\n');
            previous_day = day;
            previous_time.clear();
            previous_speaker.clear();
        }
        let time = format!("{:02}{:02}", local.hour(), local.minute());
        if time != previous_time {
            output.push_str(&time);
        }
        output.push('|');
        if item.speaker_alias != previous_speaker {
            output.push_str(&escape_field(&item.speaker_alias));
        }
        output.push('|');
        output.push_str(&escape_field(&item.message.content));
        output.push('\n');
        previous_time = time;
        previous_speaker.clone_from(&item.speaker_alias);
    }
    output
}

fn render_cct3(messages: &[AliasedMessage], config: ExportConfig) -> String {
    let mut output = format!(
        "!CCT3|g={}|z={}\n",
        config.session_gap_minutes,
        config.timezone.name()
    );
    let mut previous_thread = String::new();
    let mut previous_day = String::new();
    let mut previous_speaker = String::new();
    let mut previous_timestamp: Option<chrono::DateTime<chrono::Utc>> = None;

    for item in sorted_messages(messages) {
        let timestamp = item.message.original.timestamp_utc;
        let local = timestamp.with_timezone(&config.timezone);
        if item.thread_alias != previous_thread {
            output.push_str("T|");
            output.push_str(&escape_field(&item.thread_alias));
            output.push('\n');
            previous_thread.clone_from(&item.thread_alias);
            previous_day.clear();
            previous_speaker.clear();
            previous_timestamp = None;
        }
        let day = local.format("%y%m%d").to_string();
        if day != previous_day {
            output.push_str("D|");
            output.push_str(&day);
            output.push('\n');
            previous_day = day;
            previous_speaker.clear();
            previous_timestamp = None;
        }
        let begins_session = previous_timestamp.is_none_or(|previous| {
            (timestamp - previous).num_minutes() > i64::from(config.session_gap_minutes)
        });
        if begins_session {
            output.push_str("S|");
            output.push_str(&format!("{:02}{:02}\n", local.hour(), local.minute()));
            previous_speaker.clear();
        }
        if item.speaker_alias != previous_speaker {
            output.push_str(&escape_field(&item.speaker_alias));
        }
        output.push('|');
        output.push_str(&escape_field(&item.message.content));
        output.push('\n');
        previous_speaker.clone_from(&item.speaker_alias);
        previous_timestamp = Some(timestamp);
    }
    output
}

pub fn escape_field(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '|' => output.push_str("\\|"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            character => output.push(character),
        }
    }
    output
}

pub fn unescape_field(value: &str) -> Result<String> {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars();
    while let Some(character) = characters.next() {
        if character != '\\' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('\\') => output.push('\\'),
            Some('|') => output.push('|'),
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some(other) => bail!("unknown CCT escape: \\{other}"),
            None => bail!("unterminated CCT escape"),
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use chrono_tz::Asia::Seoul;

    use super::{escape_field, render_cct, unescape_field};
    use crate::{
        export::ExportConfig,
        model::{AliasedMessage, MessageKind, NormalizedMessage, SourceKind},
        optimizer::{OptimizationProfile, OptimizedMessage},
    };

    fn aliased(id: &str, minute: u32, speaker: &str, content: &str) -> AliasedMessage {
        AliasedMessage {
            message: OptimizedMessage {
                original: NormalizedMessage {
                    source: SourceKind::KakaoTalk,
                    source_message_id: id.to_string(),
                    source_thread_id: "1".to_string(),
                    thread_name: "Hidden".to_string(),
                    source_author_id: speaker.to_string(),
                    author_name: "Hidden".to_string(),
                    is_from_me: speaker == "A",
                    timestamp_utc: Utc.with_ymd_and_hms(2026, 7, 22, 0, minute, 0).unwrap(),
                    kind: MessageKind::Text,
                    content: content.to_string(),
                    attachments: Vec::new(),
                },
                content: content.to_string(),
                transforms: Vec::new(),
            },
            thread_alias: "K001".to_string(),
            speaker_alias: speaker.to_string(),
        }
    }

    #[test]
    fn field_escaping_round_trips() {
        let input = "a|b\\c\nd\r";
        assert_eq!(unescape_field(&escape_field(input)).unwrap(), input);
        assert!(unescape_field("bad\\x").is_err());
        assert!(unescape_field("bad\\").is_err());
    }

    #[test]
    fn cct3_inherits_speaker_and_starts_new_session() {
        let messages = vec![
            aliased("1", 0, "A", "one"),
            aliased("2", 1, "A", "two"),
            aliased("3", 40, "B", "three"),
        ];
        let rendered = render_cct(
            &messages,
            ExportConfig {
                profile: OptimizationProfile::Schedule,
                timezone: Seoul,
                session_gap_minutes: 30,
            },
        )
        .unwrap();
        assert_eq!(
            rendered,
            "!CCT3|g=30|z=Asia/Seoul\nT|K001\nD|260722\nS|0900\nA|one\n|two\nS|0940\nB|three\n"
        );
        assert!(!rendered.contains("Hidden"));
    }

    #[test]
    fn cct2_preserves_minute_changes() {
        let messages = vec![aliased("1", 0, "A", "one"), aliased("2", 1, "A", "two")];
        let rendered = render_cct(
            &messages,
            ExportConfig {
                profile: OptimizationProfile::Exact,
                timezone: Seoul,
                session_gap_minutes: 30,
            },
        )
        .unwrap();
        assert!(rendered.contains("0900|A|one\n0901||two\n"));
    }
}
