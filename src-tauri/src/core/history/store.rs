use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::{params, types::Type, Connection};
use tauri::Manager;

use crate::core::{
    history::{
        HistoryResultDto, HistoryResultStatus, HistorySessionDto, HistoryTrigger, NewHistoryResult,
        NewHistorySession,
    },
    translation::TokenUsage,
};

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("无法获取应用配置目录：{0}")]
    AppConfigDir(tauri::Error),
    #[error("无法创建历史目录：{0}")]
    CreateDir(std::io::Error),
    #[error("无法打开历史数据库：{0}")]
    Open(rusqlite::Error),
    #[error("无法初始化历史数据库：{0}")]
    Init(rusqlite::Error),
    #[error("历史数据库连接锁已损坏")]
    Lock,
    #[error("历史数据库操作失败：{0}")]
    Sql(rusqlite::Error),
}

#[derive(Clone)]
pub struct HistoryStore {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl HistoryStore {
    pub fn load<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<Self, HistoryError> {
        let dir = app
            .path()
            .app_config_dir()
            .map_err(HistoryError::AppConfigDir)?;
        std::fs::create_dir_all(&dir).map_err(HistoryError::CreateDir)?;
        Self::open(dir.join("history.sqlite3"))
    }

    pub fn open(path: PathBuf) -> Result<Self, HistoryError> {
        let conn = Connection::open(&path).map_err(HistoryError::Open)?;
        Self::from_connection(path, conn)
    }

    pub fn in_memory() -> Result<Self, HistoryError> {
        let conn = Connection::open_in_memory().map_err(HistoryError::Open)?;
        Self::from_connection(PathBuf::from(":memory:"), conn)
    }

