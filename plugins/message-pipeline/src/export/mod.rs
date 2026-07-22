mod cct;
mod json;
mod tsv;

use std::{fmt, str::FromStr, time::Instant};

use anyhow::{Result, bail};
use chrono_tz::Tz;
use tracing::info;

use crate::{model::AliasedMessage, optimizer::OptimizationProfile};

pub use cct::{escape_field, render_cct, unescape_field};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Cct,
    Tsv,
    Json,
}

impl fmt::Display for ExportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Cct => "cct",
            Self::Tsv => "tsv",
            Self::Json => "json",
        })
    }
}

impl FromStr for ExportFormat {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "cct" => Ok(Self::Cct),
            "tsv" => Ok(Self::Tsv),
            "json" => Ok(Self::Json),
            _ => bail!("unsupported format; expected cct, tsv, or json"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ExportConfig {
    pub profile: OptimizationProfile,
    pub timezone: Tz,
    pub session_gap_minutes: u32,
}

pub fn render(
    format: ExportFormat,
    messages: &[AliasedMessage],
    config: ExportConfig,
) -> Result<String> {
    info!(
        format = %format,
        profile = %config.profile,
        row_count = messages.len(),
        "[export:render:start] rendering aliased messages"
    );
    let started = Instant::now();
    let rendered = match format {
        ExportFormat::Cct => cct::render_cct(messages, config),
        ExportFormat::Tsv => Ok(tsv::render_tsv(messages, config.timezone)),
        ExportFormat::Json => json::render_json(messages, config.timezone),
    }?;
    info!(
        format = %format,
        output_bytes = rendered.len(),
        duration_ms = started.elapsed().as_millis(),
        "[export:render:success] aliased messages rendered"
    );
    Ok(rendered)
}

pub(crate) fn sorted_messages(messages: &[AliasedMessage]) -> Vec<&AliasedMessage> {
    let mut sorted: Vec<_> = messages.iter().collect();
    sorted.sort_by(|left, right| {
        left.thread_alias
            .cmp(&right.thread_alias)
            .then_with(|| {
                left.message
                    .original
                    .timestamp_utc
                    .cmp(&right.message.original.timestamp_utc)
            })
            .then_with(|| {
                left.message
                    .original
                    .source_message_id
                    .cmp(&right.message.original.source_message_id)
            })
    });
    sorted
}
