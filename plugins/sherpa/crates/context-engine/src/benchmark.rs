use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::{
    export::{ExportConfig, ExportFormat, render},
    model::{AliasedMessage, SourceKind},
    optimizer::{OptimizationOutcome, TransformKind},
};

#[derive(Debug, Serialize)]
pub struct BenchmarkReport {
    pub source: SourceKind,
    pub input_messages: usize,
    pub optimized_messages: usize,
    pub removed_or_collapsed_messages: usize,
    pub threads: usize,
    pub transform_counts: BTreeMap<String, usize>,
    pub o200k_base_tokens: TokenCounts,
    pub cct_savings_vs_raw_json_percent: f64,
    pub thread_cct_tokens: ThreadDistribution,
    pub thread_manifest: Vec<ThreadManifestEntry>,
}

#[derive(Debug, Serialize)]
pub struct TokenCounts {
    pub raw_content_lines: usize,
    pub raw_compact_json: usize,
    pub optimized_compact_json: usize,
    pub optimized_tsv: usize,
    pub optimized_cct: usize,
}

#[derive(Debug, Serialize)]
pub struct ThreadDistribution {
    pub sum_with_header_per_thread: usize,
    pub median: f64,
    pub p90: usize,
    pub p95: usize,
    pub max: usize,
    pub over_32k: usize,
    pub over_64k: usize,
}

#[derive(Debug, Serialize)]
pub struct ThreadManifestEntry {
    pub thread_alias: String,
    pub messages: usize,
    pub first_at_utc: String,
    pub last_at_utc: String,
    pub cct_tokens: usize,
}

pub fn measure(
    source: SourceKind,
    raw: &[AliasedMessage],
    optimized: &[AliasedMessage],
    outcome: &OptimizationOutcome,
    config: ExportConfig,
) -> Result<BenchmarkReport> {
    let tokenizer =
        tiktoken::get_encoding("o200k_base").context("o200k_base tokenizer is unavailable")?;
    let raw_content = raw
        .iter()
        .map(|message| message.message.content.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let raw_json = render(ExportFormat::Json, raw, config)?;
    let optimized_json = render(ExportFormat::Json, optimized, config)?;
    let optimized_tsv = render(ExportFormat::Tsv, optimized, config)?;
    let optimized_cct = render(ExportFormat::Cct, optimized, config)?;
    let tokens = TokenCounts {
        raw_content_lines: tokenizer.count(&raw_content),
        raw_compact_json: tokenizer.count(&raw_json),
        optimized_compact_json: tokenizer.count(&optimized_json),
        optimized_tsv: tokenizer.count(&optimized_tsv),
        optimized_cct: tokenizer.count(&optimized_cct),
    };

    let mut grouped = BTreeMap::<String, Vec<AliasedMessage>>::new();
    for message in optimized {
        grouped
            .entry(message.thread_alias.clone())
            .or_default()
            .push(message.clone());
    }
    let mut thread_manifest = Vec::with_capacity(grouped.len());
    for (alias, mut messages) in grouped {
        messages.sort_by_key(|message| message.message.original.timestamp_utc);
        let first = messages
            .first()
            .context("thread group unexpectedly has no first message")?;
        let last = messages
            .last()
            .context("thread group unexpectedly has no last message")?;
        thread_manifest.push(ThreadManifestEntry {
            thread_alias: alias,
            messages: messages.len(),
            first_at_utc: first.message.original.timestamp_utc.to_rfc3339(),
            last_at_utc: last.message.original.timestamp_utc.to_rfc3339(),
            cct_tokens: tokenizer.count(&render(ExportFormat::Cct, &messages, config)?),
        });
    }
    let mut thread_tokens: Vec<_> = thread_manifest
        .iter()
        .map(|thread| thread.cct_tokens)
        .collect();
    thread_tokens.sort_unstable();
    let distribution = ThreadDistribution {
        sum_with_header_per_thread: thread_tokens.iter().sum(),
        median: median(&thread_tokens),
        p90: percentile(&thread_tokens, 0.90),
        p95: percentile(&thread_tokens, 0.95),
        max: thread_tokens.last().copied().unwrap_or_default(),
        over_32k: thread_tokens
            .iter()
            .filter(|value| **value > 32_000)
            .count(),
        over_64k: thread_tokens
            .iter()
            .filter(|value| **value > 64_000)
            .count(),
    };
    let savings = if tokens.raw_compact_json == 0 {
        0.0
    } else {
        (1.0 - tokens.optimized_cct as f64 / tokens.raw_compact_json as f64) * 100.0
    };
    let selected_source_threads: BTreeSet<_> = optimized
        .iter()
        .map(|message| message.message.original.source_thread_id.as_str())
        .collect();
    let mut transform_counts = BTreeMap::<String, usize>::new();
    for audit in outcome
        .audits
        .iter()
        .filter(|audit| selected_source_threads.contains(audit.source_thread_id.as_str()))
    {
        for transform in &audit.transforms {
            *transform_counts
                .entry(transform_name(*transform).to_string())
                .or_default() += 1;
        }
    }
    Ok(BenchmarkReport {
        source,
        input_messages: raw.len(),
        optimized_messages: optimized.len(),
        removed_or_collapsed_messages: raw.len().saturating_sub(optimized.len()),
        threads: thread_manifest.len(),
        transform_counts,
        o200k_base_tokens: tokens,
        cct_savings_vs_raw_json_percent: (savings * 100.0).round() / 100.0,
        thread_cct_tokens: distribution,
        thread_manifest,
    })
}

fn transform_name(kind: TransformKind) -> &'static str {
    kind.code()
}

fn percentile(values: &[usize], fraction: f64) -> usize {
    if values.is_empty() {
        return 0;
    }
    let index = ((values.len() - 1) as f64 * fraction).floor() as usize;
    values[index]
}

fn median(values: &[usize]) -> f64 {
    match values.len() {
        0 => 0.0,
        length if length % 2 == 1 => values[length / 2] as f64,
        length => (values[length / 2 - 1] + values[length / 2]) as f64 / 2.0,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rust_o200k_matches_python_reference_for_korean_chat() {
        let tokenizer = tiktoken::get_encoding("o200k_base").unwrap();
        assert_eq!(tokenizer.count("ㅋㅋㅋㅋ 내일 오후 3시 가능해?"), 11);
    }
}
