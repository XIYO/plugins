use std::{
    collections::HashMap,
    fs::{self, OpenOptions, Permissions},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use tracing::{error, info};

use crate::{
    model::{AliasedMessage, SourceKind},
    optimizer::{MessageAudit, OptimizationOutcome},
};

const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextScope {
    Global,
    Thread(String),
}

impl ContextScope {
    fn database_values(&self) -> (&'static str, &str) {
        match self {
            Self::Global => ("global", "global"),
            Self::Thread(alias) => ("thread", alias),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisContextMetadata {
    pub id: i64,
    pub scope: String,
    pub scope_key: String,
    pub period_start_utc: String,
    pub period_end_utc: String,
    pub model: String,
    pub reasoning_effort: String,
    pub created_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub metadata: AnalysisContextMetadata,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadIdentityMap {
    pub thread_alias: String,
    pub thread_name: String,
    pub source: String,
    pub speakers: Vec<SpeakerIdentity>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeakerIdentity {
    pub alias: String,
    pub display_name: String,
    pub is_self: bool,
}

pub struct StateStore {
    connection: Connection,
    path: Option<PathBuf>,
}

impl StateStore {
    pub fn default_path() -> Result<PathBuf> {
        let project = ProjectDirs::from("dev", "XIYO", "msgpipe")
            .context("unable to resolve the local application data directory")?;
        Ok(project.data_local_dir().join("state.sqlite3"))
    }

    pub fn open(path: &Path) -> Result<Self> {
        info!("[state:sqlite:start] opening protected local state");
        let started = Instant::now();
        if let Err(error_value) = prepare_path(path) {
            error!(
                error = ?error_value,
                "[state:sqlite:failure] state path preparation failed"
            );
            return Err(error_value);
        }
        let connection = Connection::open(path).context("unable to open local state database")?;
        let mut store = Self {
            connection,
            path: Some(path.to_path_buf()),
        };
        if let Err(error_value) = store.migrate() {
            error!(
                error = ?error_value,
                "[state:sqlite:failure] state migration failed"
            );
            return Err(error_value);
        }
        enforce_file_mode(path)?;
        info!(
            duration_ms = started.elapsed().as_millis(),
            "[state:sqlite:success] protected local state opened"
        );
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        let mut store = Self {
            connection,
            path: None,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn register(&mut self, outcome: &OptimizationOutcome) -> Result<Vec<AliasedMessage>> {
        info!(
            profile = %outcome.profile,
            input_count = outcome.input_count,
            output_count = outcome.messages.len(),
            "[state:sqlite:start] registering aliases and message audit"
        );
        let started = Instant::now();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("unable to start state transaction")?;
        let result = register_transaction(&transaction, outcome);
        let aliased = match result {
            Ok(aliased) => aliased,
            Err(error_value) => {
                error!(
                    error = ?error_value,
                    "[state:sqlite:failure] alias registration failed"
                );
                return Err(error_value);
            }
        };
        transaction
            .commit()
            .context("unable to commit state transaction")?;
        if let Some(path) = &self.path {
            enforce_file_mode(path)?;
        }
        info!(
            row_count = aliased.len(),
            duration_ms = started.elapsed().as_millis(),
            "[state:sqlite:success] aliases and audit registered"
        );
        Ok(aliased)
    }

    pub fn save_analysis_context(
        &mut self,
        scope: &ContextScope,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        model: &str,
        reasoning_effort: &str,
        summary: &str,
    ) -> Result<i64> {
        if period_start >= period_end {
            bail!("analysis context start must be earlier than end")
        }
        if model.trim().is_empty() || reasoning_effort.trim().is_empty() {
            bail!("analysis context model and reasoning effort are required")
        }
        if summary.trim().is_empty() {
            bail!("analysis context summary is empty")
        }
        let (scope_name, scope_key) = scope.database_values();
        info!(
            scope = scope_name,
            summary_bytes = summary.len(),
            "[state:context:start] saving derived analysis context"
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if matches!(scope, ContextScope::Thread(_)) {
            let thread_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM source_thread WHERE alias = ?1)",
                [scope_key],
                |row| row.get(0),
            )?;
            if !thread_exists {
                bail!("thread alias does not exist in local state")
            }
        }
        transaction.execute(
            "INSERT INTO analysis_context\n\
             (scope, scope_key, period_start_utc, period_end_utc, model, reasoning_effort,\n\
              summary, created_at_utc)\n\
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                scope_name,
                scope_key,
                period_start.to_rfc3339(),
                period_end.to_rfc3339(),
                model,
                reasoning_effort,
                summary,
                Utc::now().to_rfc3339(),
            ],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        if let Some(path) = &self.path {
            enforce_file_mode(path)?;
        }
        info!(
            scope = scope_name,
            "[state:context:success] derived analysis context saved"
        );
        Ok(id)
    }

    pub fn latest_analysis_context(&self, scope: &ContextScope) -> Result<Option<AnalysisContext>> {
        let (scope_name, scope_key) = scope.database_values();
        info!(
            scope = scope_name,
            "[state:context:start] loading latest derived analysis context"
        );
        let record = self
            .connection
            .query_row(
                "SELECT id, scope, scope_key, period_start_utc, period_end_utc, model,\n\
                        reasoning_effort, created_at_utc, summary\n\
                 FROM analysis_context\n\
                 WHERE scope = ?1 AND scope_key = ?2\n\
                 ORDER BY id DESC LIMIT 1",
                params![scope_name, scope_key],
                |row| {
                    Ok(AnalysisContext {
                        metadata: AnalysisContextMetadata {
                            id: row.get(0)?,
                            scope: row.get(1)?,
                            scope_key: row.get(2)?,
                            period_start_utc: row.get(3)?,
                            period_end_utc: row.get(4)?,
                            model: row.get(5)?,
                            reasoning_effort: row.get(6)?,
                            created_at_utc: row.get(7)?,
                        },
                        summary: row.get(8)?,
                    })
                },
            )
            .optional()?;
        info!(
            scope = scope_name,
            found = record.is_some(),
            "[state:context:success] latest derived analysis context loaded"
        );
        Ok(record)
    }

    pub fn list_analysis_contexts(&self) -> Result<Vec<AnalysisContextMetadata>> {
        info!("[state:context:start] listing derived analysis context metadata");
        let mut statement = self.connection.prepare(
            "SELECT id, scope, scope_key, period_start_utc, period_end_utc, model,\n\
                    reasoning_effort, created_at_utc\n\
             FROM analysis_context ORDER BY id ASC",
        )?;
        let records = statement
            .query_map([], |row| {
                Ok(AnalysisContextMetadata {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    scope_key: row.get(2)?,
                    period_start_utc: row.get(3)?,
                    period_end_utc: row.get(4)?,
                    model: row.get(5)?,
                    reasoning_effort: row.get(6)?,
                    created_at_utc: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        info!(
            row_count = records.len(),
            "[state:context:success] derived analysis context metadata listed"
        );
        Ok(records)
    }

    pub fn identity_map(&self, thread_alias: &str) -> Result<ThreadIdentityMap> {
        info!("[state:identity:start] resolving explicit alias map request");
        let thread: Option<(i64, String, String)> = self
            .connection
            .query_row(
                "SELECT id, display_name, source FROM source_thread WHERE alias = ?1",
                [thread_alias],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (thread_id, thread_name, source) =
            thread.context("thread alias does not exist in local state")?;
        let mut statement = self.connection.prepare(
            "SELECT alias, display_name, is_self FROM speaker\n\
             WHERE thread_id = ?1 ORDER BY LENGTH(alias), alias",
        )?;
        let speakers = statement
            .query_map([thread_id], |row| {
                Ok(SpeakerIdentity {
                    alias: row.get(0)?,
                    display_name: row.get(1)?,
                    is_self: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        info!(
            speaker_count = speakers.len(),
            "[state:identity:success] explicit alias map resolved"
        );
        Ok(ThreadIdentityMap {
            thread_alias: thread_alias.to_string(),
            thread_name,
            source,
            speakers,
        })
    }

    fn migrate(&mut self) -> Result<()> {
        self.connection.execute_batch(
            "PRAGMA foreign_keys = ON;\n\
             PRAGMA journal_mode = DELETE;\n\
             PRAGMA synchronous = FULL;",
        )?;
        let initial_version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if initial_version > SCHEMA_VERSION {
            bail!(
                "state schema version {initial_version} is unsupported; expected at most {SCHEMA_VERSION}"
            )
        }
        if initial_version == 0 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                    "CREATE TABLE source_thread (\n\
                         id INTEGER PRIMARY KEY,\n\
                         source TEXT NOT NULL,\n\
                         source_thread_id TEXT NOT NULL,\n\
                         display_name TEXT NOT NULL,\n\
                         alias TEXT NOT NULL UNIQUE,\n\
                         first_seen_utc TEXT NOT NULL,\n\
                         last_seen_utc TEXT NOT NULL,\n\
                         UNIQUE(source, source_thread_id)\n\
                     );\n\
                     CREATE TABLE speaker (\n\
                         id INTEGER PRIMARY KEY,\n\
                         thread_id INTEGER NOT NULL REFERENCES source_thread(id) ON DELETE CASCADE,\n\
                         source_author_id TEXT NOT NULL,\n\
                         display_name TEXT NOT NULL,\n\
                         alias TEXT NOT NULL,\n\
                         is_self INTEGER NOT NULL CHECK(is_self IN (0, 1)),\n\
                         first_seen_utc TEXT NOT NULL,\n\
                         last_seen_utc TEXT NOT NULL,\n\
                         UNIQUE(thread_id, source_author_id),\n\
                         UNIQUE(thread_id, alias)\n\
                     );\n\
                     CREATE TABLE message_audit (\n\
                         source TEXT NOT NULL,\n\
                         source_message_id TEXT NOT NULL,\n\
                         profile TEXT NOT NULL,\n\
                         thread_id INTEGER NOT NULL REFERENCES source_thread(id) ON DELETE CASCADE,\n\
                         speaker_id INTEGER NOT NULL REFERENCES speaker(id) ON DELETE CASCADE,\n\
                         timestamp_utc TEXT NOT NULL,\n\
                         kind TEXT NOT NULL,\n\
                         content_sha256 TEXT NOT NULL,\n\
                         kept INTEGER NOT NULL CHECK(kept IN (0, 1)),\n\
                         transform_codes TEXT NOT NULL,\n\
                         export_ordinal INTEGER,\n\
                         PRIMARY KEY(source, source_message_id, profile)\n\
                     );\n\
                     CREATE INDEX message_audit_thread_time\n\
                         ON message_audit(thread_id, profile, timestamp_utc);\n\
                     PRAGMA user_version = 1;",
            )?;
            transaction.commit()?;
        }
        let version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 1 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "CREATE TABLE analysis_context (\n\
                     id INTEGER PRIMARY KEY,\n\
                     scope TEXT NOT NULL CHECK(scope IN ('global', 'thread')),\n\
                     scope_key TEXT NOT NULL,\n\
                     period_start_utc TEXT NOT NULL,\n\
                     period_end_utc TEXT NOT NULL,\n\
                     model TEXT NOT NULL,\n\
                     reasoning_effort TEXT NOT NULL,\n\
                     summary TEXT NOT NULL,\n\
                     created_at_utc TEXT NOT NULL\n\
                 );\n\
                 CREATE INDEX analysis_context_scope_latest\n\
                     ON analysis_context(scope, scope_key, id DESC);\n\
                 PRAGMA user_version = 2;",
            )?;
            transaction.commit()?;
        }
        let final_version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if final_version != SCHEMA_VERSION {
            bail!("state schema migration stopped at {final_version}; expected {SCHEMA_VERSION}")
        }
        Ok(())
    }
}

type ThreadKey = (SourceKind, String);
type SpeakerKey = (SourceKind, String, String);

fn register_transaction(
    transaction: &Transaction<'_>,
    outcome: &OptimizationOutcome,
) -> Result<Vec<AliasedMessage>> {
    let mut threads = HashMap::<ThreadKey, (i64, String)>::new();
    let mut speakers = HashMap::<SpeakerKey, (i64, String)>::new();
    let mut ordinals = HashMap::<ThreadKey, i64>::new();

    for audit in &outcome.audits {
        let thread_key = (audit.source, audit.source_thread_id.clone());
        let (thread_id, thread_alias) = if let Some(value) = threads.get(&thread_key) {
            value.clone()
        } else {
            let value = get_or_create_thread(transaction, audit)?;
            threads.insert(thread_key.clone(), value.clone());
            value
        };
        let speaker_key = (
            audit.source,
            audit.source_thread_id.clone(),
            audit.source_author_id.clone(),
        );
        let (speaker_id, speaker_alias) = if let Some(value) = speakers.get(&speaker_key) {
            value.clone()
        } else {
            let value = get_or_create_speaker(transaction, thread_id, audit)?;
            speakers.insert(speaker_key, value.clone());
            value
        };
        let export_ordinal = if audit.kept {
            let ordinal = ordinals.entry(thread_key).or_default();
            *ordinal += 1;
            Some(*ordinal)
        } else {
            None
        };
        upsert_audit(
            transaction,
            outcome,
            audit,
            thread_id,
            speaker_id,
            export_ordinal,
        )?;

        let _ = (thread_alias, speaker_alias);
    }

    outcome
        .messages
        .iter()
        .map(|message| {
            let thread_key = (
                message.original.source,
                message.original.source_thread_id.clone(),
            );
            let speaker_key = (
                message.original.source,
                message.original.source_thread_id.clone(),
                message.original.source_author_id.clone(),
            );
            let thread_alias = threads
                .get(&thread_key)
                .map(|value| value.1.clone())
                .context("thread alias was not registered")?;
            let speaker_alias = speakers
                .get(&speaker_key)
                .map(|value| value.1.clone())
                .context("speaker alias was not registered")?;
            Ok(AliasedMessage {
                message: message.clone(),
                thread_alias,
                speaker_alias,
            })
        })
        .collect()
}

fn get_or_create_thread(
    transaction: &Transaction<'_>,
    audit: &MessageAudit,
) -> Result<(i64, String)> {
    let source = audit.source.as_str();
    let existing: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, alias FROM source_thread WHERE source = ?1 AND source_thread_id = ?2",
            params![source, audit.source_thread_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some(existing) = existing {
        transaction.execute(
            "UPDATE source_thread\n\
             SET display_name = ?1,\n\
                 first_seen_utc = MIN(first_seen_utc, ?2),\n\
                 last_seen_utc = MAX(last_seen_utc, ?2)\n\
             WHERE id = ?3",
            params![
                audit.thread_name,
                audit.timestamp_utc.to_rfc3339(),
                existing.0
            ],
        )?;
        return Ok(existing);
    }

    let maximum: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(CAST(SUBSTR(alias, 2) AS INTEGER)), 0)\n\
         FROM source_thread WHERE source = ?1",
        [source],
        |row| row.get(0),
    )?;
    let alias = format!("{}{:03}", audit.source.alias_prefix(), maximum + 1);
    let timestamp = audit.timestamp_utc.to_rfc3339();
    transaction.execute(
        "INSERT INTO source_thread\n\
         (source, source_thread_id, display_name, alias, first_seen_utc, last_seen_utc)\n\
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![
            source,
            audit.source_thread_id,
            audit.thread_name,
            alias,
            timestamp
        ],
    )?;
    Ok((transaction.last_insert_rowid(), alias))
}

fn get_or_create_speaker(
    transaction: &Transaction<'_>,
    thread_id: i64,
    audit: &MessageAudit,
) -> Result<(i64, String)> {
    let existing: Option<(i64, String)> = transaction
        .query_row(
            "SELECT id, alias FROM speaker WHERE thread_id = ?1 AND source_author_id = ?2",
            params![thread_id, audit.source_author_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some(existing) = existing {
        transaction.execute(
            "UPDATE speaker\n\
             SET display_name = ?1,\n\
                 is_self = MAX(is_self, ?2),\n\
                 first_seen_utc = MIN(first_seen_utc, ?3),\n\
                 last_seen_utc = MAX(last_seen_utc, ?3)\n\
             WHERE id = ?4",
            params![
                audit.author_name,
                audit.is_from_me,
                audit.timestamp_utc.to_rfc3339(),
                existing.0
            ],
        )?;
        return Ok(existing);
    }

    let alias = if audit.is_from_me {
        "A".to_string()
    } else {
        next_speaker_alias(transaction, thread_id)?
    };
    let timestamp = audit.timestamp_utc.to_rfc3339();
    transaction.execute(
        "INSERT INTO speaker\n\
         (thread_id, source_author_id, display_name, alias, is_self, first_seen_utc, last_seen_utc)\n\
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![
            thread_id,
            audit.source_author_id,
            audit.author_name,
            alias,
            audit.is_from_me,
            timestamp
        ],
    )?;
    Ok((transaction.last_insert_rowid(), alias))
}

fn next_speaker_alias(transaction: &Transaction<'_>, thread_id: i64) -> Result<String> {
    let mut index = 1usize;
    loop {
        let alias = alphabetic_alias(index);
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM speaker WHERE thread_id = ?1 AND alias = ?2)",
            params![thread_id, alias],
            |row| row.get(0),
        )?;
        if !exists {
            return Ok(alias);
        }
        index += 1;
    }
}

fn alphabetic_alias(mut index: usize) -> String {
    let mut characters = Vec::new();
    loop {
        characters.push((b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    characters.iter().rev().collect()
}

fn upsert_audit(
    transaction: &Transaction<'_>,
    outcome: &OptimizationOutcome,
    audit: &MessageAudit,
    thread_id: i64,
    speaker_id: i64,
    export_ordinal: Option<i64>,
) -> Result<()> {
    let transform_codes = audit
        .transforms
        .iter()
        .map(|transform| transform.code())
        .collect::<Vec<_>>()
        .join(",");
    transaction.execute(
        "INSERT INTO message_audit\n\
         (source, source_message_id, profile, thread_id, speaker_id, timestamp_utc, kind,\n\
          content_sha256, kept, transform_codes, export_ordinal)\n\
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)\n\
         ON CONFLICT(source, source_message_id, profile) DO UPDATE SET\n\
             thread_id = excluded.thread_id, speaker_id = excluded.speaker_id,\n\
             timestamp_utc = excluded.timestamp_utc, kind = excluded.kind,\n\
             content_sha256 = excluded.content_sha256, kept = excluded.kept,\n\
             transform_codes = excluded.transform_codes, export_ordinal = excluded.export_ordinal",
        params![
            audit.source.as_str(),
            audit.source_message_id,
            outcome.profile.to_string(),
            thread_id,
            speaker_id,
            audit.timestamp_utc.to_rfc3339(),
            audit.kind.to_string(),
            audit.content_sha256,
            audit.kept,
            transform_codes,
            export_ordinal
        ],
    )?;
    Ok(())
}

fn prepare_path(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .context("state database path has no parent directory")?;
    fs::create_dir_all(parent).context("unable to create state directory")?;
    fs::set_permissions(parent, Permissions::from_mode(0o700))
        .context("unable to protect state directory")?;
    if !path.exists() {
        OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .context("unable to create protected state database")?;
    }
    enforce_file_mode(path)
}

fn enforce_file_mode(path: &Path) -> Result<()> {
    fs::set_permissions(path, Permissions::from_mode(0o600))
        .context("unable to protect state database")
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use chrono::{TimeZone, Utc};
    use tempfile::tempdir;

    use super::{StateStore, alphabetic_alias};
    use crate::{
        model::{MessageKind, NormalizedMessage, SourceKind},
        optimizer::{OptimizationProfile, optimize},
    };

    fn messages() -> Vec<NormalizedMessage> {
        vec![
            NormalizedMessage {
                source: SourceKind::KakaoTalk,
                source_message_id: "1".to_string(),
                source_thread_id: "thread".to_string(),
                thread_name: "Room".to_string(),
                source_author_id: "other".to_string(),
                author_name: "Other".to_string(),
                is_from_me: false,
                timestamp_utc: Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
                kind: MessageKind::Text,
                content: "hello".to_string(),
                attachments: Vec::new(),
            },
            NormalizedMessage {
                source: SourceKind::KakaoTalk,
                source_message_id: "2".to_string(),
                source_thread_id: "thread".to_string(),
                thread_name: "Room".to_string(),
                source_author_id: "self".to_string(),
                author_name: "Me".to_string(),
                is_from_me: true,
                timestamp_utc: Utc.with_ymd_and_hms(2026, 7, 1, 0, 1, 0).unwrap(),
                kind: MessageKind::Text,
                content: "hi".to_string(),
                attachments: Vec::new(),
            },
        ]
    }

    #[test]
    fn reserves_a_for_self_even_when_other_speaks_first() {
        let outcome = optimize(&messages(), OptimizationProfile::Schedule).unwrap();
        let mut state = StateStore::open_in_memory().unwrap();
        let aliased = state.register(&outcome).unwrap();
        assert_eq!(aliased[0].speaker_alias, "B");
        assert_eq!(aliased[1].speaker_alias, "A");
        assert_eq!(aliased[0].thread_alias, "K001");
    }

    #[test]
    fn aliases_remain_stable_across_runs() {
        let outcome = optimize(&messages(), OptimizationProfile::Schedule).unwrap();
        let mut state = StateStore::open_in_memory().unwrap();
        let first = state.register(&outcome).unwrap();
        let second = state.register(&outcome).unwrap();
        assert_eq!(first[0].thread_alias, second[0].thread_alias);
        assert_eq!(first[0].speaker_alias, second[0].speaker_alias);
    }

    #[test]
    fn state_path_is_owner_only() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("private").join("state.sqlite3");
        let outcome = optimize(&messages(), OptimizationProfile::Schedule).unwrap();
        let mut state = StateStore::open(&path).unwrap();
        state.register(&outcome).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(directory.path().join("private"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let database_bytes = std::fs::read(&path).unwrap();
        assert!(
            !database_bytes
                .windows(b"hello".len())
                .any(|value| value == b"hello")
        );
        assert!(
            !database_bytes
                .windows(b"hi".len())
                .any(|value| value == b"hi")
        );
    }

    #[test]
    fn alphabetic_aliases_extend_beyond_z() {
        assert_eq!(alphabetic_alias(0), "A");
        assert_eq!(alphabetic_alias(25), "Z");
        assert_eq!(alphabetic_alias(26), "AA");
        assert_eq!(alphabetic_alias(27), "AB");
    }

    #[test]
    fn derived_context_is_append_only_and_scoped_by_alias() {
        let outcome = optimize(&messages(), OptimizationProfile::Schedule).unwrap();
        let mut state = StateStore::open_in_memory().unwrap();
        state.register(&outcome).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap();
        state
            .save_analysis_context(
                &super::ContextScope::Thread("K001".to_string()),
                start,
                end,
                "gpt-5.6-terra",
                "medium",
                "first derived summary",
            )
            .unwrap();
        state
            .save_analysis_context(
                &super::ContextScope::Thread("K001".to_string()),
                start,
                end,
                "gpt-5.6-terra",
                "medium",
                "second derived summary",
            )
            .unwrap();
        let latest = state
            .latest_analysis_context(&super::ContextScope::Thread("K001".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(latest.summary, "second derived summary");
        assert_eq!(state.list_analysis_contexts().unwrap().len(), 2);
        assert!(
            state
                .save_analysis_context(
                    &super::ContextScope::Thread("K999".to_string()),
                    start,
                    end,
                    "gpt-5.6-terra",
                    "medium",
                    "unknown thread",
                )
                .is_err()
        );
    }

    #[test]
    fn explicit_identity_map_omits_source_identifiers() {
        let outcome = optimize(&messages(), OptimizationProfile::Schedule).unwrap();
        let mut state = StateStore::open_in_memory().unwrap();
        state.register(&outcome).unwrap();
        let identity = state.identity_map("K001").unwrap();
        assert_eq!(identity.thread_alias, "K001");
        assert_eq!(identity.speakers[0].alias, "A");
        assert!(identity.speakers[0].is_self);
        let serialized = serde_json::to_string(&identity).unwrap();
        assert!(!serialized.contains("source_author_id"));
        assert!(!serialized.contains("source_thread_id"));
    }
}
