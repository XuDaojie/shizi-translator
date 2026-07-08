# 高级日志系统设计

## 背景

当前后端仅 1 处 `eprintln!`（`core/config/store.rs:56`），无任何日志框架，`Cargo.toml` 无 `log`/`tracing` 依赖；前端零 `console.log`，无日志机制。设置页 [AdvancedPanel.vue](../../../frontend/src/settings/panels/AdvancedPanel.vue) 已有「日志」分组占位（日志等级下拉 + 导出按钮），但导出按钮无 `@click`、等级选择未接后端，后端 `AppConfig` 也无 `logLevel` 字段。

本轮目标：为前后端各建一套日志，**物理分开保存**到独立文件，主要服务于「用户遇到问题 → 导出日志 → 开发者分析」的排查链路。core 业务层基本纯 Rust（仅 `config/store.rs` 为拿路径依赖了 `tauri::Manager`，属范围外技术债，本次不动）。

## 目标范围

### 必须实现

- 后端日志：core 层用 `log` 标准门面打日志，装配层注册 `tauri-plugin-log` 作为 backend，写入 `logs/Shizi.log`（tauri-plugin-log 按 productName 默认文件名，不支持自定义）。
- 前端日志：`frontend/public/logger.js`（纯 ES module）内存环形缓冲 + 批量 invoke `write_frontend_log` command，写入 `logs/frontend.log`。
- 分开保存：`Shizi.log` 与 `frontend.log` 物理隔离，互不混入。
- 日志等级（error/warn/info/debug）运行时切换即时生效，无需重启。
- API Key 永远脱敏（前 4 + 后 4）；翻译正文 info 级别记摘要（长度 + 前 20 字）、debug 级别记全文。
- 按大小 5MB 轮转（`KeepAll`）+ 启动时清理 >7 天文件。
- 导出 zip：`Shizi.log*` + `frontend.log*` + `config-snapshot.json`（apiKey 脱敏）+ `system-info.txt`。

### 明确不做

- 不做按日期轮转（`tauri-plugin-log` 不支持，自建 appender 违背选型 A 的核心优势）。
- 不做日志远程上报 / 云端收集。
- 不做请求级 `tracing` span 追踪（tracing 方案已排除）。
- 不重构 `config/store.rs` 的 `tauri::Manager` 依赖（范围外技术债）。
- 不做应用内日志查看器（只做导出到文件）。

## 数据模型

后端 `AppConfig` 增加 `log_level: String`（camelCase `logLevel`，默认 `"info"`）。

| 字段 | 取值 | 映射 |
| --- | --- | --- |
| `log_level` | `error` / `warn` / `info` / `debug` | `log::LevelFilter::Error` / `Warn` / `Info` / `Debug` |

归一化规则：

- 缺失或非法值回退 `"info"`。
- 前后端字段保持 camelCase JSON；Rust 内部 snake_case。
- 纳入现有 `save_app_config` / `app-config:changed` 链路，无新事件；前后端配置同步沿用 `mergeBackendIntoServices` 同款按字段合并，`logLevel` 作为后端权威字段。

前端 `AdvancedSettings.logLevel`（`types.ts:25`）已存在，仅需在 `frontend/src/types/config.ts` 的 `AppConfig` 同步新增 `logLevel` 字段。

## 后端设计

### 分层

| 层 | 依赖 | 职责 |
| --- | --- | --- |
| core 层（llm/translation/ocr/selection/capture…） | 仅 `log` crate | 打 `log::info!`/`log::error!`，不耦合 Tauri |
| `core/logging.rs`（新） | 无 | 纯函数 `redact_api_key` / `redact_text` |
| 装配层 `app/logging.rs`（新） | `tauri-plugin-log` | `init_logging` + `cleanup_old_logs` |
| UI 桥 `ui/logging.rs`（新） | `tauri-plugin-dialog` / `zip` | `write_frontend_log` / `export_logs` command |

### `init_logging(app_handle, log_level)`

- 算日志目录 = `app_config_dir()/logs/`。
- `tauri-plugin-log` 配置：`Target::Folder(logs)` + `max_file_size(5MB)` + `RotationStrategy::KeepAll` + `level(log_level)`。
- 文件名 `Shizi.log`（tauri-plugin-log 按 productName 固定，不支持自定义），轮转产生 `Shizi_<timestamp>.log`（日期格式，KeepAll）。
- best-effort：失败 `eprintln!` 兜底，不阻止启动。

### 运行时切换等级

`save_app_config` 保存 `logLevel` 后，调 `log::set_max_level()` 即时生效（`log` facade 全局 filter，无需重启插件），随后 `emit("app-config:changed")` 通知前端。

### `write_frontend_log(entries: Vec<FrontendLogEntry>, state)`

- `FrontendLogEntry { level, message, timestamp, source, meta? }`，`source` 标记 `translate`/`settings`/`overlay`。
- 直接 `std::fs::OpenOptions::append` 写 `frontend.log`，**不走 `log` facade**，确保与 `Shizi.log` 物理隔离。
- 超 5MB 轮转（重命名 `.1`/`.2`…，策略与后端一致）。
- 按当前 `log_level` 过滤（低于等级的丢弃，双保险，前端已过滤但后端再校验一次）。

### `cleanup_old_logs(dir, days=7)`

- 启动时扫描 `logs/`，所有 `*.log*` 文件，`mtime > 7 天` 删除。
- best-effort，失败 `eprintln!` 不阻止启动。

### `export_logs(app, state)`

- 打包 zip：`Shizi.log*` + `frontend.log*` + `config-snapshot.json`（`apiKey` 脱敏）+ `system-info.txt`（app 版本、OS 版本、导出时间、当前 `logLevel`）。
- `tauri-plugin-dialog` save 对话框选保存位置。
- 返回保存路径；失败返回错误供前端 toast。

