# 翻译历史完整实现设计规格

日期：2026-07-11

## 背景

当前翻译历史已经有设置页入口和 `HistoryPanel.vue` 双栏展示 UI，但数据链路仍是临时实现：

- 历史面板读取 `state.ocrHistory`，数据存放在前端 `localStorage`。
- 现有 `ocrHistory` 只表达截图 OCR 单结果，不能覆盖手动输入、划词、多服务批量翻译。
- 面板内存在 mock 演示数据和“此功能正在开发中”提示。
- 设置侧边栏“翻译历史”仍带 `wip` 标记，描述也写成“仅展示截图翻译记录”。
- 后端翻译入口 `start_translation_from_input` 已统一覆盖 `manualText`、`selectedText`、`ocrText`，但未把翻译事件写入历史。

本轮目标是把翻译历史作为正式功能落地，不兼容旧 `ocrHistory` 数据，也不迁移旧 localStorage 数据。

## 目标

1. 所有正常进入翻译链路的请求都写入历史：
   - 手动输入翻译
   - 划词翻译
   - 截图 OCR 翻译
2. 多服务批次翻译按一次 session 展示，session 下包含多个服务结果。
3. 成功、失败、取消结果都可记录，失败记录保留错误信息，便于排查服务配置。
4. 历史数据持久化到本机 SQLite 数据库，设置页只通过 Tauri command 查询和清空。
5. 复用现有 `HistoryPanel.vue` 布局，删除 mock 数据和开发中标记。
6. 历史数量遵循现有 `translation.historyLimit` 配置，默认 500 条 session。

## 非目标

- 不迁移旧 `state.ocrHistory` / localStorage 历史数据。
- 不实现全文搜索、收藏、导出、单条删除、按服务筛选。
- 不让前端直接访问 SQLite。
- 不引入远端同步或账号体系。

## 存储选择

采用 Rust 后端 `rusqlite` 直接访问 SQLite。

推荐依赖：

```toml
rusqlite = { version = "0.32", features = ["bundled"] }
```

原因：

- 本地桌面单用户场景，`rusqlite` 足够。
- `bundled` 避免 Windows 环境依赖系统 SQLite。
- 不需要 `sqlx` 的异步连接池和编译期 SQL 校验。
- 不需要 `tauri-plugin-sql`，历史写入应由后端翻译链路统一触发，前端只负责展示。

## 后端设计

新增模块：

```text
src-tauri/src/core/history/
  mod.rs
  store.rs
  types.rs
```

`HistoryStore` 负责：

- 初始化数据库文件，建议路径为 `app_config_dir()/history.sqlite3`。
- 首次启动创建表和索引。
- 创建 session。
- upsert 单个服务结果。
- 查询最近 session 列表及其结果。
- 清空全部历史。
- 按 `historyLimit` 裁剪旧 session。

`AppState` 持有 `HistoryStore`，与 `ConfigStore` 同级，不放进前端 settings store。

## SQLite 表结构

最小两表：

```sql
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
```

`trigger` 映射：

- `manualText` -> `manual`
- `selectedText` -> `selection`
- `ocrText` -> `screenshot`

## 写入链路

写入触发点接在 `src-tauri/src/ui/web_popup.rs`，但 SQLite 代码不写在该文件内。

实际分层：

```text
src-tauri/src/ui/web_popup.rs
  翻译批次编排，调用 HistoryStore

src-tauri/src/core/history/
  SQLite 读写与 DTO
```

写入时机：

1. `start_translation_from_input` 构建 batch requests 后，创建一条 session。
2. 发送每个 `Started` 事件时创建对应 result，占位状态为 `pending`。
3. `Finished` 事件更新 result 为 `success`，写入 `full_text`、usage、完成时间。
4. `Failed` 事件更新 result 为 `error`，写入错误信息。
5. `Cancelled` 事件更新 result 为 `cancelled`。
6. `join_all` 完成后按 `historyLimit` 删除旧 session。

如果历史写入失败，只记录后端日志，不阻断翻译主流程。

## Tauri Commands

新增命令：

```rust
list_translation_history(limit: Option<usize>) -> Result<Vec<HistorySessionDto>, String>
clear_translation_history() -> Result<(), String>
```

DTO 使用 camelCase，直接匹配前端面板需要的形状：

```ts
type HistoryTrigger = 'selection' | 'manual' | 'screenshot'

interface HistorySessionDto {
  id: string
  timestamp: string
  trigger: HistoryTrigger
  sourceLang: string
  targetLang: string
  source: string
  results: HistoryResultDto[]
}

interface HistoryResultDto {
  serviceInstanceId: string
  serviceName: string
  serviceType: string
  protocol: string
  modelName: string
  translation: string
  errorMessage: string
  status: 'success' | 'error' | 'cancelled' | 'pending'
  inputTokens: number | null
  outputTokens: number | null
}
```

## 前端设计

保留 `frontend/src/settings/panels/HistoryPanel.vue` 的双栏布局和复用组件：

- `SourceCardView`
- `LanguageToolbar`
- `ResultCardView`

改动：

- 删除 `MOCK_SESSIONS`。
- 删除 `OcrHistoryEntry` 适配层。
- 面板挂载时调用 `list_translation_history`。
- 清空按钮调用 `clear_translation_history` 后刷新列表。
- 空状态文案改为通用翻译历史，不再限定截图。
- 侧边栏 `SettingsSidebar.vue` 删除 `history` 分类的 `badge: 'wip'`，描述改为“查看最近翻译记录”。
- 顶部“此功能正在开发中”提示条删除。

当前不需要实时订阅历史变化。设置页打开时拉取一次，清空后刷新一次即可。翻译弹窗和设置页通常不是同一工作流，实时同步可后续按需要添加。

## 配置关系

保留现有 `translation.historyLimit` 设置项，用于限制最多保留的历史 session 数。

裁剪由后端执行：

- 每次翻译批次结束后裁剪。
- 保存配置时不主动裁剪，避免设置保存路径额外碰数据库。

## 错误处理

- SQLite 初始化失败：应用启动应失败并返回清晰错误，因为正式历史功能依赖数据库可用。
- 单次历史写入失败：翻译不失败，仅记录日志。
- 查询历史失败：设置页 toast 错误并显示空状态。
- 清空历史失败：toast 错误，不修改当前列表。

## 测试策略

后端单元测试：

- `HistoryStore` 初始化会创建表。
- 创建 session + 多 result 后，查询能按时间倒序返回聚合结构。
- `Finished` / `Failed` / `Cancelled` 更新状态正确。
- `historyLimit` 裁剪会删除旧 session 及其 results。
- `trigger` 映射正确。

前端测试：

- `HistoryPanel` 空状态渲染。
- 有 session 时展示触发类型、原文、多个服务结果。
- 清空按钮调用 command 并刷新列表。

验证命令：

```bash
cd src-tauri && cargo test
npm run typecheck
npm run test
npm run build
```

## 文档收尾

实现完成后同步：

- `README.md` 当前能力，补“翻译历史已支持手动/划词/截图 OCR，多服务结果存 SQLite”。
- `AGENTS.md` 与 `CLAUDE.md` 架构关键点，说明历史模块与 SQLite 存储。
- 相关 plan 复选框。
