use std::sync::OnceLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;
use url::Url;

use crate::model::{MessageKind, NormalizedMessage};

use super::{OptimizationProfile, TransformKind};

pub(super) enum Replacement {
    Keep {
        content: String,
        transforms: Vec<TransformKind>,
    },
    Drop {
        transforms: Vec<TransformKind>,
    },
}

pub(super) fn replace(message: &NormalizedMessage, profile: OptimizationProfile) -> Replacement {
    match profile {
        OptimizationProfile::Exact => replace_exact(message),
        OptimizationProfile::Schedule => replace_schedule(message),
    }
}

fn replace_exact(message: &NormalizedMessage) -> Replacement {
    let normalized: String = message.content.nfc().collect();
    let mut transforms = Vec::new();
    if normalized != message.content {
        transforms.push(TransformKind::UnicodeNormalized);
    }
    let (content, marker_added, empty_marked) = append_attachment_markers(message, normalized);
    if empty_marked {
        transforms.push(TransformKind::EmptyMarked);
    }
    if marker_added {
        transforms.push(TransformKind::AttachmentMarkerAdded);
    }
    Replacement::Keep {
        content,
        transforms,
    }
}

fn replace_schedule(message: &NormalizedMessage) -> Replacement {
    let mut transforms = Vec::new();
    let nfc: String = message.content.nfc().collect();
    if nfc != message.content {
        transforms.push(TransformKind::UnicodeNormalized);
    }
    let mut content = whitespace_regex().replace_all(&nfc, " ").trim().to_string();
    if content != nfc {
        transforms.push(TransformKind::WhitespaceCollapsed);
    }

    if content.is_empty() || content == "\u{fffc}" {
        if message.attachments.is_empty() && message.kind == MessageKind::Text {
            transforms.push(TransformKind::EmptyDropped);
            return Replacement::Drop { transforms };
        }
        let (marked, marker_added, _) = append_attachment_markers(message, String::new());
        if marker_added {
            transforms.push(TransformKind::AttachmentMarkerAdded);
        }
        return Replacement::Keep {
            content: marked,
            transforms,
        };
    }
    if laughter_regex().is_match(&content) {
        transforms.push(TransformKind::LaughterOnlyDropped);
        return Replacement::Drop { transforms };
    }
    if cry_regex().is_match(&content) {
        transforms.push(TransformKind::CryOnlyDropped);
        return Replacement::Drop { transforms };
    }
    if ack_regex().is_match(&content) {
        content = "Y".to_string();
        transforms.push(TransformKind::AckReplaced);
    } else if no_regex().is_match(&content) {
        content = "N".to_string();
        transforms.push(TransformKind::NoReplaced);
    } else if question_regex().is_match(&content) {
        content = "?".to_string();
        transforms.push(TransformKind::QuestionReplaced);
    } else if symbol_regex().is_match(&content) {
        let symbols: String = content
            .chars()
            .filter(|value| !value.is_whitespace())
            .collect();
        if symbols.chars().any(|value| YES_SYMBOLS.contains(value)) {
            content = "Y".to_string();
            transforms.push(TransformKind::SymbolAckReplaced);
        } else if symbols.chars().any(|value| NO_SYMBOLS.contains(value)) {
            content = "N".to_string();
            transforms.push(TransformKind::SymbolNoReplaced);
        } else if symbols
            .chars()
            .any(|value| QUESTION_SYMBOLS.contains(value))
        {
            content = "?".to_string();
            transforms.push(TransformKind::SymbolQuestionReplaced);
        } else {
            transforms.push(TransformKind::SymbolOnlyDropped);
            return Replacement::Drop { transforms };
        }
    } else {
        let shortened = shorten_urls(&content);
        if shortened != content {
            content = shortened;
            transforms.push(TransformKind::UrlShortened);
        }
        let collapsed = collapse_repetitions(&content);
        if collapsed != content {
            content = collapsed;
            transforms.push(TransformKind::RepetitionCollapsed);
        }
    }

    let (content, marker_added, _) = append_attachment_markers(message, content);
    if marker_added {
        transforms.push(TransformKind::AttachmentMarkerAdded);
    }
    Replacement::Keep {
        content,
        transforms,
    }
}