### 脱敏（`core/logging.rs`）

- `redact_api_key(key) -> String`：前 4 + `...` + 后 4（如 `sk-x...3f2a`）；短于 8 字符则全遮蔽 `****`。
- `redact_text(text, level) -> String`：`info` 及以上 → `"[len=N] 前20字..."`；`debug` → 原文。

## 前端设计

### `logger.js`（`frontend/public/`，纯 ES module）

照 [translate-card-sync.js](../../../frontend/public/translate-card-sync.js) 先例：无依赖纯 ES module，三页 import 同一份，Vite 工程可引用与测试。

- `createLogger(source)` 返回 `{ debug, info, warn, error, redactText }`。
- 内存环形缓冲（容量 1000 条）。
- 批量 flush 触发条件：满 50 条 / 每 2 秒 / `visibilitychange` + `beforeunload`。
- 本地按 `level` 过滤：启动时 `get_app_config` 拿 `logLevel`，订阅 `app-config:changed` 更新本地 `logger.level`。
- `redactText(text)`：按当前 `level` 返回摘要（info）或全文（debug），供调用方处理翻译正文。
- flush 时 `invoke('write_frontend_log', { entries })` 批量提交；invoke 失败重试一次，仍失败丢弃该批，缓冲继续累积。

### 三页接入

- `translate.js`：`import { createLogger } from './logger.js'`，关键流程（翻译开始/取消/重试/失败、卡片状态、IPC 错误）打日志。
- `overlay.html`：内联 `<script type="module">` import，框选提交/取消/IPC 错误打日志。
- settings 页：通过 Vite alias 引入同份 `logger.js`，配置加载/保存/服务校验打日志。

### `AdvancedPanel.vue`

- 日志等级下拉 `v-model` 接 `state.advanced.logLevel`，经 `save_app_config` 持久化（纳入现有保存按钮链路）。
- 导出按钮 `@click` 接 `invokeExportLogs`；若当前日志含 debug 级别正文，导出前提示「导出内容含翻译正文」。

### `frontend/src/lib/tauri.ts`

新增 `invokeWriteFrontendLog(entries)` / `invokeExportLogs()`，沿用现有 `window.__TAURI__.core.invoke` 桥。

## 错误处理

全链路 **best-effort，绝不影响翻译主流程**：

- `init_logging` / `cleanup_old_logs` 失败 → `eprintln!` 兜底，不阻止启动（类比快捷键 best-effort）。
- `write_frontend_log` 失败 → command 返回 Err，前端 logger 静默丢弃该批，缓冲继续累积。
- 导出失败 → 返回错误，前端 toast 提示。
- 日志文件占用/只读 → 追加失败跳过，不崩。

## 验收标准

- 翻译流程产生 `Shizi.log` 与 `frontend.log` 两个独立文件，内容不互相混入。
- 修改日志等级并保存后即时生效（后端 `set_max_level` + 前端 `logger.level` 更新），无需重启。
- API Key 在两份日志中始终脱敏（前 4 + 后 4）。
- `info` 级别翻译正文只记摘要；`debug` 级别记全文。
- 单文件超 5MB 自动轮转，产生 `.1`/`.2`… 备份。
- 启动时清理 `mtime > 7 天` 的日志文件。
- 导出 zip 包含 `Shizi.log*` + `frontend.log*` + `config-snapshot.json`（apiKey 脱敏）+ `system-info.txt`。
- 日志系统任何环节失败不影响翻译、截图、快捷键等主流程。

## 测试与验证

- Rust 单元测试：
  - `redact_api_key` 各种长度（含短 key 全遮蔽）。
  - `redact_text` info 摘要 vs debug 全文。
  - `cleanup_old_logs` 旧文件删 / 新文件留（`tempdir`）。
  - `write_frontend_log` 追加写入 + 5MB 轮转切分（`tempdir`）。
  - `AppConfig.log_level` normalized 非法值回退 `"info"`。
- 前端单元测试（vitest）：
  - `logger.js` 等级过滤、环形缓冲、flush 触发条件、`redactText`。
  - `AdvancedPanel` 等级切换触发 save、导出触发 invoke。
- 构建验证：
  - `cd src-tauri && cargo test`
  - `cd src-tauri && cargo build`
  - `npm run typecheck`
  - `npm run test`
- 手动验证：
  - `npm run tauri dev`，逐项验证验收标准。
  - 改文件 `mtime` 模拟 7 天清理。

## 架构原则

1. core 层只 `use log`，不依赖 `tauri-plugin-log` / Tauri。
2. 日志目录路径由装配层算好，不进 core（避免重蹈 `config/store.rs` 依赖 `tauri::Manager` 的覆辙）。
3. 脱敏责任在产生方：后端代码调 `redact_*` 后再 `log!`；前端调用方调 `logger.redactText` 后再传。
4. 日志系统任何环节失败都 best-effort，绝不影响翻译主流程。

## 配置与能力变更

- `Cargo.toml`：加 `tauri-plugin-log = "2"`、`tauri-plugin-dialog = "2"`、`zip`。
- `capabilities/default.json`：加 `dialog:allow-save`（导出保存框）。
- `AppConfig` 加 `log_level` 字段；前端 `types/config.ts` 同步 `logLevel`。
- 新增 `chrono` 依赖（system-info.txt 导出时间）；`export_logs` 用 `tauri-plugin-dialog` 的 `blocking_save_file()`（async command 推荐用法）。

## 文档同步

编码完成后同步：

- `AGENTS.md` 与 `CLAUDE.md` 的日志能力说明（前后端通信、配置存储等节）。
- `README` 当前能力与限制。
- `docs/roadmap` 进度。