    #[cfg(test)]
    pub fn in_memory_for_test() -> Result<Self, HistoryError> {
        Self::in_memory()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn from_connection(path: PathBuf, conn: Connection) -> Result<Self, HistoryError> {
        let store = Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init().map_err(HistoryError::Init)?;
        Ok(store)
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, HistoryError> {
        let conn = self.conn.lock().map_err(|_| HistoryError::Lock)?;
        f(&conn).map_err(HistoryError::Sql)
    }

    fn init(&self) -> Result<(), rusqlite::Error> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS translation_sessions (
                id TEXT PRIMARY KEY,
                batch_id TEXT NOT NULL UNIQUE,
                trigger TEXT NOT NULL CHECK (trigger IN ('manual', 'selection', 'screenshot')),
                source_lang TEXT NOT NULL,
                target_lang TEXT NOT NULL,
                source_text TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS translation_results (
                session_id TEXT NOT NULL,
                service_instance_id TEXT NOT NULL,
                service_name TEXT NOT NULL,
                service_type TEXT NOT NULL,
                protocol TEXT NOT NULL,
                model_name TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('pending', 'success', 'error', 'cancelled')),
                translated_text TEXT NOT NULL DEFAULT '',
                error_message TEXT NOT NULL DEFAULT '',
                input_tokens INTEGER,
                output_tokens INTEGER,
                finished_at TEXT,
                PRIMARY KEY(session_id, service_instance_id),
                FOREIGN KEY(session_id) REFERENCES translation_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_translation_sessions_created_at
                ON translation_sessions(created_at DESC);
            ",
        )
    }

    pub fn create_session(&self, session: &NewHistorySession) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute(
                "
                INSERT OR IGNORE INTO translation_sessions
                    (id, batch_id, trigger, source_lang, target_lang, source_text, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ",
                params![
                    session.id,
                    session.batch_id,
                    trigger_to_str(session.trigger),
                    session.source_lang,
                    session.target_lang,
                    session.source_text,
                    session.created_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_pending_result(&self, result: &NewHistoryResult) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute(
                "
                INSERT INTO translation_results
                    (session_id, service_instance_id, service_name, service_type, protocol, model_name, status)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')
                ON CONFLICT(session_id, service_instance_id) DO UPDATE SET
                    service_name = excluded.service_name,
                    service_type = excluded.service_type,
                    protocol = excluded.protocol,
                    model_name = excluded.model_name,
                    status = 'pending',
                    translated_text = '',
                    error_message = '',
                    input_tokens = NULL,
                    output_tokens = NULL,
                    finished_at = NULL
                ",
                params![
                    result.session_id,
                    result.service_instance_id,
                    result.service_name,
                    result.service_type,
                    result.protocol,
                    result.model_name,
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_success(
        &self,
        session_id: &str,
        service_instance_id: &str,
        translated_text: &str,
        usage: Option<&TokenUsage>,
    ) -> Result<(), HistoryError> {
        let (input_tokens, output_tokens) = usage
            .map(|usage| {
                (
                    Some(usage.input_tokens as i64),
                    Some(usage.output_tokens as i64),
                )
            })
            .unwrap_or((None, None));
        self.with_conn(|conn| {
            conn.execute(
                "
                UPDATE translation_results
                SET status = 'success',
                    translated_text = ?3,
                    error_message = '',
                    input_tokens = ?4,
                    output_tokens = ?5,
                    finished_at = ?6
                WHERE session_id = ?1 AND service_instance_id = ?2
                ",
                params![
                    session_id,
                    service_instance_id,
                    translated_text,
                    input_tokens,
                    output_tokens,
                    now_iso(),
                ],
            )?;
            Ok(())
        })
    }

    pub fn mark_error(
        &self,
        session_id: &str,
        service_instance_id: &str,
        message: &str,
    ) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute(
                "
                UPDATE translation_results
                SET status = 'error', error_message = ?3, finished_at = ?4
                WHERE session_id = ?1 AND service_instance_id = ?2
                ",
                params![session_id, service_instance_id, message, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn mark_cancelled(
        &self,
        session_id: &str,
        service_instance_id: &str,
    ) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute(
                "
                UPDATE translation_results
                SET status = 'cancelled', finished_at = ?3
                WHERE session_id = ?1 AND service_instance_id = ?2
                ",
                params![session_id, service_instance_id, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn list_recent(&self, limit: usize) -> Result<Vec<HistorySessionDto>, HistoryError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "
                SELECT id, created_at, trigger, source_lang, target_lang, source_text
                FROM translation_sessions
                ORDER BY created_at DESC
                LIMIT ?1
                ",
            )?;
            let rows = stmt.query_map([limit as i64], |row| {
                let id: String = row.get(0)?;
                Ok((
                    id.clone(),
                    HistorySessionDto {
                        id,
                        timestamp: row.get(1)?,
                        trigger: trigger_from_str(row.get::<_, String>(2)?.as_str())?,
                        source_lang: row.get(3)?,
                        target_lang: row.get(4)?,
                        source: row.get(5)?,
                        results: Vec::new(),
                    },
                ))
            })?;
            let mut sessions = Vec::new();
            for row in rows {
                let (id, mut session) = row?;
                session.results = results_for_conn(conn, &id)?;
                sessions.push(session);
            }
            Ok(sessions)
        })
    }

    pub fn trim_sessions(&self, limit: usize) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute(
                "
                DELETE FROM translation_sessions
                WHERE id NOT IN (
                    SELECT id FROM translation_sessions ORDER BY created_at DESC LIMIT ?1
                )
                ",
                [limit as i64],
            )?;
            Ok(())
        })
    }

    pub fn clear(&self) -> Result<(), HistoryError> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM translation_sessions", [])?;
            Ok(())
        })
    }
}

fn results_for_conn(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<HistoryResultDto>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "
        SELECT service_instance_id, service_name, service_type, protocol, model_name,
               translated_text, error_message, status, input_tokens, output_tokens
        FROM translation_results
        WHERE session_id = ?1
        ORDER BY rowid ASC
        ",
    )?;
    let rows = stmt.query_map([session_id], |row| {
        let input_tokens: Option<i64> = row.get(8)?;
        let output_tokens: Option<i64> = row.get(9)?;
        Ok(HistoryResultDto {
            service_instance_id: row.get(0)?,
            service_name: row.get(1)?,
            service_type: row.get(2)?,
            protocol: row.get(3)?,
            model_name: row.get(4)?,
            translation: row.get(5)?,
            error_message: row.get(6)?,
            status: status_from_str(row.get::<_, String>(7)?.as_str())?,
            input_tokens: input_tokens.map(|value| value as u64),
            output_tokens: output_tokens.map(|value| value as u64),
        })
    })?;
    rows.collect()
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn trigger_to_str(trigger: HistoryTrigger) -> &'static str {
    match trigger {
        HistoryTrigger::Manual => "manual",
        HistoryTrigger::Selection => "selection",
        HistoryTrigger::Screenshot => "screenshot",
    }
}

