mod replacer;
mod structure;

use std::{collections::BTreeMap, fmt};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::model::{MessageKind, NormalizedMessage};

use self::{replacer::Replacement, structure::is_consecutive_duplicate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationProfile {
    Exact,
    Schedule,
}

impl fmt::Display for OptimizationProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Exact => "exact",
            Self::Schedule => "schedule",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransformKind {
    UnicodeNormalized,
    WhitespaceCollapsed,
    EmptyDropped,
    EmptyMarked,
    LaughterOnlyDropped,
    CryOnlyDropped,
    SymbolOnlyDropped,
    AckReplaced,
    NoReplaced,
    QuestionReplaced,
    SymbolAckReplaced,
    SymbolNoReplaced,
    SymbolQuestionReplaced,
    UrlShortened,
    RepetitionCollapsed,
    AttachmentMarkerAdded,
    ConsecutiveDuplicateDropped,
}

impl TransformKind {
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnicodeNormalized => "nfc",
            Self::WhitespaceCollapsed => "ws",
            Self::EmptyDropped => "drop_empty",
            Self::EmptyMarked => "mark_empty",
            Self::LaughterOnlyDropped => "drop_laugh",
            Self::CryOnlyDropped => "drop_cry",
            Self::SymbolOnlyDropped => "drop_symbol",
            Self::AckReplaced => "ack_y",
            Self::NoReplaced => "no_n",
            Self::QuestionReplaced => "question",
            Self::SymbolAckReplaced => "symbol_y",
            Self::SymbolNoReplaced => "symbol_n",
            Self::SymbolQuestionReplaced => "symbol_question",
            Self::UrlShortened => "url",
            Self::RepetitionCollapsed => "repeat",
            Self::AttachmentMarkerAdded => "attachment",
            Self::ConsecutiveDuplicateDropped => "drop_duplicate",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizedMessage {
    pub original: NormalizedMessage,
    pub content: String,
    pub transforms: Vec<TransformKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageAudit {
    pub source: crate::model::SourceKind,
    pub source_message_id: String,
    pub source_thread_id: String,
    pub thread_name: String,
    pub source_author_id: String,
    pub author_name: String,
    pub is_from_me: bool,
    pub timestamp_utc: chrono::DateTime<chrono::Utc>,
    pub kind: MessageKind,
    pub content_sha256: String,
    pub kept: bool,
    pub transforms: Vec<TransformKind>,
}

impl MessageAudit {
    fn from_message(
        message: &NormalizedMessage,
        kept: bool,
        transforms: Vec<TransformKind>,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(message.content.as_bytes());
        Self {
            source: message.source,
            source_message_id: message.source_message_id.clone(),
            source_thread_id: message.source_thread_id.clone(),
            thread_name: message.thread_name.clone(),
            source_author_id: message.source_author_id.clone(),
            author_name: message.author_name.clone(),
            is_from_me: message.is_from_me,
            timestamp_utc: message.timestamp_utc,
            kind: message.kind,
            content_sha256: hex::encode(digest.finalize()),
            kept,
            transforms,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationOutcome {
    pub profile: OptimizationProfile,
    pub messages: Vec<OptimizedMessage>,
    pub audits: Vec<MessageAudit>,
    pub transform_counts: BTreeMap<TransformKind, usize>,
    pub input_count: usize,
}

impl OptimizationOutcome {
    pub fn removed_count(&self) -> usize {
        self.input_count.saturating_sub(self.messages.len())
    }
}

pub fn optimize(
    messages: &[NormalizedMessage],
    profile: OptimizationProfile,
) -> Result<OptimizationOutcome> {
    let mut optimized = Vec::<OptimizedMessage>::new();
    let mut audits = Vec::with_capacity(messages.len());
    let mut counts = BTreeMap::new();

    for message in messages {
        message.validate()?;
        let (content, mut transforms) = match replacer::replace(message, profile) {
            Replacement::Keep {
                content,
                transforms,
            } => (content, transforms),
            Replacement::Drop { transforms } => {
                increment_counts(&mut counts, &transforms);
                audits.push(MessageAudit::from_message(message, false, transforms));
                continue;
            }
        };

        let is_duplicate = profile == OptimizationProfile::Schedule
            && optimized
                .last()
                .is_some_and(|previous| is_consecutive_duplicate(previous, message, &content));
        if is_duplicate {
            transforms.push(TransformKind::ConsecutiveDuplicateDropped);
            increment_counts(&mut counts, &transforms);
            audits.push(MessageAudit::from_message(message, false, transforms));
            continue;
        }

        increment_counts(&mut counts, &transforms);
        audits.push(MessageAudit::from_message(
            message,
            true,
            transforms.clone(),
        ));
        optimized.push(OptimizedMessage {
            original: message.clone(),
            content,
            transforms,
        });
    }

    Ok(OptimizationOutcome {
        profile,
        messages: optimized,
        audits,
        transform_counts: counts,
        input_count: messages.len(),
    })
}

fn increment_counts(counts: &mut BTreeMap<TransformKind, usize>, transforms: &[TransformKind]) {
    for transform in transforms {
        *counts.entry(*transform).or_default() += 1;
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::{OptimizationProfile, TransformKind, optimize};
    use crate::model::{AttachmentMeta, MessageKind, NormalizedMessage, SourceKind};

    fn message(id: &str, minute: u32, content: &str) -> NormalizedMessage {
        NormalizedMessage {
            source: SourceKind::IMessage,
            source_message_id: id.to_string(),
            source_thread_id: "thread".to_string(),
            thread_name: "Room".to_string(),
            source_author_id: "author".to_string(),
            author_name: "Person".to_string(),
            is_from_me: false,
            timestamp_utc: Utc.with_ymd_and_hms(2026, 7, 1, 0, minute, 0).unwrap(),
            kind: MessageKind::Text,
            content: content.to_string(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn schedule_drops_noise_and_compacts_reactions() {
        let input = vec![message("1", 0, "ㅋㅋㅋㅋ"), message("2", 1, "넵!!")];
        let outcome = optimize(&input, OptimizationProfile::Schedule).unwrap();
        assert_eq!(outcome.messages.len(), 1);
        assert_eq!(outcome.messages[0].content, "Y");
        assert_eq!(
            outcome.transform_counts[&TransformKind::LaughterOnlyDropped],
            1
        );
        assert_eq!(outcome.transform_counts[&TransformKind::AckReplaced], 1);
    }

    #[test]
    fn schedule_shortens_urls_and_repeated_graphemes() {
        let input = vec![message(
            "1",
            0,
            "여기요!!!! https://www.example.com/a/b?tracking=1",
        )];
        let outcome = optimize(&input, OptimizationProfile::Schedule).unwrap();
        assert_eq!(outcome.messages[0].content, "여기요! U:example.com");
    }

    #[test]
    fn attachment_only_message_is_marked_instead_of_dropped() {
        let mut input = message("1", 0, "\u{fffc}");
        input.kind = MessageKind::Image;
        input.attachments.push(AttachmentMeta {
            kind: MessageKind::Image,
            mime_type: Some("image/png".to_string()),
            byte_count: Some(10),
            is_sticker: false,
            is_missing: false,
        });
        let outcome = optimize(&[input], OptimizationProfile::Schedule).unwrap();
        assert_eq!(outcome.messages[0].content, "@image");
    }

    #[test]
    fn only_consecutive_duplicates_within_five_minutes_are_removed() {
        let input = vec![
            message("1", 0, "확인했습니다"),
            message("2", 4, "확인했습니다"),
            message("3", 10, "확인했습니다"),
        ];
        let outcome = optimize(&input, OptimizationProfile::Schedule).unwrap();
        assert_eq!(outcome.messages.len(), 2);
        assert_eq!(
            outcome.transform_counts[&TransformKind::ConsecutiveDuplicateDropped],
            1
        );
    }
}
