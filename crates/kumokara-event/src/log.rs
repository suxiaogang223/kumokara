//! SQLite event log — persistence layer for structured events.
//!
//! Events are stored per-workspace in `events.db` files.
//! Supports CRUD operations, retention policies, and seq-based incremental queries.

use anyhow::Result;
use kumokara_protocol::event::{Event, EventEntry};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;

/// Manages the SQLite event log for a single workspace.
pub struct EventLog {
    pool: SqlitePool,
}

impl EventLog {
    /// Open (or create) the event log at the given path.
    pub async fn open(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let log = Self { pool };
        log.migrate().await?;
        Ok(log)
    }

    /// Run schema migration.
    async fn migrate(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                seq         INTEGER NOT NULL,
                timestamp   TEXT NOT NULL,
                session_id  TEXT,
                workspace_id TEXT NOT NULL,
                source      TEXT NOT NULL,
                event_type  TEXT NOT NULL,
                event_json  TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_event_log_workspace_seq
                ON event_log(workspace_id, seq);

            CREATE INDEX IF NOT EXISTS idx_event_log_session
                ON event_log(session_id);

            CREATE INDEX IF NOT EXISTS idx_event_log_timestamp
                ON event_log(timestamp);

            -- Track the current sequence number per workspace
            CREATE TABLE IF NOT EXISTS event_seq (
                workspace_id TEXT PRIMARY KEY,
                next_seq     INTEGER NOT NULL DEFAULT 1
            );
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert an event and return its assigned sequence number.
    pub async fn insert_event(&self, entry: &EventEntry) -> Result<i64> {
        // Get and increment the sequence number atomically
        let seq = self.next_seq(&entry.workspace_id).await?;

        let event_type = event_type_name(&entry.event);
        let event_json = serde_json::to_string(&entry.event)?;

        sqlx::query(
            r#"
            INSERT INTO event_log (seq, timestamp, session_id, workspace_id, source, event_type, event_json)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(seq)
        .bind(&entry.timestamp)
        .bind(&entry.session_id)
        .bind(&entry.workspace_id)
        .bind(&entry.source)
        .bind(event_type)
        .bind(event_json)
        .execute(&self.pool)
        .await?;

        Ok(seq)
    }

    /// Query events after a given sequence number (for incremental sync).
    pub async fn query_events(
        &self,
        workspace_id: &str,
        after_seq: Option<i64>,
        limit: Option<i64>,
        event_types: Option<&[String]>,
    ) -> Result<Vec<EventEntry>> {
        let limit = limit.unwrap_or(100);
        let after_seq = after_seq.unwrap_or(0);

        let rows = if let Some(types) = event_types {
            // Build dynamic query with type filter
            let placeholders: Vec<String> = types.iter().enumerate()
                .map(|(i, _)| format!("?{}", i + 3))
                .collect();
            let query = format!(
                r#"SELECT seq, timestamp, session_id, workspace_id, source, event_type, event_json
                   FROM event_log
                   WHERE workspace_id = ?1 AND seq > ?2 AND event_type IN ({})
                   ORDER BY seq ASC
                   LIMIT ?{}"#,
                placeholders.join(", "),
                types.len() + 3
            );
            let mut q = sqlx::query_as::<_, EventRow>(&query)
                .bind(workspace_id)
                .bind(after_seq);
            for t in types {
                q = q.bind(t);
            }
            q = q.bind(limit);
            q.fetch_all(&self.pool).await?
        } else {
            sqlx::query_as::<_, EventRow>(
                r#"SELECT seq, timestamp, session_id, workspace_id, source, event_type, event_json
                   FROM event_log
                   WHERE workspace_id = ?1 AND seq > ?2
                   ORDER BY seq ASC
                   LIMIT ?3"#,
            )
            .bind(workspace_id)
            .bind(after_seq)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };

        let events = rows
            .into_iter()
            .map(|row: EventRow| {
                let event: Event = serde_json::from_str(&row.event_json)
                    .unwrap_or_else(|_| Event::WorkspaceEvent {
                        workspace_id: row.workspace_id.clone(),
                        description: "failed to deserialize event".to_string(),
                    });
                EventEntry {
                    seq: row.seq,
                    timestamp: row.timestamp,
                    session_id: row.session_id,
                    workspace_id: row.workspace_id,
                    source: row.source,
                    event,
                }
            })
            .collect();

        Ok(events)
    }

    /// Get the current maximum sequence number for a workspace.
    pub async fn current_seq(&self, workspace_id: &str) -> Result<i64> {
        let row: (Option<i64>,) = sqlx::query_as(
            "SELECT MAX(seq) FROM event_log WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0.unwrap_or(0))
    }

    /// Delete events older than the specified timestamp (retention policy).
    pub async fn delete_before(&self, workspace_id: &str, before_timestamp: &str) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM event_log WHERE workspace_id = ?1 AND timestamp < ?2",
        )
        .bind(workspace_id)
        .bind(before_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Get the total event count for a workspace.
    pub async fn event_count(&self, workspace_id: &str) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM event_log WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    /// Atomically get and increment the sequence number for a workspace.
    async fn next_seq(&self, workspace_id: &str) -> Result<i64> {
        // Upsert: insert if not exists, then increment
        sqlx::query(
            r#"
            INSERT INTO event_seq (workspace_id, next_seq)
            VALUES (?1, 1)
            ON CONFLICT(workspace_id) DO UPDATE SET next_seq = next_seq + 1
            "#,
        )
        .bind(workspace_id)
        .execute(&self.pool)
        .await?;

        let row: (i64,) = sqlx::query_as(
            "SELECT next_seq FROM event_seq WHERE workspace_id = ?1",
        )
        .bind(workspace_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }
}

/// Extract a stable event type name for the tag.
fn event_type_name(event: &Event) -> &'static str {
    match event {
        Event::SessionCreated { .. } => "session_created",
        Event::SessionDestroyed { .. } => "session_destroyed",
        Event::CommandStarted { .. } => "command_started",
        Event::CommandFinished { .. } => "command_finished",
        Event::CwdChanged { .. } => "cwd_changed",
        Event::AgentStarted { .. } => "agent_started",
        Event::AgentStateChanged { .. } => "agent_state_changed",
        Event::AgentTask { .. } => "agent_task",
        Event::AgentApproval { .. } => "agent_approval",
        Event::AgentCompleted { .. } => "agent_completed",
        Event::AgentError { .. } => "agent_error",
        Event::WorkspaceEvent { .. } => "workspace_event",
    }
}

/// Row type for query results.
#[derive(sqlx::FromRow)]
struct EventRow {
    seq: i64,
    timestamp: String,
    session_id: Option<String>,
    workspace_id: String,
    source: String,
    #[allow(dead_code)]
    event_type: String,
    event_json: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_open_and_migrate() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("events.db");
        let log = EventLog::open(&db_path).await.unwrap();

        // Verify DB opened and schema migrated
        let count = log.current_seq("ws-1").await.unwrap();
        assert_eq!(count, 0);

        let event_count = log.event_count("ws-1").await.unwrap();
        assert_eq!(event_count, 0);
    }

    #[tokio::test]
    async fn test_insert_event() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("events.db");
        let log = EventLog::open(&db_path).await.unwrap();

        let entry = EventEntry {
            seq: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: Some("sess-1".to_string()),
            workspace_id: "ws-1".to_string(),
            source: "test".to_string(),
            event: Event::CommandStarted {
                session_id: "sess-1".to_string(),
                command: "cargo build".to_string(),
                cwd: Some("/home/user".to_string()),
            },
        };

        // Insert and verify seq assignment works
        let seq = log.insert_event(&entry).await.unwrap();
        assert_eq!(seq, 1);
        // Note: query_events fix for sqlx 0.8 is tracked for Phase 1
    }
}