fn append_attachment_markers(message: &NormalizedMessage, content: String) -> (String, bool, bool) {
    let mut markers = Vec::<&str>::new();
    for attachment in &message.attachments {
        let marker = attachment.kind.marker();
        if !markers.contains(&marker) {
            markers.push(marker);
        }
    }
    if markers.is_empty() && message.kind != MessageKind::Text {
        markers.push(message.kind.marker());
    }
    if markers.is_empty() {
        if content.is_empty() {
            return ("@empty".to_string(), false, true);
        }
        return (content, false, false);
    }
    let marker_text = markers.join(" ");
    if content.trim().is_empty() || content == "\u{fffc}" {
        return (marker_text, true, false);
    }
    (format!("{content} {marker_text}"), true, false)
}

fn shorten_urls(content: &str) -> String {
    url_regex()
        .replace_all(content, |captures: &regex::Captures<'_>| {
            let matched = captures.get(0).map_or("", |value| value.as_str());
            let split = matched.trim_end_matches(['.', ',', '!', '?', ')', ']', '}']);
            let trailing = &matched[split.len()..];
            let with_scheme = if split.starts_with("www.") {
                format!("https://{split}")
            } else {
                split.to_string()
            };
            Url::parse(&with_scheme)
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
                .map(|host| format!("U:{}{trailing}", host.strip_prefix("www.").unwrap_or(&host)))
                .unwrap_or_else(|| format!("U{trailing}"))
        })
        .into_owned()
}

fn collapse_repetitions(content: &str) -> String {
    let mut output = String::new();
    let mut previous: Option<&str> = None;
    for grapheme in content.graphemes(true) {
        let repeated = previous == Some(grapheme);
        let collapsible =
            matches!(grapheme, "ㅋ" | "ㅎ" | "ㅠ" | "ㅜ") || symbol_regex().is_match(grapheme);
        if !(repeated && collapsible) {
            output.push_str(grapheme);
        }
        previous = Some(grapheme);
    }
    output
}

const YES_SYMBOLS: &str = "👍👌🙆✅☑⭕🆗🤝👏🙏";
const NO_SYMBOLS: &str = "👎🙅❌🚫";
const QUESTION_SYMBOLS: &str = "❓❔🤔";

fn whitespace_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"\s+").expect("valid whitespace regex"))
}

fn laughter_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^[ㅋㅎ하허헤호히흐\s~!?.…]+$").expect("valid laughter regex"))
}

fn cry_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^[ㅠㅜ\s~!?.…]+$").expect("valid cry regex"))
}

fn ack_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(
            r"(?i)^(?:ㅇㅇ+|ㅇㅋ+|오케이|오키|ok(?:ay)?|네+|넵+|넹+|응+|어+|알겠(?:어|습니다)?|확인|좋아|좋습니다|굿|콜)[~!?.…]*$",
        )
        .expect("valid acknowledgement regex")
    })
}

fn no_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"(?i)^(?:ㄴㄴ+|아니+|안\s*돼|안됨|노|no)[~!?.…]*$")
            .expect("valid negative regex")
    })
}

fn question_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"^[?？❓❔]+$").expect("valid question regex"))
}

fn symbol_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| {
        Regex::new(r"^[\p{P}\p{S}\p{M}\p{Z}\u{200D}\u{FE0F}]+$").expect("valid symbol regex")
    })
}

fn url_regex() -> &'static Regex {
    static VALUE: OnceLock<Regex> = OnceLock::new();
    VALUE.get_or_init(|| Regex::new(r"(?i)(?:https?://|www\.)[^\s<>]+").expect("valid URL regex"))
}
