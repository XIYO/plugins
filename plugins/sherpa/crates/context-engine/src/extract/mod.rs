mod imessage;
mod kakao;

use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};

use crate::{
    command::{CommandRunner, SystemCommandRunner},
    model::{NormalizedMessage, SourceKind, sort_messages},
    time_range::DateRange,
};

pub use imessage::IMessageExtractor;
pub use kakao::KakaoExtractor;

#[derive(Debug, Clone)]
pub struct ExtractRequest {
    pub range: DateRange,
    pub binary: PathBuf,
    pub chat_limit: usize,
    pub message_limit_per_chat: usize,
}

impl ExtractRequest {
    pub fn for_binary(range: DateRange, binary: PathBuf) -> Self {
        Self {
            range,
            binary,
            chat_limit: 10_000,
            message_limit_per_chat: 1_000_000,
        }
    }
}

pub trait Extractor {
    fn extract(&self, request: &ExtractRequest) -> Result<Vec<NormalizedMessage>>;
}

pub fn extract_source(
    source: SourceKind,
    request: &ExtractRequest,
) -> Result<Vec<NormalizedMessage>> {
    let runner: Arc<dyn CommandRunner> = Arc::new(SystemCommandRunner);
    let mut messages = match source {
        SourceKind::KakaoTalk => KakaoExtractor::new(runner).extract(request),
        SourceKind::IMessage => IMessageExtractor::new(runner).extract(request),
    }
    .with_context(|| format!("{} extraction failed", source.as_str()))?;
    sort_messages(&mut messages);
    Ok(messages)
}

pub fn resolve_binary(explicit: Option<PathBuf>, program: &str) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path);
    }
    which::which(program).with_context(|| format!("{program} was not found on PATH"))
}
