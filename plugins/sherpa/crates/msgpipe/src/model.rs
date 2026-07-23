use std::{fmt, str::FromStr};

use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKind {
    #[serde(rename = "kakaotalk")]
    KakaoTalk,
    #[serde(rename = "imessage")]
    IMessage,
}

impl SourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KakaoTalk => "kakaotalk",
            Self::IMessage => "imessage",
        }
    }

    pub const fn alias_prefix(self) -> char {
        match self {
            Self::KakaoTalk => 'K',
            Self::IMessage => 'I',
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for SourceKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "kakao" | "kakaotalk" => Ok(Self::KakaoTalk),
            "imessage" | "imsg" => Ok(Self::IMessage),
            _ => bail!("unsupported source; expected kakao or imessage"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageKind {
    Text,
    Image,
    Video,
    Audio,
    File,
    Sticker,
    Location,
    Link,
    System,
    Unknown,
}

impl MessageKind {
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Text => "@text",
            Self::Image => "@image",
            Self::Video => "@video",
            Self::Audio => "@audio",
            Self::File => "@file",
            Self::Sticker => "@sticker",
            Self::Location => "@location",
            Self::Link => "@link",
            Self::System => "@system",
            Self::Unknown => "@attachment",
        }
    }
}

impl fmt::Display for MessageKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::File => "file",
            Self::Sticker => "sticker",
            Self::Location => "location",
            Self::Link => "link",
            Self::System => "system",
            Self::Unknown => "unknown",
        };
        formatter.write_str(value)
    }
}

impl FromStr for MessageKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            "file" => Ok(Self::File),
            "sticker" => Ok(Self::Sticker),
            "location" => Ok(Self::Location),
            "link" => Ok(Self::Link),
            "system" => Ok(Self::System),
            "unknown" => Ok(Self::Unknown),
            _ => bail!("unsupported archived message kind"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub kind: MessageKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_count: Option<u64>,
    pub is_sticker: bool,
    pub is_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedMessage {
    pub source: SourceKind,
    pub source_message_id: String,
    pub source_thread_id: String,
    pub thread_name: String,
    pub source_author_id: String,
    pub author_name: String,
    pub is_from_me: bool,
    pub timestamp_utc: DateTime<Utc>,
    pub kind: MessageKind,
    pub content: String,
    pub attachments: Vec<AttachmentMeta>,
}

impl NormalizedMessage {
    pub fn validate(&self) -> Result<()> {
        if self.source_message_id.is_empty() {
            bail!("source message id is empty")
        }
        if self.source_thread_id.is_empty() {
            bail!("source thread id is empty")
        }
        if self.source_author_id.is_empty() {
            bail!("source author id is empty")
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AliasedMessage {
    #[serde(skip)]
    pub message: crate::optimizer::OptimizedMessage,
    pub thread_alias: String,
    pub speaker_alias: String,
}

pub fn sort_messages(messages: &mut [NormalizedMessage]) {
    messages.sort_by(|left, right| {
        left.source
            .as_str()
            .cmp(right.source.as_str())
            .then_with(|| left.source_thread_id.cmp(&right.source_thread_id))
            .then_with(|| left.timestamp_utc.cmp(&right.timestamp_utc))
            .then_with(|| left.source_message_id.cmp(&right.source_message_id))
    });
}
