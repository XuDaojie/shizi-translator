# 翻译历史 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将手动输入、划词、截图 OCR 进入统一翻译链路的请求持久化为 SQLite 翻译历史，并让设置页历史面板读取真实后端数据。

**架构：** 后端新增 `core::history`，用 `rusqlite` 管理 `app_config_dir()/history.sqlite3`，`AppState` 与 `ConfigStore` 同级持有 `HistoryStore`。`web_popup.rs` 只在统一翻译入口触发历史写入，SQL 全部留在 `core/history`；设置页通过 Tauri command 拉取和清空历史，不再读 `localStorage` 的 `ocrHistory`。

**技术栈：** Rust / Tauri 2 / `rusqlite` bundled / Vue 3 / Vitest / `window.__TAURI__.core.invoke`。

**规格：** `docs/superpowers/specs/2026-07-11-translation-history-design.md`

---

## 文件结构

| 文件 | 职责 | 动作 |
|---|---|---|
| `src-tauri/Cargo.toml` | 新增 SQLite 依赖 | 修改 |
| `src-tauri/src/core/mod.rs` | 暴露 `history` 核心模块 | 修改 |
| `src-tauri/src/core/history/mod.rs` | re-export 历史类型与 store | 创建 |
| `src-tauri/src/core/history/types.rs` | SQLite/command 共用 DTO、trigger/status 类型、输入映射 | 创建 |
| `src-tauri/src/core/history/store.rs` | SQLite schema、CRUD、聚合查询、裁剪 | 创建 |
| `src-tauri/src/core/config/types.rs` | 后端 `AppConfig.historyLimit` | 修改 |
| `src-tauri/src/app/state.rs` | `AppState` 持有 `HistoryStore` | 修改 |
| `src-tauri/src/lib.rs` | 启动时初始化 `HistoryStore`，注册 history commands | 修改 |
| `src-tauri/src/ui/mod.rs` | 暴露 `ui::history` | 修改 |
| `src-tauri/src/ui/history.rs` | `list_translation_history` / `clear_translation_history` commands | 创建 |
| `src-tauri/src/ui/web_popup.rs` | 翻译批次创建、result pending/success/error/cancelled 落库，结束后裁剪 | 修改 |
| `frontend/src/types/config.ts` | 前端 `AppConfig.historyLimit` | 修改 |
| `frontend/src/lib/config.ts` | 投影 `translation.historyLimit` 到后端配置 | 修改 |
| `frontend/src/lib/tauri.ts` | 新增历史 DTO 类型和 invoke helper | 修改 |
| `frontend/src/settings/history.ts` | 设置页历史数据状态、command 编排、结果状态映射 | 创建 |
| `frontend/src/settings/history.test.ts` | 历史空状态、session 数据、清空刷新 command 编排测试 | 创建 |
| `frontend/src/settings/types.ts` | 删除 `OcrHistoryEntry` 与 `AppSettings.ocrHistory` | 修改 |
| `frontend/src/settings/stores/settings.ts` | 删除 `ocrHistory` 本地存储适配层和 helper，合并 `historyLimit` | 修改 |
| `frontend/src/settings/stores/settings.test.ts` | 删除 OCR 历史默认值断言，补 `historyLimit` 同步断言 | 修改 |
| `frontend/src/lib/config.test.ts` | 补 `historyLimit` 投影断言 | 修改 |
| `frontend/src/settings/panels/HistoryPanel.vue` | 改为后端真实数据源，删除 mock、`ocrHistory` 适配和开发中提示 | 修改 |
| `frontend/src/settings/SettingsSidebar.vue` | 删除历史 wip badge，描述改为真实历史 | 修改 |
| `README.md` | 实现完成后同步当前能力和限制 | 修改 |
| `AGENTS.md` | 实现完成后同步架构关键点 | 修改 |
| `CLAUDE.md` | 与 `AGENTS.md` 同步 | 修改 |
| `docs/superpowers/plans/2026-07-11-translation-history.md` | 实现完成后回填复选框 | 修改 |

## 关键约定

```rust
// core/history/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryTrigger {
    Manual,
    Selection,
    Screenshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryResultStatus {
    Pending,
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionDto {
    pub id: String,
    pub timestamp: String,
    pub trigger: HistoryTrigger,
    pub source_lang: String,
    pub target_lang: String,
    pub source: String,
    pub results: Vec<HistoryResultDto>,
}
```

`translation_sessions.id` 直接使用 `batch_id`；`translation_results.session_id` 引用这个 `batch_id`。翻译运行时的 `TranslationRequest.session_id` 仍保持 `{batch_id}:{service_id}`，只用于事件流，不作为历史 session 主键。

`modelName` 只能从 `ServiceInstanceConfig.model` 取得，不能从 `TranslationServiceMeta` 推断。

---

## 任务 1：让 `historyLimit` 进入后端配置链路

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [ ] **步骤 1：编写失败测试：前端配置投影包含 `historyLimit`**

在 `frontend/src/lib/config.test.ts` 的 `projectToAppConfig` 第一个用例中，`const config = projectToAppConfig(state)` 后追加断言：

```ts
expect(config.historyLimit).toBe(500);
```

在同文件 `validateConfig` 的 `base` 对象中追加字段：

```ts
historyLimit: 500,
```

- [ ] **步骤 2：编写失败测试：后端同步回填 `historyLimit`**

在 `frontend/src/settings/stores/settings.test.ts` 的 `syncFromBackend` 中，给所有 `invokeGetAppConfig.mockResolvedValue` 返回对象追加：

```ts
historyLimit: 500,
```

并在“后端非空时按 id 合并到 state，不推覆盖”用例的后端返回对象里把值改为 123，断言：

```ts
expect(settings.state.translation.historyLimit).toBe(123);
```

- [ ] **步骤 3：运行前端测试确认失败**

运行：`npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts`

预期：TypeScript/Vitest 失败，提示 `historyLimit` 不在 `AppConfig` 或投影结果中。

- [ ] **步骤 4：实现最小前端配置改动**

`frontend/src/types/config.ts` 的 `AppConfig` 增加：

```ts
historyLimit: number;
```

`frontend/src/lib/config.ts` 的 `projectToAppConfig` 返回对象增加：

```ts
historyLimit: Math.max(1, Number(state.translation.historyLimit) || 500),
```

`frontend/src/settings/stores/settings.ts` 的 `syncFromBackend` 在 `restoreClipboard` 合并后增加：