fn trigger_from_str(value: &str) -> rusqlite::Result<HistoryTrigger> {
    match value {
        "manual" => Ok(HistoryTrigger::Manual),
        "selection" => Ok(HistoryTrigger::Selection),
        "screenshot" => Ok(HistoryTrigger::Screenshot),
        _ => Err(invalid_history_value(value)),
    }
}

fn status_from_str(value: &str) -> rusqlite::Result<HistoryResultStatus> {
    match value {
        "pending" => Ok(HistoryResultStatus::Pending),
        "success" => Ok(HistoryResultStatus::Success),
        "error" => Ok(HistoryResultStatus::Error),
        "cancelled" => Ok(HistoryResultStatus::Cancelled),
        _ => Err(invalid_history_value(value)),
    }
}

fn invalid_history_value(value: &str) -> rusqlite::Error {
    let err: Box<dyn Error + Send + Sync + 'static> = Box::new(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("非法历史枚举值：{value}"),
    ));
    rusqlite::Error::FromSqlConversionFailure(0, Type::Text, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::history::{
        HistoryResultStatus, HistoryTrigger, NewHistoryResult, NewHistorySession,
    };
    use crate::core::translation::TokenUsage;

    fn session(id: &str, batch_id: &str, created_at: &str) -> NewHistorySession {
        NewHistorySession {
            id: id.to_string(),
            batch_id: batch_id.to_string(),
            trigger: HistoryTrigger::Manual,
            source_lang: "auto".to_string(),
            target_lang: "zh-CN".to_string(),
            source_text: format!("source-{id}"),
            created_at: created_at.to_string(),
        }
    }

    fn result(session_id: &str, service_instance_id: &str) -> NewHistoryResult {
        NewHistoryResult {
            session_id: session_id.to_string(),
            service_instance_id: service_instance_id.to_string(),
            service_name: format!("service-{service_instance_id}"),
            service_type: "llm".to_string(),
            protocol: "openai_chat".to_string(),
            model_name: format!("model-{service_instance_id}"),
        }
    }

    #[test]
    fn initializes_schema_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.sqlite3");

        let store = HistoryStore::open(path.clone()).unwrap();

        assert_eq!(store.path(), path.as_path());
        assert!(path.exists());
    }

    #[test]
    fn stores_sessions_and_results_descending() {
        let store = HistoryStore::in_memory_for_test().unwrap();
        store
            .create_session(&session("old", "batch-old", "2026-01-01T00:00:00Z"))
            .unwrap();
        store
            .create_session(&session("new", "batch-new", "2026-01-02T00:00:00Z"))
            .unwrap();
        store.upsert_pending_result(&result("new", "a")).unwrap();
        store.upsert_pending_result(&result("new", "b")).unwrap();
        store
            .mark_success(
                "new",
                "a",
                "译文-a",
                Some(&TokenUsage {
                    input_tokens: 11,
                    output_tokens: 22,
                }),
            )
            .unwrap();
        store.mark_error("new", "b", "失败-b").unwrap();

        let sessions = store.list_recent(10).unwrap();

        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, "new");
        assert_eq!(sessions[0].timestamp, "2026-01-02T00:00:00Z");
        assert_eq!(sessions[0].source, "source-new");
        assert_eq!(sessions[0].results.len(), 2);
        assert_eq!(sessions[0].results[0].service_instance_id, "a");
        assert_eq!(sessions[0].results[0].translation, "译文-a");
        assert_eq!(sessions[0].results[0].status, HistoryResultStatus::Success);
        assert_eq!(sessions[0].results[0].input_tokens, Some(11));
        assert_eq!(sessions[0].results[0].output_tokens, Some(22));
        assert_eq!(sessions[0].results[1].service_instance_id, "b");
        assert_eq!(sessions[0].results[1].error_message, "失败-b");
        assert_eq!(sessions[0].results[1].status, HistoryResultStatus::Error);
        assert_eq!(sessions[1].id, "old");
    }

    #[test]
    fn stores_cancelled_status() {
        let store = HistoryStore::in_memory_for_test().unwrap();
        store
            .create_session(&session("s1", "batch-1", "2026-01-01T00:00:00Z"))
            .unwrap();
        store.upsert_pending_result(&result("s1", "a")).unwrap();

        store.mark_cancelled("s1", "a").unwrap();

        let sessions = store.list_recent(10).unwrap();
        assert_eq!(
            sessions[0].results[0].status,
            HistoryResultStatus::Cancelled
        );
    }

    #[test]
    fn upsert_pending_result_resets_previous_output_fields() {
        let store = HistoryStore::in_memory_for_test().unwrap();
        store
            .create_session(&session("s1", "batch-1", "2026-01-01T00:00:00Z"))
            .unwrap();
        let result = result("s1", "a");
        store.upsert_pending_result(&result).unwrap();
        store
            .mark_success(
                "s1",
                "a",
                "旧译文",
                Some(&TokenUsage {
                    input_tokens: 1,
                    output_tokens: 2,
                }),
            )
            .unwrap();

        store.upsert_pending_result(&result).unwrap();

        let result = &store.list_recent(10).unwrap()[0].results[0];
        assert_eq!(result.status, HistoryResultStatus::Pending);
        assert_eq!(result.translation, "");
        assert_eq!(result.error_message, "");
        assert_eq!(result.input_tokens, None);
        assert_eq!(result.output_tokens, None);
    }

    #[test]
    fn schema_rejects_invalid_trigger_and_status() {
        let store = HistoryStore::in_memory_for_test().unwrap();

        let invalid_trigger = store.with_conn(|conn| {
            conn.execute(
                "
                INSERT INTO translation_sessions
                    (id, batch_id, trigger, source_lang, target_lang, source_text, created_at)
                VALUES ('bad-trigger', 'batch-bad-trigger', 'unknown', 'auto', 'zh-CN', 'x', '2026-01-01T00:00:00Z')
                ",
                [],
            )
        });
        assert!(invalid_trigger.is_err());

        store
            .create_session(&session("s1", "batch-1", "2026-01-01T00:00:00Z"))
            .unwrap();
        let invalid_status = store.with_conn(|conn| {
            conn.execute(
                "
                INSERT INTO translation_results
                    (session_id, service_instance_id, service_name, service_type, protocol, model_name, status)
                VALUES ('s1', 'bad-status', 'svc', 'llm', 'mock', 'model', 'unknown')
                ",
                [],
            )
        });
        assert!(invalid_status.is_err());
    }

    #[test]
    fn trim_sessions_removes_old_results() {
        let store = HistoryStore::in_memory_for_test().unwrap();
        store
            .create_session(&session("old", "batch-old", "2026-01-01T00:00:00Z"))
            .unwrap();
        store.upsert_pending_result(&result("old", "a")).unwrap();
        store
            .create_session(&session("new", "batch-new", "2026-01-02T00:00:00Z"))
            .unwrap();
        store.upsert_pending_result(&result("new", "b")).unwrap();

        store.trim_sessions(1).unwrap();

        let sessions = store.list_recent(10).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, "new");
        assert_eq!(sessions[0].results.len(), 1);
        assert_eq!(sessions[0].results[0].service_instance_id, "b");
    }

    #[test]
    fn clear_removes_all_history() {
        let store = HistoryStore::in_memory_for_test().unwrap();
        store
            .create_session(&session("s1", "batch-1", "2026-01-01T00:00:00Z"))
            .unwrap();
        store.upsert_pending_result(&result("s1", "a")).unwrap();

        store.clear().unwrap();

        assert!(store.list_recent(10).unwrap().is_empty());
    }
}
