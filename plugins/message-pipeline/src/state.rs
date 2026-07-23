use std::{
    collections::HashMap,
    fs::{self, DirBuilder, OpenOptions, Permissions},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
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
    archive::{self, ArchiveQuery, ArchiveSyncReport, ThreadArchiveStatus},
    model::{AliasedMessage, NormalizedMessage, SourceKind},
    optimizer::{MessageAudit, OptimizationOutcome, OptimizationProfile, optimize},
    time_range::DateRange,
};

const SCHEMA_VERSION: i64 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextScope {
    Global,
    Thread(String),
    Session(String),
}

impl ContextScope {
    fn database_values(&self) -> (&'static str, &str) {
        match self {
            Self::Global => ("global", "global"),
            Self::Thread(alias) => ("thread", alias),
            Self::Session(alias) => ("session", alias),
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
    pub message_count: Option<i64>,
    pub message_set_sha256: Option<String>,
    pub input_context_count: Option<i64>,
    pub input_context_max_id: Option<i64>,
    pub rolled_up_by_context_id: Option<i64>,
    pub created_at_utc: String,
}

#[derive(Debug, Clone)]
pub struct AnalysisContext {
    pub metadata: AnalysisContextMetadata,
    pub summary: String,
}

#[derive(Debug, Clone, Copy)]
pub struct AnalysisContextDraft<'a> {
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub model: &'a str,
    pub reasoning_effort: &'a str,
    pub summary: &'a str,
    pub input_context_max_id: Option<i64>,
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

#[derive(Debug, Clone, Serialize)]
pub struct PurgeReport {
    pub deleted_files: usize,
}

pub struct StateStore {
    connection: Connection,
    path: Option<PathBuf>,
}

impl StateStore {
    pub fn default_path() -> Result<PathBuf> {
        let project = ProjectDirs::from("dev", "XIYO", "sherpa")
            .context("unable to resolve the local application data directory")?;
        let canonical = project.data_local_dir().join("context/state.sqlite3");
        let legacy = ProjectDirs::from("dev", "XIYO", "msgpipe")
            .context("unable to resolve the legacy local application data directory")?
            .data_local_dir()
            .join("state.sqlite3");
        if !canonical.exists() && legacy.exists() {
            return Ok(legacy);
        }
        Ok(canonical)
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

    pub fn purge(path: &Path) -> Result<PurgeReport> {
        info!("[state:purge:start] deleting protected local state");
        let parent = path
            .parent()
            .context("state database path has no parent directory")?;
        validate_private_directory(parent)?;

        let mut deleted_files = 0;
        for candidate in [
            path.to_path_buf(),
            sidecar_path(path, "-wal"),
            sidecar_path(path, "-shm"),
            sidecar_path(path, "-journal"),
        ] {
            let metadata = match fs::symlink_metadata(&candidate) {
                Ok(metadata) => metadata,
                Err(error_value) if error_value.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error_value) => {
                    return Err(error_value).context("unable to inspect state file for deletion");
                }
            };
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("refusing to delete a state path that is not a regular file")
            }
            fs::remove_file(&candidate).context("unable to delete protected state file")?;
            deleted_files += 1;
        }
        info!(
            deleted_files,
            "[state:purge:success] protected local state deleted"
        );
        Ok(PurgeReport { deleted_files })
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

    pub fn archive_messages(
        &mut self,
        source: SourceKind,
        range: DateRange,
        messages: &[NormalizedMessage],
    ) -> Result<ArchiveSyncReport> {
        info!(
            source = %source,
            row_count = messages.len(),
            "[state:archive:start] storing normalized source messages"
        );
        let started = Instant::now();
        let exact = optimize(messages, OptimizationProfile::Exact)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .context("unable to start archive transaction")?;
        if let Err(error_value) = register_transaction(&transaction, &exact) {
            error!(
                error = ?error_value,
                "[state:archive:failure] identity registration failed"
            );
            return Err(error_value);
        }
        let report = match archive::ingest(&transaction, source, range, messages) {
            Ok(report) => report,
            Err(error_value) => {
                error!(
                    error = ?error_value,
                    "[state:archive:failure] message archive ingestion failed"
                );
                return Err(error_value);
            }
        };
        transaction
            .commit()
            .context("unable to commit archive transaction")?;
        if let Some(path) = &self.path {
            enforce_file_mode(path)?;
        }
        info!(
            source = %source,
            inserted_count = report.inserted_messages,
            updated_count = report.updated_messages,
            unchanged_count = report.unchanged_messages,
            duration_ms = started.elapsed().as_millis(),
            "[state:archive:success] normalized source messages stored"
        );
        Ok(report)
    }

    pub fn load_archived_messages(&self, query: &ArchiveQuery) -> Result<Vec<NormalizedMessage>> {
        info!(
            source = %query.source,
            pending_only = query.pending_only,
            "[state:archive:start] loading archived messages"
        );
        let started = Instant::now();
        let messages = archive::load(&self.connection, query)?;
        info!(
            source = %query.source,
            row_count = messages.len(),
            duration_ms = started.elapsed().as_millis(),
            "[state:archive:success] archived messages loaded"
        );
        Ok(messages)
    }

    pub fn mark_presented(&mut self, messages: &[NormalizedMessage]) -> Result<()> {
        info!(
            row_count = messages.len(),
            "[state:archive:start] recording transcript presentation"
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        archive::mark_presented(&transaction, messages)?;
        transaction.commit()?;
        if let Some(path) = &self.path {
            enforce_file_mode(path)?;
        }
        info!(
            row_count = messages.len(),
            "[state:archive:success] transcript presentation recorded"
        );
        Ok(())
    }

    pub fn archive_status(
        &self,
        source: Option<SourceKind>,
        thread_alias: Option<&str>,
    ) -> Result<Vec<ThreadArchiveStatus>> {
        info!("[state:archive:start] loading archive status");
        let records = archive::status(&self.connection, source, thread_alias)?;
        info!(
            row_count = records.len(),
            "[state:archive:success] archive status loaded"
        );
        Ok(records)
    }

    pub fn save_analysis_context(
        &mut self,
        scope: &ContextScope,
        draft: AnalysisContextDraft<'_>,
    ) -> Result<i64> {
        let AnalysisContextDraft {
            period_start,
            period_end,
            model,
            reasoning_effort,
            summary,
            input_context_max_id,
        } = draft;
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
        if !matches!(scope, ContextScope::Global) {
            let thread_exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM source_thread WHERE alias = ?1)",
                [scope_key],
                |row| row.get(0),
            )?;
            if !thread_exists {
                bail!("thread alias does not exist in local state")
            }
        }
        let coverage = match scope {
            ContextScope::Session(_) => {
                if input_context_max_id.is_some() {
                    bail!("--through-context-id is not valid for session context")
                }
                Some(archive::pending_coverage(
                    &transaction,
                    scope_key,
                    period_start,
                    period_end,
                )?)
            }
            ContextScope::Global | ContextScope::Thread(_) => None,
        };
        let rollup_input_ids = match scope {
            ContextScope::Session(_) => Vec::new(),
            ContextScope::Global | ContextScope::Thread(_) => {
                let through = input_context_max_id
                    .context("--through-context-id is required for thread and global rollups")?;
                pending_rollup_input_ids(&transaction, scope, through)?
            }
        };
        let created_at = Utc::now().to_rfc3339();
        transaction.execute(
            "INSERT INTO analysis_context\n\
             (scope, scope_key, period_start_utc, period_end_utc, model, reasoning_effort,\n\
              summary, message_count, message_set_sha256, input_context_count,\n\
              input_context_max_id, created_at_utc)\n\
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                scope_name,
                scope_key,
                period_start.to_rfc3339(),
                period_end.to_rfc3339(),
                model,
                reasoning_effort,
                summary,
                coverage.as_ref().map(|value| value.message_count),
                coverage
                    .as_ref()
                    .map(|value| value.message_set_sha256.as_str()),
                (!rollup_input_ids.is_empty()).then_some(rollup_input_ids.len() as i64),
                input_context_max_id,
                created_at,
            ],
        )?;
        let id = transaction.last_insert_rowid();
        if let Some(coverage) = coverage {
            let marked = archive::mark_analyzed(
                &transaction,
                scope_key,
                period_start,
                period_end,
                id,
                &created_at,
            )?;
            if marked as i64 != coverage.message_count {
                bail!("analysis coverage changed before the summary could be committed")
            }
        }
        for input_id in rollup_input_ids {
            let changed = transaction.execute(
                "UPDATE analysis_context SET rolled_up_by_context_id = ?1\n\
                 WHERE id = ?2 AND rolled_up_by_context_id IS NULL",
                params![id, input_id],
            )?;
            if changed != 1 {
                bail!("rollup input changed before the summary could be committed")
            }
        }
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
                        reasoning_effort, message_count, message_set_sha256, input_context_count,\n\
                        input_context_max_id, rolled_up_by_context_id, created_at_utc, summary\n\
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
                            message_count: row.get(7)?,
                            message_set_sha256: row.get(8)?,
                            input_context_count: row.get(9)?,
                            input_context_max_id: row.get(10)?,
                            rolled_up_by_context_id: row.get(11)?,
                            created_at_utc: row.get(12)?,
                        },
                        summary: row.get(13)?,
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
                    reasoning_effort, message_count, message_set_sha256, input_context_count,\n\
                    input_context_max_id, rolled_up_by_context_id, created_at_utc\n\
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
                    message_count: row.get(7)?,
                    message_set_sha256: row.get(8)?,
                    input_context_count: row.get(9)?,
                    input_context_max_id: row.get(10)?,
                    rolled_up_by_context_id: row.get(11)?,
                    created_at_utc: row.get(12)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        info!(
            row_count = records.len(),
            "[state:context:success] derived analysis context metadata listed"
        );
        Ok(records)
    }

    pub fn pending_rollup_contexts(&self, scope: &ContextScope) -> Result<Vec<AnalysisContext>> {
        info!("[state:context:start] loading pending rollup inputs");
        let contexts = match scope {
            ContextScope::Thread(alias) => {
                load_pending_rollup_contexts(&self.connection, "session", Some(alias.as_str()))?
            }
            ContextScope::Global => load_pending_rollup_contexts(&self.connection, "thread", None)?,
            ContextScope::Session(_) => bail!("session context cannot consume rollup inputs"),
        };
        info!(
            row_count = contexts.len(),
            "[state:context:success] pending rollup inputs loaded"
        );
        Ok(contexts)
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
        let version: i64 = self
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version == 2 {
            let transaction = self.connection.transaction()?;
            transaction.execute_batch(
                "DROP INDEX analysis_context_scope_latest;\n\
                 ALTER TABLE analysis_context RENAME TO analysis_context_v2;\n\
                 CREATE TABLE analysis_context (\n\
                     id INTEGER PRIMARY KEY,\n\
                     scope TEXT NOT NULL CHECK(scope IN ('global', 'thread', 'session')),\n\
                     scope_key TEXT NOT NULL,\n\
                     period_start_utc TEXT NOT NULL,\n\
                     period_end_utc TEXT NOT NULL,\n\
                     model TEXT NOT NULL,\n\
                     reasoning_effort TEXT NOT NULL,\n\
                     summary TEXT NOT NULL,\n\
                     message_count INTEGER,\n\
                     message_set_sha256 TEXT,\n\
                     input_context_count INTEGER,\n\
                     input_context_max_id INTEGER,\n\
                     rolled_up_by_context_id INTEGER REFERENCES analysis_context(id) ON DELETE SET NULL,\n\
                     created_at_utc TEXT NOT NULL\n\
                 );\n\
                 INSERT INTO analysis_context\n\
                     (id, scope, scope_key, period_start_utc, period_end_utc, model,\n\
                      reasoning_effort, summary, created_at_utc)\n\
                 SELECT id, scope, scope_key, period_start_utc, period_end_utc, model,\n\
                        reasoning_effort, summary, created_at_utc\n\
                 FROM analysis_context_v2;\n\
                 DROP TABLE analysis_context_v2;\n\
                 CREATE INDEX analysis_context_scope_latest\n\
                     ON analysis_context(scope, scope_key, id DESC);\n\
                 CREATE INDEX analysis_context_pending_rollup\n\
                     ON analysis_context(scope, scope_key, rolled_up_by_context_id, id);\n\
                 CREATE TABLE archived_message (\n\
                     id INTEGER PRIMARY KEY,\n\
                     source TEXT NOT NULL,\n\
                     source_message_id TEXT NOT NULL,\n\
                     thread_id INTEGER NOT NULL REFERENCES source_thread(id) ON DELETE CASCADE,\n\
                     speaker_id INTEGER NOT NULL REFERENCES speaker(id) ON DELETE CASCADE,\n\
                     timestamp_utc TEXT NOT NULL,\n\
                     kind TEXT NOT NULL,\n\
                     content TEXT NOT NULL,\n\
                     attachments_json TEXT NOT NULL,\n\
                     content_sha256 TEXT NOT NULL,\n\
                     first_ingested_at_utc TEXT NOT NULL,\n\
                     last_ingested_at_utc TEXT NOT NULL,\n\
                     last_presented_at_utc TEXT,\n\
                     analyzed_at_utc TEXT,\n\
                     analysis_context_id INTEGER REFERENCES analysis_context(id) ON DELETE SET NULL,\n\
                     UNIQUE(source, source_message_id)\n\
                 );\n\
                 CREATE INDEX archived_message_thread_pending_time\n\
                     ON archived_message(thread_id, analysis_context_id, timestamp_utc, source_message_id);\n\
                 CREATE TABLE source_sync (\n\
                     id INTEGER PRIMARY KEY,\n\
                     source TEXT NOT NULL,\n\
                     period_start_utc TEXT NOT NULL,\n\
                     period_end_utc TEXT NOT NULL,\n\
                     extracted_messages INTEGER NOT NULL,\n\
                     inserted_messages INTEGER NOT NULL,\n\
                     updated_messages INTEGER NOT NULL,\n\
                     unchanged_messages INTEGER NOT NULL,\n\
                     completed_at_utc TEXT NOT NULL\n\
                 );\n\
                 CREATE INDEX source_sync_latest ON source_sync(source, id DESC);\n\
                 PRAGMA user_version = 3;",
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

fn pending_rollup_input_ids(
    transaction: &Transaction<'_>,
    scope: &ContextScope,
    through_context_id: i64,
) -> Result<Vec<i64>> {
    if through_context_id <= 0 {
        bail!("--through-context-id must be greater than zero")
    }
    let ids = match scope {
        ContextScope::Thread(alias) => {
            let mut statement = transaction.prepare(
                "SELECT id FROM analysis_context\n\
                 WHERE scope = 'session' AND scope_key = ?1\n\
                   AND rolled_up_by_context_id IS NULL AND id <= ?2\n\
                 ORDER BY id ASC",
            )?;
            statement
                .query_map(params![alias, through_context_id], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        }
        ContextScope::Global => {
            let mut statement = transaction.prepare(
                "SELECT id FROM analysis_context\n\
                 WHERE scope = 'thread' AND rolled_up_by_context_id IS NULL AND id <= ?1\n\
                 ORDER BY id ASC",
            )?;
            statement
                .query_map([through_context_id], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        }
        ContextScope::Session(_) => bail!("session context cannot consume rollup inputs"),
    };
    if ids.last().copied() != Some(through_context_id) {
        bail!("rollup input watermark is stale or invalid; reload context inputs")
    }
    Ok(ids)
}

fn load_pending_rollup_contexts(
    connection: &Connection,
    input_scope: &str,
    scope_key: Option<&str>,
) -> Result<Vec<AnalysisContext>> {
    let mut statement = connection.prepare(
        "SELECT id, scope, scope_key, period_start_utc, period_end_utc, model,\n\
                reasoning_effort, message_count, message_set_sha256, input_context_count,\n\
                input_context_max_id, rolled_up_by_context_id, created_at_utc, summary\n\
         FROM analysis_context\n\
         WHERE scope = ?1 AND (?2 IS NULL OR scope_key = ?2)\n\
           AND rolled_up_by_context_id IS NULL\n\
           AND (?1 != 'thread' OR id = (\n\
               SELECT MAX(newer.id) FROM analysis_context newer\n\
               WHERE newer.scope = 'thread'\n\
                 AND newer.scope_key = analysis_context.scope_key\n\
                 AND newer.rolled_up_by_context_id IS NULL\n\
           ))\n\
         ORDER BY id ASC",
    )?;
    Ok(statement
        .query_map(params![input_scope, scope_key], |row| {
            Ok(AnalysisContext {
                metadata: AnalysisContextMetadata {
                    id: row.get(0)?,
                    scope: row.get(1)?,
                    scope_key: row.get(2)?,
                    period_start_utc: row.get(3)?,
                    period_end_utc: row.get(4)?,
                    model: row.get(5)?,
                    reasoning_effort: row.get(6)?,
                    message_count: row.get(7)?,
                    message_set_sha256: row.get(8)?,
                    input_context_count: row.get(9)?,
                    input_context_max_id: row.get(10)?,
                    rolled_up_by_context_id: row.get(11)?,
                    created_at_utc: row.get(12)?,
                },
                summary: row.get(13)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?)
}

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
    if !parent.exists() {
        let mut builder = DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder
            .create(parent)
            .context("unable to create private state directory")?;
    }

    validate_private_directory(parent)?;

    if path.exists() {
        let file_metadata =
            fs::symlink_metadata(path).context("unable to inspect state database")?;
        if file_metadata.file_type().is_symlink() || !file_metadata.is_file() {
            bail!("state database must be a regular file, not a symlink or directory")
        }
    }
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

fn validate_private_directory(parent: &Path) -> Result<()> {
    let parent_metadata =
        fs::symlink_metadata(parent).context("unable to inspect state directory permissions")?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        bail!("state directory must be a real directory, not a symlink or file")
    }
    let parent_mode = parent_metadata.permissions().mode() & 0o777;
    if parent_mode & 0o077 != 0 {
        bail!(
            "state directory permissions are {parent_mode:o}; use a dedicated owner-only directory (0700)"
        )
    }
    Ok(())
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
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

    use super::{AnalysisContextDraft, StateStore, alphabetic_alias};
    use crate::{
        archive::ArchiveQuery,
        model::{MessageKind, NormalizedMessage, SourceKind},
        optimizer::{OptimizationProfile, optimize},
        time_range::DateRange,
    };

    fn range() -> DateRange {
        DateRange::new(
            Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap(),
            Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap(),
        )
        .unwrap()
    }

    fn context_draft<'a>(
        start: chrono::DateTime<Utc>,
        end: chrono::DateTime<Utc>,
        summary: &'a str,
    ) -> AnalysisContextDraft<'a> {
        AnalysisContextDraft {
            period_start: start,
            period_end: end,
            model: "gpt-5.6-terra",
            reasoning_effort: "medium",
            summary,
            input_context_max_id: None,
        }
    }

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
        let mut state = StateStore::open(&path).unwrap();
        state
            .archive_messages(SourceKind::KakaoTalk, range(), &messages())
            .unwrap();
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
        let archived = state
            .load_archived_messages(&ArchiveQuery {
                source: SourceKind::KakaoTalk,
                range: range(),
                thread_alias: None,
                pending_only: false,
            })
            .unwrap();
        assert_eq!(
            archived
                .iter()
                .map(|message| message.content.as_str())
                .collect::<Vec<_>>(),
            ["hello", "hi"]
        );
    }

    #[test]
    fn refuses_shared_parent_without_changing_its_permissions() {
        let directory = tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = directory.path().join("state.sqlite3");

        let error = match StateStore::open(&path) {
            Ok(_) => panic!("공유 디렉터리를 허용하면 안 됨"),
            Err(error_value) => error_value,
        };

        assert!(error.to_string().contains("owner-only directory"));
        assert!(!path.exists());
        assert_eq!(
            std::fs::metadata(directory.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn purge_deletes_only_the_database_and_sqlite_sidecars() {
        let directory = tempdir().unwrap();
        let private = directory.path().join("private");
        let path = private.join("state.sqlite3");
        let unrelated = private.join("keep.txt");
        {
            let _state = StateStore::open(&path).unwrap();
        }
        std::fs::write(format!("{}-wal", path.display()), "wal").unwrap();
        std::fs::write(format!("{}-shm", path.display()), "shm").unwrap();
        std::fs::write(format!("{}-journal", path.display()), "journal").unwrap();
        std::fs::write(&unrelated, "keep").unwrap();

        let report = StateStore::purge(&path).unwrap();

        assert_eq!(report.deleted_files, 4);
        assert!(!path.exists());
        assert!(!std::path::Path::new(&format!("{}-wal", path.display())).exists());
        assert!(!std::path::Path::new(&format!("{}-shm", path.display())).exists());
        assert!(!std::path::Path::new(&format!("{}-journal", path.display())).exists());
        assert_eq!(std::fs::read_to_string(unrelated).unwrap(), "keep");
    }

    #[test]
    fn migrates_v2_contexts_and_accepts_session_scope() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("private").join("state.sqlite3");
        {
            let state = StateStore::open(&path).unwrap();
            state
                .connection
                .execute_batch(
                    "DROP TABLE source_sync;
                     DROP TABLE archived_message;
                     DROP INDEX analysis_context_scope_latest;
                     DROP TABLE analysis_context;
                     CREATE TABLE analysis_context (
                         id INTEGER PRIMARY KEY,
                         scope TEXT NOT NULL CHECK(scope IN ('global', 'thread')),
                         scope_key TEXT NOT NULL,
                         period_start_utc TEXT NOT NULL,
                         period_end_utc TEXT NOT NULL,
                         model TEXT NOT NULL,
                         reasoning_effort TEXT NOT NULL,
                         summary TEXT NOT NULL,
                         created_at_utc TEXT NOT NULL
                     );
                     CREATE INDEX analysis_context_scope_latest
                         ON analysis_context(scope, scope_key, id DESC);
                     INSERT INTO analysis_context
                         (scope, scope_key, period_start_utc, period_end_utc, model,
                          reasoning_effort, summary, created_at_utc)
                     VALUES
                         ('global', 'global', '2026-06-01T00:00:00+00:00',
                          '2026-07-01T00:00:00+00:00', 'gpt-5.6-terra', 'medium',
                          'existing global rollup', '2026-07-01T00:00:00+00:00');
                     PRAGMA user_version = 2;",
                )
                .unwrap();
        }

        let mut state = StateStore::open(&path).unwrap();
        let version: i64 = state
            .connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
        let context = state
            .latest_analysis_context(&super::ContextScope::Global)
            .unwrap()
            .unwrap();
        assert_eq!(context.summary, "existing global rollup");
        assert_eq!(context.metadata.message_count, None);

        state
            .archive_messages(SourceKind::KakaoTalk, range(), &messages())
            .unwrap();
        let query = ArchiveQuery {
            source: SourceKind::KakaoTalk,
            range: range(),
            thread_alias: Some("K001".to_string()),
            pending_only: true,
        };
        let pending = state.load_archived_messages(&query).unwrap();
        state.mark_presented(&pending).unwrap();
        state
            .save_analysis_context(
                &super::ContextScope::Session("K001".to_string()),
                context_draft(range().start, range().end, "migrated session summary"),
            )
            .unwrap();
        assert!(state.load_archived_messages(&query).unwrap().is_empty());
    }

    #[test]
    fn alphabetic_aliases_extend_beyond_z() {
        assert_eq!(alphabetic_alias(0), "A");
        assert_eq!(alphabetic_alias(25), "Z");
        assert_eq!(alphabetic_alias(26), "AA");
        assert_eq!(alphabetic_alias(27), "AB");
    }

    #[test]
    fn session_context_tracks_coverage_and_thread_context_is_a_rollup() {
        let mut state = StateStore::open_in_memory().unwrap();
        state
            .archive_messages(SourceKind::KakaoTalk, range(), &messages())
            .unwrap();
        let query = ArchiveQuery {
            source: SourceKind::KakaoTalk,
            range: range(),
            thread_alias: Some("K001".to_string()),
            pending_only: true,
        };
        let pending = state.load_archived_messages(&query).unwrap();
        state.mark_presented(&pending).unwrap();
        let start = Utc.with_ymd_and_hms(2026, 7, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 7, 2, 0, 0, 0).unwrap();
        let first_session_id = state
            .save_analysis_context(
                &super::ContextScope::Session("K001".to_string()),
                context_draft(start, end, "first derived summary"),
            )
            .unwrap();
        let inputs = state
            .pending_rollup_contexts(&super::ContextScope::Thread("K001".to_string()))
            .unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].metadata.id, first_session_id);
        assert!(
            state
                .save_analysis_context(
                    &super::ContextScope::Session("K001".to_string()),
                    context_draft(start, end, "duplicate summary"),
                )
                .is_err()
        );
        let mut changed = messages();
        changed[1].content = "updated original".to_string();
        let report = state
            .archive_messages(SourceKind::KakaoTalk, range(), &changed)
            .unwrap();
        assert_eq!(report.updated_messages, 1);
        assert_eq!(report.unchanged_messages, 1);
        let pending = state.load_archived_messages(&query).unwrap();
        state.mark_presented(&pending).unwrap();
        state
            .save_analysis_context(
                &super::ContextScope::Thread("K001".to_string()),
                AnalysisContextDraft {
                    input_context_max_id: Some(first_session_id),
                    ..context_draft(start, end, "first cumulative thread rollup")
                },
            )
            .unwrap();
        assert_eq!(state.load_archived_messages(&query).unwrap().len(), 1);
        let second_session_id = state
            .save_analysis_context(
                &super::ContextScope::Session("K001".to_string()),
                context_draft(start, end, "second derived summary"),
            )
            .unwrap();
        let latest = state
            .latest_analysis_context(&super::ContextScope::Session("K001".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(latest.summary, "second derived summary");
        let inputs = state
            .pending_rollup_contexts(&super::ContextScope::Thread("K001".to_string()))
            .unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].metadata.id, second_session_id);
        let second_thread_id = state
            .save_analysis_context(
                &super::ContextScope::Thread("K001".to_string()),
                AnalysisContextDraft {
                    input_context_max_id: Some(second_session_id),
                    ..context_draft(start, end, "cumulative thread rollup")
                },
            )
            .unwrap();
        let latest = state
            .latest_analysis_context(&super::ContextScope::Thread("K001".to_string()))
            .unwrap()
            .unwrap();
        assert_eq!(latest.summary, "cumulative thread rollup");
        assert_eq!(latest.metadata.message_count, None);
        assert_eq!(latest.metadata.input_context_count, Some(1));
        assert_eq!(
            latest.metadata.input_context_max_id,
            Some(second_session_id)
        );
        assert!(
            state
                .pending_rollup_contexts(&super::ContextScope::Thread("K001".to_string()))
                .unwrap()
                .is_empty()
        );
        let global_inputs = state
            .pending_rollup_contexts(&super::ContextScope::Global)
            .unwrap();
        assert_eq!(global_inputs.len(), 1);
        assert_eq!(global_inputs[0].metadata.id, second_thread_id);
        state
            .save_analysis_context(
                &super::ContextScope::Global,
                AnalysisContextDraft {
                    input_context_max_id: Some(second_thread_id),
                    ..context_draft(start, end, "cumulative global rollup")
                },
            )
            .unwrap();
        let global = state
            .latest_analysis_context(&super::ContextScope::Global)
            .unwrap()
            .unwrap();
        assert_eq!(global.metadata.input_context_count, Some(2));
        assert_eq!(global.metadata.input_context_max_id, Some(second_thread_id));
        assert!(
            state
                .pending_rollup_contexts(&super::ContextScope::Global)
                .unwrap()
                .is_empty()
        );
        assert_eq!(state.list_analysis_contexts().unwrap().len(), 5);
        assert!(
            state
                .save_analysis_context(
                    &super::ContextScope::Thread("K999".to_string()),
                    AnalysisContextDraft {
                        input_context_max_id: Some(second_session_id),
                        ..context_draft(start, end, "unknown thread")
                    },
                )
                .is_err()
        );
    }

    #[test]
    fn archive_is_idempotent_and_tracks_presentation_and_analysis() {
        let mut state = StateStore::open_in_memory().unwrap();
        let first = state
            .archive_messages(SourceKind::KakaoTalk, range(), &messages())
            .unwrap();
        assert_eq!(first.inserted_messages, 2);
        let second = state
            .archive_messages(SourceKind::KakaoTalk, range(), &messages())
            .unwrap();
        assert_eq!(second.unchanged_messages, 2);

        let query = ArchiveQuery {
            source: SourceKind::KakaoTalk,
            range: range(),
            thread_alias: Some("K001".to_string()),
            pending_only: true,
        };
        let pending = state.load_archived_messages(&query).unwrap();
        assert_eq!(pending.len(), 2);
        state.mark_presented(&pending).unwrap();
        let status = state
            .archive_status(Some(SourceKind::KakaoTalk), Some("K001"))
            .unwrap();
        assert_eq!(status[0].archived_messages, 2);
        assert_eq!(status[0].pending_messages, 2);
        assert!(status[0].last_presented_at_utc.is_some());

        state
            .save_analysis_context(
                &super::ContextScope::Session("K001".to_string()),
                context_draft(range().start, range().end, "session summary"),
            )
            .unwrap();
        assert!(state.load_archived_messages(&query).unwrap().is_empty());
        let status = state
            .archive_status(Some(SourceKind::KakaoTalk), Some("K001"))
            .unwrap();
        assert_eq!(status[0].pending_messages, 0);
        assert!(status[0].last_analyzed_at_utc.is_some());
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