```ts
state.translation.historyLimit =
  backend.historyLimit ?? state.translation.historyLimit
```

- [ ] **步骤 5：实现后端配置字段**

`src-tauri/src/core/config/types.rs` 增加默认函数：

```rust
fn default_history_limit() -> usize {
    500
}
```

`AppConfig` 增加字段：

```rust
#[serde(default = "default_history_limit")]
pub history_limit: usize,
```

`AppConfig::from_env()` 初始化增加：

```rust
history_limit: default_history_limit(),
```

`AppConfig::normalized()` 增加：

```rust
if self.history_limit == 0 {
    self.history_limit = default_history_limit();
}
```

在 `src-tauri/src/core/config/types.rs` 测试模块增加：

```rust
#[test]
fn normalized_fills_empty_history_limit() {
    let mut config = AppConfig::from_env();
    config.history_limit = 0;

    let normalized = config.normalized();

    assert_eq!(normalized.history_limit, 500);
}
```

- [ ] **步骤 6：运行测试验证通过**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts
cd src-tauri && cargo test --lib core::config::types::tests::normalized_fills_empty_history_limit
```

预期：全部 PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/config/types.rs frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(config): 同步翻译历史上限配置"
```

---

## 任务 2：新增 SQLite `HistoryStore`

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/src/core/mod.rs`
- 创建：`src-tauri/src/core/history/mod.rs`
- 创建：`src-tauri/src/core/history/types.rs`
- 创建：`src-tauri/src/core/history/store.rs`

- [ ] **步骤 1：新增依赖**

在 `src-tauri/Cargo.toml` 的 `[dependencies]` 中增加：

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

- [ ] **步骤 2：创建模块出口**

`src-tauri/src/core/mod.rs` 增加：

```rust
pub mod history;
```

创建 `src-tauri/src/core/history/mod.rs`：

```rust
pub mod store;
pub mod types;

