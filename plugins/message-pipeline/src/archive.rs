use std::str::FromStr;

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    model::{AttachmentMeta, MessageKind, NormalizedMessage, SourceKind},
    time_range::DateRange,
};

#[derive(Debug, Clone)]
pub struct ArchiveQuery {
    pub source: SourceKind,
    pub range: DateRange,
    pub thread_alias: Option<String>,
    pub pending_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveSyncReport {
    pub source: SourceKind,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub extracted_messages: usize,
    pub inserted_messages: usize,
    pub updated_messages: usize,
    pub unchanged_messages: usize,
    pub completed_at_utc: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadArchiveStatus {
    pub thread_alias: String,
    pub source: String,
    pub archived_messages: i64,
    pub pending_messages: i64,
    pub first_message_at_utc: Option<String>,
    pub last_message_at_utc: Option<String>,
    pub last_ingested_at_utc: Option<String>,
    pub last_presented_at_utc: Option<String>,
    pub last_analyzed_at_utc: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AnalysisCoverage {
    pub message_count: i64,
    pub message_set_sha256: String,
}

pub(crate) fn ingest(
    transaction: &Transaction<'_>,
    source: SourceKind,
    range: DateRange,
    messages: &[NormalizedMessage],
) -> Result<ArchiveSyncReport> {
    let completed_at = Utc::now();
    let completed_at_text = completed_at.to_rfc3339();
    let mut inserted = 0usize;
    let mut updated = 0usize;
    let mut unchanged = 0usize;

    for message in messages {
        if message.source != source {
            bail!("archive batch contains a message from a different source")
        }
        message.validate()?;
        let (thread_id, speaker_id): (i64, i64) = transaction
            .query_row(
                "SELECT st.id, sp.id\n\
                 FROM source_thread st\n\
                 JOIN speaker sp ON sp.thread_id = st.id\n\
                 WHERE st.source = ?1 AND st.source_thread_id = ?2\n\
                   AND sp.source_author_id = ?3",
                params![
                    source.as_str(),
                    message.source_thread_id,
                    message.source_author_id
                ],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .context("message identity was not registered before archive ingestion")?;
        let attachments_json = serde_json::to_string(&message.attachments)?;
        let content_sha256 = sha256(message.content.as_bytes());
        let timestamp = message.timestamp_utc.to_rfc3339();
        let kind = message.kind.to_string();
        let existing: Option<(i64, i64, String, String, String, String)> = transaction
            .query_row(
                "SELECT thread_id, speaker_id, timestamp_utc, kind, content_sha256, attachments_json\n\
                 FROM archived_message WHERE source = ?1 AND source_message_id = ?2",
                params![source.as_str(), message.source_message_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()?;

        match existing {
            None => {
                transaction.execute(
                    "INSERT INTO archived_message\n\
                     (source, source_message_id, thread_id, speaker_id, timestamp_utc, kind,\n\
                      content, attachments_json, content_sha256, first_ingested_at_utc,\n\
                      last_ingested_at_utc)\n\
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                    params![
                        source.as_str(),
                        message.source_message_id,
                        thread_id,
                        speaker_id,
                        timestamp,
                        kind,
                        message.content,
                        attachments_json,
                        content_sha256,
                        completed_at_text,
                    ],
                )?;
                inserted += 1;
            }
            Some(existing) => {
                let changed = existing.0 != thread_id
                    || existing.1 != speaker_id
                    || existing.2 != timestamp
                    || existing.3 != kind
                    || existing.4 != content_sha256
                    || existing.5 != attachments_json;
                if changed {
                    transaction.execute(
                        "UPDATE archived_message SET\n\
                             thread_id = ?1, speaker_id = ?2, timestamp_utc = ?3, kind = ?4,\n\
                             content = ?5, attachments_json = ?6, content_sha256 = ?7,\n\
                             last_ingested_at_utc = ?8, last_presented_at_utc = NULL,\n\
                             analyzed_at_utc = NULL, analysis_context_id = NULL\n\
                         WHERE source = ?9 AND source_message_id = ?10",
                        params![
                            thread_id,
                            speaker_id,
                            timestamp,
                            kind,
                            message.content,
                            attachments_json,
                            content_sha256,
                            completed_at_text,
                            source.as_str(),
                            message.source_message_id,
                        ],
                    )?;
                    updated += 1;
                } else {
                    transaction.execute(
                        "UPDATE archived_message SET last_ingested_at_utc = ?1\n\
                         WHERE source = ?2 AND source_message_id = ?3",
                        params![
                            completed_at_text,
                            source.as_str(),
                            message.source_message_id
                        ],
                    )?;
                    unchanged += 1;
                }
            }
        }
    }

    transaction.execute(
        "INSERT INTO source_sync\n\
         (source, period_start_utc, period_end_utc, extracted_messages, inserted_messages,\n\
          updated_messages, unchanged_messages, completed_at_utc)\n\
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            source.as_str(),
            range.start.to_rfc3339(),
            range.end.to_rfc3339(),
            messages.len() as i64,
            inserted as i64,
            updated as i64,
            unchanged as i64,
            completed_at_text,
        ],
    )?;

    Ok(ArchiveSyncReport {
        source,
        period_start_utc: range.start.to_rfc3339(),
        period_end_utc: range.end.to_rfc3339(),
        extracted_messages: messages.len(),
        inserted_messages: inserted,
        updated_messages: updated,
        unchanged_messages: unchanged,
        completed_at_utc: completed_at_text,
    })
}

pub(crate) fn load(
    connection: &Connection,
    query: &ArchiveQuery,
) -> Result<Vec<NormalizedMessage>> {
    let mut statement = connection.prepare(
        "SELECT am.source, am.source_message_id, st.source_thread_id, st.display_name,\n\
                sp.source_author_id, sp.display_name, sp.is_self, am.timestamp_utc, am.kind,\n\
                am.content, am.attachments_json\n\
         FROM archived_message am\n\
         JOIN source_thread st ON st.id = am.thread_id\n\
         JOIN speaker sp ON sp.id = am.speaker_id\n\
         WHERE am.source = ?1 AND am.timestamp_utc >= ?2 AND am.timestamp_utc < ?3\n\
           AND (?4 IS NULL OR st.alias = ?4)\n\
           AND (?5 = 0 OR am.analysis_context_id IS NULL)\n\
         ORDER BY st.source_thread_id, am.timestamp_utc, am.source_message_id",
    )?;
    let rows = statement
        .query_map(
            params![
                query.source.as_str(),
                query.range.start.to_rfc3339(),
                query.range.end.to_rfc3339(),
                query.thread_alias.as_deref(),
                query.pending_only,
            ],
            |row| {
                Ok(ArchivedRow {
                    source: row.get(0)?,
                    source_message_id: row.get(1)?,
                    source_thread_id: row.get(2)?,
                    thread_name: row.get(3)?,
                    source_author_id: row.get(4)?,
                    author_name: row.get(5)?,
                    is_from_me: row.get(6)?,
                    timestamp_utc: row.get(7)?,
                    kind: row.get(8)?,
                    content: row.get(9)?,
                    attachments_json: row.get(10)?,
                })
            },
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    rows.into_iter().map(ArchivedRow::normalize).collect()
}

pub(crate) fn mark_presented(
    transaction: &Transaction<'_>,
    messages: &[NormalizedMessage],
) -> Result<()> {
    let presented_at = Utc::now().to_rfc3339();
    for message in messages {
        transaction.execute(
            "UPDATE archived_message SET last_presented_at_utc = ?1\n\
             WHERE source = ?2 AND source_message_id = ?3",
            params![
                presented_at,
                message.source.as_str(),
                message.source_message_id
            ],
        )?;
    }
    Ok(())
}

pub(crate) fn pending_coverage(
    transaction: &Transaction<'_>,
    thread_alias: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
) -> Result<AnalysisCoverage> {
    let mut statement = transaction.prepare(
        "SELECT am.source_message_id, am.content_sha256\n\
         FROM archived_message am\n\
         JOIN source_thread st ON st.id = am.thread_id\n\
         WHERE st.alias = ?1 AND am.timestamp_utc >= ?2 AND am.timestamp_utc < ?3\n\
           AND am.analysis_context_id IS NULL AND am.last_presented_at_utc IS NOT NULL\n\
         ORDER BY am.timestamp_utc, am.source_message_id",
    )?;
    let rows = statement
        .query_map(
            params![
                thread_alias,
                period_start.to_rfc3339(),
                period_end.to_rfc3339()
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if rows.is_empty() {
        bail!("thread period has no pending archived messages presented for analysis")
    }
    let mut digest = Sha256::new();
    for (message_id, content_sha256) in &rows {
        digest.update(message_id.as_bytes());
        digest.update([0]);
        digest.update(content_sha256.as_bytes());
        digest.update([0xff]);
    }
    Ok(AnalysisCoverage {
        message_count: rows.len() as i64,
        message_set_sha256: hex::encode(digest.finalize()),
    })
}

pub(crate) fn mark_analyzed(
    transaction: &Transaction<'_>,
    thread_alias: &str,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    context_id: i64,
    analyzed_at: &str,
) -> Result<usize> {
    let changed = transaction.execute(
        "UPDATE archived_message SET analyzed_at_utc = ?1, analysis_context_id = ?2\n\
         WHERE thread_id = (SELECT id FROM source_thread WHERE alias = ?3)\n\
           AND timestamp_utc >= ?4 AND timestamp_utc < ?5\n\
           AND analysis_context_id IS NULL AND last_presented_at_utc IS NOT NULL",
        params![
            analyzed_at,
            context_id,
            thread_alias,
            period_start.to_rfc3339(),
            period_end.to_rfc3339(),
        ],
    )?;
    Ok(changed)
}

pub(crate) fn status(
    connection: &Connection,
    source: Option<SourceKind>,
    thread_alias: Option<&str>,
) -> Result<Vec<ThreadArchiveStatus>> {
    let mut statement = connection.prepare(
        "SELECT st.alias, st.source, COUNT(am.id),\n\
                COALESCE(SUM(CASE WHEN am.analysis_context_id IS NULL THEN 1 ELSE 0 END), 0),\n\
                MIN(am.timestamp_utc), MAX(am.timestamp_utc), MAX(am.last_ingested_at_utc),\n\
                MAX(am.last_presented_at_utc), MAX(am.analyzed_at_utc)\n\
         FROM source_thread st\n\
         LEFT JOIN archived_message am ON am.thread_id = st.id\n\
         WHERE (?1 IS NULL OR st.source = ?1) AND (?2 IS NULL OR st.alias = ?2)\n\
         GROUP BY st.id, st.alias, st.source\n\
         HAVING COUNT(am.id) > 0\n\
         ORDER BY st.alias",
    )?;
    let source_name = source.map(|value| value.as_str());
    Ok(statement
        .query_map(params![source_name, thread_alias], |row| {
            Ok(ThreadArchiveStatus {
                thread_alias: row.get(0)?,
                source: row.get(1)?,
                archived_messages: row.get(2)?,
                pending_messages: row.get(3)?,
                first_message_at_utc: row.get(4)?,
                last_message_at_utc: row.get(5)?,
                last_ingested_at_utc: row.get(6)?,
                last_presented_at_utc: row.get(7)?,
                last_analyzed_at_utc: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

struct ArchivedRow {
    source: String,
    source_message_id: String,
    source_thread_id: String,
    thread_name: String,
    source_author_id: String,
    author_name: String,
    is_from_me: bool,
    timestamp_utc: String,
    kind: String,
    content: String,
    attachments_json: String,
}

impl ArchivedRow {
    fn normalize(self) -> Result<NormalizedMessage> {
        let message = NormalizedMessage {
            source: SourceKind::from_str(&self.source)?,
            source_message_id: self.source_message_id,
            source_thread_id: self.source_thread_id,
            thread_name: self.thread_name,
            source_author_id: self.source_author_id,
            author_name: self.author_name,
            is_from_me: self.is_from_me,
            timestamp_utc: DateTime::parse_from_rfc3339(&self.timestamp_utc)
                .context("archived message timestamp is invalid")?
                .with_timezone(&Utc),
            kind: MessageKind::from_str(&self.kind)?,
            content: self.content,
            attachments: serde_json::from_str::<Vec<AttachmentMeta>>(&self.attachments_json)
                .context("archived attachment metadata is invalid")?,
        };
        message.validate()?;
        Ok(message)
    }
}

fn sha256(value: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(value);
    hex::encode(digest.finalize())
}