pub use store::HistoryStore;
pub use types::{
    history_trigger_for_input, HistoryResultDto, HistoryResultStatus, HistorySessionDto,
    HistoryTrigger, NewHistoryResult, NewHistorySession,
};
```

- [ ] **步骤 3：编写失败测试：trigger 映射**

创建 `src-tauri/src/core/history/types.rs`，先写类型和测试骨架：

```rust
use crate::core::translation::TranslationInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryTrigger {
    Manual,
    Selection,
    Screenshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryResultStatus {
    Pending,
    Success,
    Error,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct NewHistorySession {
    pub id: String,
    pub batch_id: String,
    pub trigger: HistoryTrigger,
    pub source_lang: String,
    pub target_lang: String,
    pub source_text: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewHistoryResult {
    pub session_id: String,
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryResultDto {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
    pub translation: String,
    pub error_message: String,
    pub status: HistoryResultStatus,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistorySessionDto {
    pub id: String,
    pub timestamp: String,
    pub trigger: HistoryTrigger,
    pub source_lang: String,
    pub target_lang: String,
    pub source: String,
    pub results: Vec<HistoryResultDto>,
}

pub fn history_trigger_for_input(input: &TranslationInput) -> HistoryTrigger {
    match input {
        TranslationInput::ManualText(_) => HistoryTrigger::Manual,
        TranslationInput::SelectedText(_) => HistoryTrigger::Selection,
        TranslationInput::OcrText { .. } => HistoryTrigger::Screenshot,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_translation_input_to_history_trigger() {
        assert_eq!(
            history_trigger_for_input(&TranslationInput::ManualText("x".to_string())),
            HistoryTrigger::Manual
        );
        assert_eq!(
            history_trigger_for_input(&TranslationInput::SelectedText("x".to_string())),
            HistoryTrigger::Selection
        );
        assert_eq!(
            history_trigger_for_input(&TranslationInput::OcrText { text: "x".to_string(), image_id: None }),
            HistoryTrigger::Screenshot
        );
    }
}
```

运行：`cd src-tauri && cargo test --lib core::history::types::tests::maps_translation_input_to_history_trigger`

预期：PASS。

- [ ] **步骤 4：编写失败测试：store schema、聚合查询、状态更新、裁剪**

创建 `src-tauri/src/core/history/store.rs`，先写测试和签名：

```rust
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use rusqlite::{params, Connection, OptionalExtension};
use tauri::Manager;

use crate::core::translation::TokenUsage;

use super::types::{
    HistoryResultDto, HistoryResultStatus, HistorySessionDto, HistoryTrigger, NewHistoryResult,
    NewHistorySession,
};

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("无法获取应用配置目录：{0}")]
    AppConfigDir(String),
    #[error("无法创建历史目录：{0}")]
    CreateDir(#[source] std::io::Error),
    #[error("无法打开历史数据库：{0}")]
    Open(#[source] rusqlite::Error),
    #[error("无法初始化历史数据库：{0}")]
    Init(#[source] rusqlite::Error),
    #[error("历史数据库状态锁已损坏")]
    Lock,
    #[error("历史数据库操作失败：{0}")]
    Sql(#[source] rusqlite::Error),
}

#[derive(Clone)]
pub struct HistoryStore {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl HistoryStore {
    pub fn load(app: &tauri::AppHandle) -> Result<Self, HistoryError> {
        let dir = app
            .path()
            .app_config_dir()
            .map_err(|error| HistoryError::AppConfigDir(error.to_string()))?;
        std::fs::create_dir_all(&dir).map_err(HistoryError::CreateDir)?;
        Self::open(dir.join("history.sqlite3"))
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, HistoryError> {
        let path = path.as_ref().to_path_buf();
        let conn = Connection::open(&path).map_err(HistoryError::Open)?;
        let store = Self {
            path,
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn in_memory_for_test() -> Self {
        let conn = Connection::open_in_memory().expect("打开内存数据库");
        let store = Self {
            path: PathBuf::from(":memory:"),
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init().expect("初始化内存历史数据库");
        store
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, HistoryError> {
        let conn = self.conn.lock().map_err(|_| HistoryError::Lock)?;
        f(&conn).map_err(HistoryError::Sql)
    }

    pub fn init(&self) -> Result<(), HistoryError> {
        let conn = self.conn.lock().map_err(|_| HistoryError::Lock)?;
        conn.execute_batch(
            r#"
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS translation_sessions (
              id TEXT PRIMARY KEY,
              batch_id TEXT NOT NULL UNIQUE,
              trigger TEXT NOT NULL,
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
              status TEXT NOT NULL,
              translated_text TEXT NOT NULL DEFAULT '',
              error_message TEXT NOT NULL DEFAULT '',
              input_tokens INTEGER,
              output_tokens INTEGER,
              finished_at TEXT,
              PRIMARY KEY (session_id, service_instance_id),
              FOREIGN KEY (session_id) REFERENCES translation_sessions(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_translation_sessions_created_at
            ON translation_sessions(created_at DESC);
            "#,
        )
        .map_err(HistoryError::Init)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn session(id: &str, created_at: &str) -> NewHistorySession {
        NewHistorySession {
            id: id.to_string(),
            batch_id: id.to_string(),
            trigger: HistoryTrigger::Manual,
            source_lang: "auto".to_string(),
            target_lang: "zh-CN".to_string(),
            source_text: format!("source-{id}"),
            created_at: created_at.to_string(),
        }
    }

    fn result(session_id: &str, service_id: &str) -> NewHistoryResult {
        NewHistoryResult {
            session_id: session_id.to_string(),
            service_instance_id: service_id.to_string(),
            service_name: format!("svc-{service_id}"),
            service_type: "deepseek".to_string(),
            protocol: "openai_chat".to_string(),
            model_name: "deepseek-chat".to_string(),
        }
    }

    #[test]
    fn initializes_schema_file() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("history.sqlite3");
        let store = HistoryStore::open(&db).unwrap();

        assert!(store.path().ends_with("history.sqlite3"));
        assert!(db.exists());
    }

    #[test]
    fn stores_sessions_and_results_descending() {
        let store = HistoryStore::in_memory_for_test();
        store.create_session(&session("old", "2026-07-11T00:00:00Z")).unwrap();
        store.create_session(&session("new", "2026-07-11T01:00:00Z")).unwrap();
        store.upsert_pending_result(&result("new", "a")).unwrap();
        store.upsert_pending_result(&result("new", "b")).unwrap();
        store.mark_success("new", "a", "译文 A", Some(&TokenUsage { input_tokens: 1, output_tokens: 2 })).unwrap();
        store.mark_error("new", "b", "失败").unwrap();

        let rows = store.list_recent(10).unwrap();

        assert_eq!(rows.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["new", "old"]);
        assert_eq!(rows[0].results.len(), 2);
        assert_eq!(rows[0].results[0].service_instance_id, "a");
        assert_eq!(rows[0].results[0].status, HistoryResultStatus::Success);
        assert_eq!(rows[0].results[0].translation, "译文 A");
        assert_eq!(rows[0].results[0].input_tokens, Some(1));
        assert_eq!(rows[0].results[1].status, HistoryResultStatus::Error);
        assert_eq!(rows[0].results[1].error_message, "失败");
    }

    #[test]
    fn stores_cancelled_status() {
        let store = HistoryStore::in_memory_for_test();
        store.create_session(&session("s1", "2026-07-11T00:00:00Z")).unwrap();
        store.upsert_pending_result(&result("s1", "a")).unwrap();
        store.mark_cancelled("s1", "a").unwrap();

        let rows = store.list_recent(10).unwrap();

        assert_eq!(rows[0].results[0].status, HistoryResultStatus::Cancelled);
    }

    #[test]
    fn trim_sessions_removes_old_results() {
        let store = HistoryStore::in_memory_for_test();
        store.create_session(&session("old", "2026-07-11T00:00:00Z")).unwrap();
        store.upsert_pending_result(&result("old", "a")).unwrap();
        store.create_session(&session("new", "2026-07-11T01:00:00Z")).unwrap();
        store.upsert_pending_result(&result("new", "a")).unwrap();

        store.trim_sessions(1).unwrap();
        let rows = store.list_recent(10).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "new");
        assert_eq!(rows[0].results.len(), 1);
    }

    #[test]
    fn clear_removes_all_history() {
        let store = HistoryStore::in_memory_for_test();
        store.create_session(&session("s1", "2026-07-11T00:00:00Z")).unwrap();
        store.upsert_pending_result(&result("s1", "a")).unwrap();

        store.clear().unwrap();

        assert!(store.list_recent(10).unwrap().is_empty());
    }
}
```

运行：`cd src-tauri && cargo test --lib core::history::store::tests`

预期：编译失败，缺少 `create_session`、`upsert_pending_result`、`mark_success`、`mark_error`、`mark_cancelled`、`list_recent`、`trim_sessions`、`clear`。

- [ ] **步骤 5：实现 store 方法**

在 `impl HistoryStore` 中追加：

```rust
pub fn create_session(&self, session: &NewHistorySession) -> Result<(), HistoryError> {
    self.with_conn(|conn| {
        conn.execute(
            r#"
            INSERT OR IGNORE INTO translation_sessions
              (id, batch_id, trigger, source_lang, target_lang, source_text, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
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
            r#"
            INSERT INTO translation_results
              (session_id, service_instance_id, service_name, service_type, protocol, model_name, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending')
            ON CONFLICT(session_id, service_instance_id) DO UPDATE SET
              service_name = excluded.service_name,
              service_type = excluded.service_type,
              protocol = excluded.protocol,
              model_name = excluded.model_name,
              status = excluded.status
            "#,
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
    let input_tokens = usage.map(|u| u.input_tokens);
    let output_tokens = usage.map(|u| u.output_tokens);
    let finished_at = now_iso();
    self.with_conn(|conn| {
        conn.execute(
            r#"
            UPDATE translation_results
            SET status = 'success',
                translated_text = ?3,
                error_message = '',
                input_tokens = ?4,
                output_tokens = ?5,
                finished_at = ?6
            WHERE session_id = ?1 AND service_instance_id = ?2
            "#,
            params![session_id, service_instance_id, translated_text, input_tokens, output_tokens, finished_at],
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
    let finished_at = now_iso();
    self.with_conn(|conn| {
        conn.execute(
            r#"
            UPDATE translation_results
            SET status = 'error',
                error_message = ?3,
                finished_at = ?4
            WHERE session_id = ?1 AND service_instance_id = ?2
            "#,
            params![session_id, service_instance_id, message, finished_at],
        )?;
        Ok(())
    })
}

pub fn mark_cancelled(&self, session_id: &str, service_instance_id: &str) -> Result<(), HistoryError> {
    let finished_at = now_iso();
    self.with_conn(|conn| {
        conn.execute(
            r#"
            UPDATE translation_results
            SET status = 'cancelled',
                finished_at = ?3
            WHERE session_id = ?1 AND service_instance_id = ?2
            "#,
            params![session_id, service_instance_id, finished_at],
        )?;
        Ok(())
    })
}

pub fn list_recent(&self, limit: usize) -> Result<Vec<HistorySessionDto>, HistoryError> {
    self.with_conn(|conn| {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, created_at, trigger, source_lang, target_lang, source_text
            FROM translation_sessions
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;
        let sessions = stmt
            .query_map(params![limit as i64], |row| {
                Ok(HistorySessionDto {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    trigger: trigger_from_str(row.get::<_, String>(2)?.as_str()),
                    source_lang: row.get(3)?,
                    target_lang: row.get(4)?,
                    source: row.get(5)?,
                    results: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut out = Vec::with_capacity(sessions.len());
        for mut session in sessions {
            session.results = self.results_for_conn(conn, &session.id)?;
            out.push(session);
        }
        Ok(out)
    })
}

fn results_for_conn(&self, conn: &Connection, session_id: &str) -> rusqlite::Result<Vec<HistoryResultDto>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT service_instance_id, service_name, service_type, protocol, model_name,
               translated_text, error_message, status, input_tokens, output_tokens
        FROM translation_results
        WHERE session_id = ?1
        ORDER BY rowid ASC
        "#,
    )?;
    stmt.query_map(params![session_id], |row| {
        Ok(HistoryResultDto {
            service_instance_id: row.get(0)?,
            service_name: row.get(1)?,
            service_type: row.get(2)?,
            protocol: row.get(3)?,
            model_name: row.get(4)?,
            translation: row.get(5)?,
            error_message: row.get(6)?,
            status: status_from_str(row.get::<_, String>(7)?.as_str()),
            input_tokens: row.get(8)?,
            output_tokens: row.get(9)?,
        })
    })?
    .collect()
}

pub fn trim_sessions(&self, limit: usize) -> Result<(), HistoryError> {
    self.with_conn(|conn| {
        conn.execute(
            r#"
            DELETE FROM translation_sessions
            WHERE id NOT IN (
              SELECT id FROM translation_sessions
              ORDER BY created_at DESC
              LIMIT ?1
            )
            "#,
            params![limit as i64],
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
```

在文件底部追加转换函数：

```rust
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

fn trigger_from_str(value: &str) -> HistoryTrigger {
    match value {
        "selection" => HistoryTrigger::Selection,
        "screenshot" => HistoryTrigger::Screenshot,
        _ => HistoryTrigger::Manual,
    }
}

fn status_from_str(value: &str) -> HistoryResultStatus {
    match value {
        "success" => HistoryResultStatus::Success,
        "error" => HistoryResultStatus::Error,
        "cancelled" => HistoryResultStatus::Cancelled,
        _ => HistoryResultStatus::Pending,
    }
}
```

- [ ] **步骤 6：运行 store 测试验证通过**

运行：`cd src-tauri && cargo test --lib core::history`

预期：全部 PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/core/mod.rs src-tauri/src/core/history
git commit -m "feat(history): 新增 SQLite 翻译历史存储"
```

---

## 任务 3：AppState 持有 HistoryStore 并暴露历史 commands

**文件：**
- 修改：`src-tauri/src/app/state.rs`
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/src/ui/mod.rs`
- 创建：`src-tauri/src/ui/history.rs`
- 修改：`src-tauri/src/ui/web_popup.rs` 测试 helper 的 `AppState` 构造

- [ ] **步骤 1：编写失败测试：AppState test helper 仍可构造**

先运行现有 AppState 测试：

```bash
cd src-tauri && cargo test --lib app::state::tests::session_languages_init_from_config
```

预期：当前通过；在本任务步骤 2 改签名后会失败，步骤 3 修复。

- [ ] **步骤 2：修改 `AppState` 结构**

`src-tauri/src/app/state.rs` import 增加：

```rust
use crate::core::history::HistoryStore;
```

`AppState` 字段增加：

```rust
pub history_store: HistoryStore,
```

`AppState::new` 签名改为：

```rust
pub fn new(config_store: ConfigStore, history_store: HistoryStore) -> Self {
```

初始化结构体增加：

```rust
history_store,
```

在 `impl AppState` 内增加测试构造器：

```rust
#[cfg(test)]
pub fn new_for_test(config_store: ConfigStore) -> Self {
    Self::new(config_store, HistoryStore::in_memory_for_test())
}
```

把 `src-tauri/src/app/state.rs` 和 `src-tauri/src/ui/web_popup.rs` 测试里的 `AppState::new(config_store)` 改为：

```rust
AppState::new_for_test(config_store)
```

- [ ] **步骤 3：启动时加载 HistoryStore**

`src-tauri/src/lib.rs` import 改为：

```rust
use core::{config::ConfigStore, history::HistoryStore};
```

setup 中替换：

```rust
let config_store = ConfigStore::load(app.handle())
    .map_err(|error| tauri::Error::Anyhow(error.into()))?;
app.manage(AppState::new(config_store));
```

为：

```rust
let config_store = ConfigStore::load(app.handle())
    .map_err(|error| tauri::Error::Anyhow(error.into()))?;
let history_store = HistoryStore::load(app.handle())
    .map_err(|error| tauri::Error::Anyhow(error.into()))?;
app.manage(AppState::new(config_store, history_store));
```

- [ ] **步骤 4：新增 command 文件**

创建 `src-tauri/src/ui/history.rs`：

```rust
use crate::{app::state::AppState, core::history::HistorySessionDto};

#[tauri::command]
pub async fn list_translation_history(
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<HistorySessionDto>, String> {
    let config = state.config_store.get().map_err(|error| error.to_string())?;
    let limit = limit.unwrap_or(config.history_limit).max(1);
    state
        .history_store
        .list_recent(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn clear_translation_history(
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .history_store
        .clear()
        .map_err(|error| error.to_string())
}
```

`src-tauri/src/ui/mod.rs` 增加：

```rust
pub mod history;
```

`src-tauri/src/lib.rs` 的 `use ui::{ ... }` 增加：

```rust
history::{clear_translation_history, list_translation_history},
```

`tauri::generate_handler!` 增加：

```rust
list_translation_history,
clear_translation_history,
```

- [ ] **步骤 5：运行构建和测试**

运行：

```bash
cd src-tauri && cargo test --lib app::state::tests ui::web_popup::tests core::history
cd src-tauri && cargo build
```

预期：全部 PASS，构建成功。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/app/state.rs src-tauri/src/lib.rs src-tauri/src/ui/mod.rs src-tauri/src/ui/history.rs src-tauri/src/ui/web_popup.rs
git commit -m "feat(history): 在应用状态中接入历史存储"
```

---

## 任务 4：在统一翻译入口写入历史

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`
- 修改：`src-tauri/src/core/history/types.rs`

- [ ] **步骤 1：补历史 session/result 构造 helper**

在 `src-tauri/src/core/history/types.rs` 追加：

```rust
use crate::core::{
    config::ServiceInstanceConfig,
    translation::TranslationRequest,
};

impl NewHistorySession {
    pub fn from_translation(
        batch_id: &str,
        input: &TranslationInput,
        source_lang: String,
        target_lang: String,
        created_at: String,
    ) -> Self {
        Self {
            id: batch_id.to_string(),
            batch_id: batch_id.to_string(),
            trigger: history_trigger_for_input(input),
            source_lang,
            target_lang,
            source_text: input.text().to_string(),
            created_at,
        }
    }
}

impl NewHistoryResult {
    pub fn from_request(request: &TranslationRequest, service: &ServiceInstanceConfig, session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            service_instance_id: request.service.service_instance_id.clone(),
            service_name: request.service.service_name.clone(),
            service_type: request.service.service_type.clone(),
            protocol: request.service.protocol.clone(),
            model_name: service.model.clone(),
        }
    }
}
```

在 tests 中追加：

```rust
#[test]
fn new_history_session_uses_batch_id_as_session_id() {
    let item = NewHistorySession::from_translation(
        "batch-1",
        &TranslationInput::ManualText("hello".to_string()),
        "auto".to_string(),
        "zh-CN".to_string(),
        "2026-07-11T00:00:00Z".to_string(),
    );

    assert_eq!(item.id, "batch-1");
    assert_eq!(item.batch_id, "batch-1");
    assert_eq!(item.trigger, HistoryTrigger::Manual);
    assert_eq!(item.source_text, "hello");
}
```

运行：`cd src-tauri && cargo test --lib core::history::types::tests`

预期：PASS。

- [ ] **步骤 2：在 `web_popup.rs` 增加历史写入工具函数**

`src-tauri/src/ui/web_popup.rs` import 的 `core::{ ... }` 中增加：

```rust
history::{NewHistoryResult, NewHistorySession},
```

文件中 `cache_automatic_source_text_for_popup` 后追加：

```rust
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn log_history_error(action: &str, error: impl std::fmt::Display) {
    log::warn!("历史写入失败: {action}: {error}");
}
```

- [ ] **步骤 3：创建历史 session 和 pending result**

在 `start_translation_from_input` 中，保留 `session_source_lang` / `session_target_lang` 的 clone：

```rust
let (session_source_lang, session_target_lang) = state.session_languages();
let requests = batch::build_batch_requests(
    input.clone(),
    session_target_lang.clone(),
    session_source_lang.clone(),
    &config.services,
    &batch_id,
)?;
```

在 `cache_automatic_source_text_for_popup` 成功后、sleep 前增加：

```rust
let history_session = NewHistorySession::from_translation(
    &batch_id,
    &input,
    session_source_lang,
    session_target_lang,
    now_iso(),
);
if let Err(error) = state.history_store.create_session(&history_session) {
    log_history_error("create_session", error);
}
```

把 Started 循环改为 zip：

```rust
for (request, service_config) in requests.iter().zip(enabled_services.iter()) {
    let history_result = NewHistoryResult::from_request(request, service_config, &batch_id);
    if let Err(error) = state.history_store.upsert_pending_result(&history_result) {
        log_history_error("upsert_pending_result", error);
    }

    emit_translation_event(
        &app,
        TranslationEvent::Started {
            session_id: request.session_id.clone(),
            service: request.service.clone(),
            source_text: request.source_text().to_string(),
            source_type: request.input.kind().to_string(),
        },
    )
    .map_err(|error| {
        let _ = state.finish_translation_if_current(generation);
        error.to_string()
    })?;
}
```

- [ ] **步骤 4：在 async job 中更新 success/error/cancelled**

在 spawn 前增加：

```rust
let history_store = state.history_store.clone();
let history_batch_id = batch_id.clone();
let history_limit = config.history_limit;
```

在 `jobs` map 闭包内增加 clone：

```rust
let history_store = history_store.clone();
let history_batch_id = history_batch_id.clone();
```

把 `translate_with` 的事件回调改为：

```rust
let result = translation_service
    .translate_with(request, collect_usage, cancel, |event| {
        match &event {
            TranslationEvent::Finished { service, full_text, usage, .. } => {
                if let Err(error) = history_store.mark_success(
                    &history_batch_id,
                    &service.service_instance_id,
                    full_text,
                    usage.as_ref(),
                ) {
                    log_history_error("mark_success", error);
                }
            }
            TranslationEvent::Cancelled { service, .. } => {
                if let Err(error) = history_store.mark_cancelled(
                    &history_batch_id,
                    &service.service_instance_id,
                ) {
                    log_history_error("mark_cancelled", error);
                }
            }
            _ => {}
        }
        let _ = emit_translation_event(&app_handle, event);
    })
    .await;
```

在 provider 执行失败分支 emit 前增加：

```rust
if let Err(history_error) = history_store.mark_error(
    &history_batch_id,
    &failed_service.service_instance_id,
    &error.to_string(),
) {
    log_history_error("mark_error", history_error);
}
```

在 provider 初始化失败分支 emit 前增加：

```rust
if let Err(history_error) = history_store.mark_error(
    &history_batch_id,
    &failed_service.service_instance_id,
    &message,
) {
    log_history_error("mark_error", history_error);
}
```

`join_all(jobs).await;` 后增加裁剪：

```rust
if let Err(error) = history_store.trim_sessions(history_limit.max(1)) {
    log_history_error("trim_sessions", error);
}
```

- [ ] **步骤 5：运行后端测试和构建**

运行：

```bash
cd src-tauri && cargo test --lib core::history ui::web_popup::tests core::translation::service::tests
cd src-tauri && cargo build
```

预期：全部 PASS，构建成功。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs src-tauri/src/core/history/types.rs
git commit -m "feat(history): 翻译批次写入历史记录"
```

---

## 任务 5：新增前端历史 command helper 和可测数据层

**文件：**
- 修改：`frontend/src/lib/tauri.ts`
- 创建：`frontend/src/settings/history.ts`
- 创建：`frontend/src/settings/history.test.ts`

- [ ] **步骤 1：编写失败测试**

创建 `frontend/src/settings/history.test.ts`：

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  clearHistoryAndReload,
  isEmptyHistory,
  loadHistory,
  resultCardStatus,
  type HistorySession,
} from './history'
import { invokeClearTranslationHistory, invokeListTranslationHistory } from '@/lib/tauri'

vi.mock('@/lib/tauri', () => ({
  invokeListTranslationHistory: vi.fn(),
  invokeClearTranslationHistory: vi.fn(),
}))

const session: HistorySession = {
  id: 'batch-1',
  timestamp: '2026-07-11T00:00:00Z',
  trigger: 'manual',
  sourceLang: 'auto',
  targetLang: 'zh-CN',
  source: 'hello',
  results: [
    {
      serviceInstanceId: 'svc-a',
      serviceName: 'DeepSeek',
      serviceType: 'deepseek',
      protocol: 'openai_chat',
      modelName: 'deepseek-chat',
      translation: '你好',
      errorMessage: '',
      status: 'success',
      inputTokens: 1,
      outputTokens: 2,
    },
  ],
}

beforeEach(() => {
  vi.clearAllMocks()
})

describe('history data helpers', () => {
  it('空数组被识别为空状态', () => {
    expect(isEmptyHistory([])).toBe(true)
  })

  it('loadHistory 读取后端 session', async () => {
    vi.mocked(invokeListTranslationHistory).mockResolvedValue([session])

    await expect(loadHistory()).resolves.toEqual([session])
    expect(invokeListTranslationHistory).toHaveBeenCalledWith(undefined)
  })

  it('clearHistoryAndReload 先清空再刷新', async () => {
    vi.mocked(invokeListTranslationHistory).mockResolvedValue([session])

    await expect(clearHistoryAndReload()).resolves.toEqual([session])

    expect(invokeClearTranslationHistory).toHaveBeenCalledTimes(1)
    expect(invokeListTranslationHistory).toHaveBeenCalledTimes(1)
  })

  it('结果状态映射到 ResultCardView 状态', () => {
    expect(resultCardStatus({ ...session.results[0], status: 'success' })).toBe('success')
    expect(resultCardStatus({ ...session.results[0], status: 'pending' })).toBe('pending')
    expect(resultCardStatus({ ...session.results[0], status: 'error' })).toBe('error')
    expect(resultCardStatus({ ...session.results[0], status: 'cancelled' })).toBe('aborted')
  })
})
```

运行：`npm run test -- frontend/src/settings/history.test.ts`

预期：失败，缺少 `./history` 和历史 invoke helper。

- [ ] **步骤 2：实现 `tauri.ts` helper**

`frontend/src/lib/tauri.ts` 追加：

```ts
export type HistoryTrigger = 'selection' | 'manual' | 'screenshot'
export type HistoryResultStatus = 'success' | 'error' | 'cancelled' | 'pending'

export interface HistoryResultDto {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  protocol: string
  modelName: string
  translation: string
  errorMessage: string
  status: HistoryResultStatus
  inputTokens: number | null
  outputTokens: number | null
}

export interface HistorySessionDto {
  id: string
  timestamp: string
  trigger: HistoryTrigger
  sourceLang: string
  targetLang: string
  source: string
  results: HistoryResultDto[]
}

export async function invokeListTranslationHistory(limit?: number): Promise<HistorySessionDto[]> {
  return requireInvoke()<HistorySessionDto[]>('list_translation_history', { limit })
}

export async function invokeClearTranslationHistory(): Promise<void> {
  return requireInvoke()<void>('clear_translation_history')
}
```

- [ ] **步骤 3：实现 `settings/history.ts`**

创建 `frontend/src/settings/history.ts`：

```ts
import {
  invokeClearTranslationHistory,
  invokeListTranslationHistory,
  type HistoryResultDto,
  type HistorySessionDto,
  type HistoryTrigger,
} from '@/lib/tauri'

export type HistoryResult = HistoryResultDto
export type HistorySession = HistorySessionDto
export type { HistoryTrigger }

export type ResultCardStatus = 'success' | 'loading' | 'pending' | 'error' | 'aborted'

export const isEmptyHistory = (sessions: HistorySession[]): boolean => sessions.length === 0

export const loadHistory = (limit?: number): Promise<HistorySession[]> =>
  invokeListTranslationHistory(limit)

export const clearHistoryAndReload = async (): Promise<HistorySession[]> => {
  await invokeClearTranslationHistory()
  return loadHistory()
}

export const resultCardStatus = (result: HistoryResult): ResultCardStatus => {
  if (result.status === 'cancelled') return 'aborted'
  return result.status
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npm run test -- frontend/src/settings/history.test.ts`

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/lib/tauri.ts frontend/src/settings/history.ts frontend/src/settings/history.test.ts
git commit -m "feat(history): 增加前端历史数据访问层"
```

---

## 任务 6：HistoryPanel 改为后端真实数据源

**文件：**
- 修改：`frontend/src/settings/panels/HistoryPanel.vue`

- [ ] **步骤 1：删除 mock 与 `ocrHistory` 适配类型**

把 imports 改为：

```ts
import { computed, onBeforeUnmount, onMounted, reactive, ref, watchEffect } from 'vue'
import { History as HistoryIcon, Trash2, Camera, ScanText, MousePointerSquareDashed, PencilLine, Layers } from '@lucide/vue'
import { Button } from '@/components/ui/button'
import Dialog from '@/components/ui/dialog/Dialog.vue'
import { toast } from '@/lib/toast'
import { speakText } from '@/popup/composables/utils'
import { LANGUAGES } from '../tokens'
import SourceCardView from '@/popup/components/SourceCardView.vue'
import ResultCardView from '@/popup/components/ResultCardView.vue'
import LanguageToolbar from '@/popup/components/LanguageToolbar.vue'
import type { AppSettings } from '../types'
import {
  clearHistoryAndReload,
  isEmptyHistory,
  loadHistory,
  resultCardStatus,
  type HistoryResult,
  type HistorySession,
  type HistoryTrigger,
} from '../history'
```

删除本地 `HistoryTrigger`、`HistoryResult`、`HistorySession`、`MOCK_SESSIONS`、`mockDismissed`、`adaptedSessions` 定义。

- [ ] **步骤 2：增加后端数据状态**

在 `showClearConfirm` 后增加：

```ts
const sessions = ref<HistorySession[]>([])
const loading = ref(false)
const loadError = ref('')
```

增加加载函数：

```ts
const refreshHistory = async (): Promise<void> => {
  loading.value = true
  loadError.value = ''
  try {
    sessions.value = await loadHistory(props.state.translation.historyLimit)
  } catch (err) {
    sessions.value = []
    loadError.value = err instanceof Error ? err.message : String(err)
    toast.error('读取翻译历史失败', loadError.value)
  } finally {
    loading.value = false
  }
}
```

把 `isEmpty` / `activeSession` 改为：

```ts
const isEmpty = computed(() => isEmptyHistory(sessions.value))
const activeSession = computed<HistorySession | null>(() =>
  activeId.value ? sessions.value.find((s) => s.id === activeId.value) ?? null : null,
)
```

把所有 `adaptedSessions.value` 替换为 `sessions.value`。

`onMounted` 里开头增加：

```ts
void refreshHistory()
```

- [ ] **步骤 3：收窄 trigger 列表并删除剪贴板入口**

`TRIGGER_META` 改为：

```ts
const TRIGGER_META: Record<HistoryTrigger, { label: string; icon: typeof Camera }> = {
  selection: { label: '划词翻译', icon: MousePointerSquareDashed },
  manual: { label: '手动输入', icon: PencilLine },
  screenshot: { label: '截图翻译', icon: ScanText },
}
```

`FILTERS` 改为：

```ts
const FILTERS = [
  { id: 'all' as const, label: '全部', icon: Layers },
  { id: 'screenshot' as const, label: '截图翻译', icon: ScanText },
  { id: 'selection' as const, label: '划词翻译', icon: MousePointerSquareDashed },
  { id: 'manual' as const, label: '手动输入', icon: PencilLine },
]
```

- [ ] **步骤 4：清空按钮调用后端 command 并刷新**

把 `clearAll` 改为：

```ts
const clearAll = async (): Promise<void> => {
  try {
    sessions.value = await clearHistoryAndReload()
    showClearConfirm.value = false
    activeId.value = ''
    toast.success('已清空翻译历史')
  } catch (err) {
    toast.error('清空翻译历史失败', err instanceof Error ? err.message : String(err))
  }
}
```

模板清空按钮改为：

```vue
<Button variant="destructive" size="sm" @click="clearAll">
```

- [ ] **步骤 5：更新状态文案并删除开发中提示条**

删除顶部 amber “此功能正在开发中”提示条，保留右侧清空按钮，替换为：

```vue
<div class="flex items-center justify-end">
  <Button variant="ghost" size="sm" :disabled="isEmpty || loading" class="text-muted-foreground hover:text-destructive" @click="showClearConfirm = true">
    <Trash2 class="h-3.5 w-3.5" />
    清空全部
  </Button>
</div>
```

空状态文案改为：

```vue
<p class="text-sm font-medium text-foreground">暂无翻译历史</p>
<p class="text-[12px] text-muted-foreground">手动输入、划词或截图 OCR 翻译后，结果会自动保存在这里。</p>
```

清空确认描述改为：

```vue
description="此操作不可撤销，所有翻译历史都会被永久删除。"
```

在空状态前增加加载/失败状态：

```vue
<div v-if="loading" class="flex flex-col items-center justify-center gap-2 rounded-lg border border-dashed border-border py-16 text-center text-muted-foreground">
  <HistoryIcon class="h-5 w-5" />
  <p class="text-sm">正在加载翻译历史...</p>
</div>

<div v-else-if="loadError" class="flex flex-col items-center justify-center gap-3 rounded-lg border border-dashed border-destructive/40 py-16 text-center">
  <HistoryIcon class="h-5 w-5 text-destructive" />
  <div class="flex flex-col gap-1">
    <p class="text-sm font-medium text-foreground">翻译历史加载失败</p>
    <p class="text-[12px] text-muted-foreground">{{ loadError }}</p>
  </div>
  <Button variant="outline" size="sm" @click="refreshHistory">重试</Button>
</div>
```

原 `v-if="isEmpty"` 改为 `v-else-if="isEmpty"`，原 `v-else` 模板改为 `v-else`。

- [ ] **步骤 6：服务类型和卡片状态使用后端 DTO**

`serviceTypeOf` 改为优先后端：

```ts
const serviceTypeOf = (r: HistoryResult): string => {
  if (r.serviceType) return r.serviceType
  const inst = props.state.services.find((s) => s.id === r.serviceInstanceId)
  return inst?.type ?? r.serviceInstanceId
}
```

`cardStatus` 改为：

```ts
const cardStatus = (r: HistoryResult): 'success' | 'loading' | 'pending' | 'error' | 'aborted' =>
  resultCardStatus(r)
```

`ResultCardView` 的 text 改为优先错误信息：

```vue
:text="r.status === 'error' ? r.errorMessage : r.translation"
```

copy/speak 也使用同一个表达式：

```vue
@copy="copy(r.status === 'error' ? r.errorMessage : r.translation)"
@speak="speak(r.status === 'error' ? r.errorMessage : r.translation)"
```

- [ ] **步骤 7：运行前端验证**

运行：

```bash
npm run typecheck
npm run test -- frontend/src/settings/history.test.ts
```

预期：PASS。

- [ ] **步骤 8：Commit**

```bash
git add frontend/src/settings/panels/HistoryPanel.vue
git commit -m "feat(history): 设置页历史面板读取后端数据"
```

---

## 任务 7：删除旧 `ocrHistory` 本地适配和历史 wip 标记

**文件：**
- 修改：`frontend/src/settings/types.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/settings/SettingsSidebar.vue`

- [ ] **步骤 1：删除类型中的 `OcrHistoryEntry`**

在 `frontend/src/settings/types.ts` 删除 `OcrHistoryEntry` interface，并从 `AppSettings` 删除：

```ts
ocrHistory: OcrHistoryEntry[]
```

- [ ] **步骤 2：删除 store 中旧历史逻辑**

在 `frontend/src/settings/stores/settings.ts`：

删除 import 中的 `OcrHistoryEntry`。

删除：

```ts
const newHistoryId = (): string => `hist-${newInstanceId().slice(5)}`
```

删除：

```ts
const seedOcrHistory = (): OcrHistoryEntry[] => {
  return []
}
```

`buildDefaults()` 删除：

```ts
ocrHistory: seedOcrHistory(),
```

`loadFromStorage()` 返回对象删除：

```ts
ocrHistory: parsed.ocrHistory ?? defaults.ocrHistory,
```

`serializeForDirty()` 删除 `ocrHistory: undefined`。

`useSettings()` 删除 `addHistory`、`removeHistory`、`clearHistory` 三个方法。

- [ ] **步骤 3：更新前端测试 fixture**

`frontend/src/lib/config.test.ts` 的 `makeState` 删除：

```ts
ocrHistory: [],
```

`frontend/src/settings/stores/settings.test.ts` 删除“首次启动 OCR 历史为空”用例，并给所有 mocked `AppConfig` 补 `historyLimit: 500`。

- [ ] **步骤 4：去掉侧边栏 wip**

`frontend/src/settings/SettingsSidebar.vue` 中 history 分类改为：

```ts
{
  id: 'history',
  label: '翻译历史',
  description: '查看最近翻译记录',
  icon: HistoryIcon,
},
```

- [ ] **步骤 5：运行验证**

运行：

```bash
npm run test -- frontend/src/lib/config.test.ts frontend/src/settings/stores/settings.test.ts frontend/src/settings/history.test.ts
npm run typecheck
```

预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/types.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts frontend/src/lib/config.test.ts frontend/src/settings/SettingsSidebar.vue
git commit -m "refactor(history): 删除旧 OCR 历史适配层"
```

---

## 任务 8：全量验证与文档同步

**文件：**
- 修改：`README.md`
- 修改：`AGENTS.md`
- 修改：`CLAUDE.md`
- 修改：`docs/superpowers/plans/2026-07-11-translation-history.md`

- [ ] **步骤 1：运行后端验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
```

预期：测试全部 PASS，构建成功。

- [ ] **步骤 2：运行前端验证**

运行：

```bash
npm run test
npm run typecheck
npm run build
```

预期：测试全部 PASS，类型检查和构建成功。

- [ ] **步骤 3：手动验证 Windows 工作流**

启动：

```bash
npm run tauri dev
```

验证：

1. 手动输入翻译完成后，打开设置页「翻译历史」，出现一条 `manual` session，包含所有启用服务结果。
2. `Alt+D` 划词翻译完成后，历史出现 `selection` session。
3. `Alt+E` 截图 OCR 翻译完成后，历史出现 `screenshot` session。
4. 多服务批次只出现一条 session，右侧展示多个结果。
5. 禁用或填错一个服务，使该服务失败；历史中该服务结果状态为错误并展示错误信息，其他服务结果保留。
6. 翻译中点击取消；历史中对应结果状态为取消。
7. 清空历史按钮调用后端清空，刷新后显示“暂无翻译历史”。
8. `translation.historyLimit` 改小后执行多次翻译，旧 session 被裁剪。

- [ ] **步骤 4：同步 README**

在 `README.md` 当前能力列表增加：

```markdown
- 翻译历史：手动输入、划词和截图 OCR 翻译都会按批次保存到本机 SQLite，支持多服务结果、失败信息和一键清空。
```

在配置章节把历史描述改为：

```markdown
- 历史（最近翻译记录，覆盖手动输入 / 划词 / 截图 OCR，多服务结果存储到本机 SQLite）
```

在限制中删除“历史实现中”相关描述，只保留不属于本轮的限制。

- [ ] **步骤 5：同步 AGENTS.md 与 CLAUDE.md**

在两个文件的项目结构中 `src-tauri/src/core/` 区域增加：

```markdown
  src/core/history/ SQLite 翻译历史：session/result 两表，HistoryStore 聚合查询与裁剪
```

在架构关键点增加：

```markdown
- **翻译历史**：历史数据由后端 `core/history::HistoryStore` 写入 `app_config_dir()/history.sqlite3`，`AppState` 与 `ConfigStore` 同级持有 store。`web_popup.rs` 仅在统一翻译入口触发 session/result 写入，不包含 SQL；设置页通过 `list_translation_history` / `clear_translation_history` 查询和清空，不再保存 `ocrHistory` 到前端 localStorage。
```

保持 `AGENTS.md` 与 `CLAUDE.md` 内容同步。

- [ ] **步骤 6：回填本计划复选框**

在 `docs/superpowers/plans/2026-07-11-translation-history.md` 中把已完成步骤由 `- [ ]` 改为 `- [x]`。

- [ ] **步骤 7：Commit**

```bash
git add README.md AGENTS.md CLAUDE.md docs/superpowers/plans/2026-07-11-translation-history.md
git commit -m "docs(history): 同步翻译历史落地状态"
```

---

## 自检

**1. 规格覆盖度：**
- Rust 后端 SQLite 历史模块，`rusqlite` bundled：任务 2 覆盖。
- `AppState` 持有 `HistoryStore`：任务 3 覆盖。
- `web_popup.rs` 只作为写入触发点，SQL 在 `core/history`：任务 2 和任务 4 覆盖。
- `list_translation_history` / `clear_translation_history` commands：任务 3 覆盖。
- `HistoryPanel` 改成后端真实数据源：任务 5 和任务 6 覆盖。
- 删除 mock 历史、`ocrHistory` 适配层和开发中标记：任务 6 和任务 7 覆盖。
- `SettingsSidebar` 去掉 wip badge 并更新描述：任务 7 覆盖。
- 测试与验证命令：任务 1-8 每组有命令，任务 8 有全量验证。
- 实现完成后的 README、AGENTS.md、CLAUDE.md 文档同步：任务 8 覆盖。

**2. 占位符扫描：** 未发现占位写法；每个代码改动步骤给出目标路径、命令和预期结果。

**3. 类型一致性：**
- 后端 DTO 字段使用 camelCase 序列化，前端 `HistorySessionDto` 字段名一致。
- trigger 使用 `manual` / `selection` / `screenshot`，前后端一致。
- result status 使用 `pending` / `success` / `error` / `cancelled`，前后端一致；前端只在传给 `ResultCardView` 时把 `cancelled` 映射为 `aborted`。
- session 主键统一为 `batch_id`，result 通过 `(session_id, service_instance_id)` upsert。
- `historyLimit` 前端字段为 `historyLimit`，Rust 字段为 `history_limit`，serde camelCase 对齐。

无遗漏，类型一致。
