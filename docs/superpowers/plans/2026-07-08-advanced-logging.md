# 高级日志系统 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为前后端各建一套日志，物理分开保存到 `app_config_dir()/logs/` 下的独立文件，支持运行时等级切换、API Key 与翻译正文脱敏、5MB 轮转 + 启动清理 7 天、导出 zip，服务于「用户导出日志 → 开发者分析」排查链路。

**架构：** 后端 core 层只用 `log` 标准门面打日志，装配层（`app/logging.rs` + `lib.rs` setup）用 `tauri-plugin-log` 作为 backend 写文件；前端 `frontend/public/logger.js` 纯 ES module 内存环形缓冲 + 批量 invoke `write_frontend_log` command，该 command 直接 `std::fs::append` 写独立文件、不走 log facade，确保与后端日志物理隔离。脱敏责任在产生方：后端调 `redact_*` 后再 `log!`，前端调 `logger.redactText` 后再传。日志系统任何环节失败都 best-effort，绝不影响翻译主流程。

**技术栈：** Rust（`log` + `tauri-plugin-log` 2 + `tauri-plugin-dialog` 2 + `zip` 2 + `chrono` 0.4 + `tempfile` 测试）、原生 ES module（`frontend/public/logger.js`）、Vue 3 + vitest。

---

## 与 spec 的实现澄清

spec（`docs/superpowers/specs/2026-07-08-advanced-logging-design.md`）是功能事实来源，以下为实现层澄清，不改变功能要求：

1. **后端日志文件名：`Shizi.log`，非 spec 所写的 `backend.log`。** tauri-plugin-log v2 的 `TargetKind::Folder(PathBuf)` 的 PathBuf 是目录，文件名由插件按 `productName` 固定生成（`tauri.conf.json` 的 `productName = "Shizi"` → `Shizi.log`，轮转 `Shizi.log.1`/`.2`…），不支持自定义文件名。自建 appender 违背 spec「选型 A」核心优势，故接受默认名。功能完全等价：与 `frontend.log` 物理隔离、5MB KeepAll 轮转、导出 zip 收集 `*.log*`、清理扫 `*.log*`。本计划所有「后端日志文件」均指 `Shizi.log*`；执行完成后需回填 README 与 spec 的实际文件名。

2. **后端日志目录：`app_config_dir()/logs/`，贴合 spec。** 通过在 `setup` 闭包里用 `app.handle().plugin(...)` 运行时注册 tauri-plugin-log + `TargetKind::Folder(app_config_dir/logs)` 实现（Tauri 2.11.3 `AppHandle::plugin` 已确认存在于 `tauri-2.11.3/src/app.rs:527`）。`frontend.log` 由 `write_frontend_log` command 写入同一目录。

3. **运行时切换等级即时生效。** tauri-plugin-log 注册时内部 logger level 设 `Debug`（最低，不挡任何记录），全局 `log::set_max_level()` 控制实际输出。`save_app_config` 保存 `logLevel` 后调 `log::set_max_level(new)` 即时生效，无需重启插件。理由：tauri-plugin-log 的 logger 内部 filter 在注册时固定，若注册为 `Info` 则运行时 `set_max_level(Debug)` 后 debug 记录仍被 logger 内部挡住；注册为 `Debug` 后内部不挡，全局 filter 独占控制权。

4. **新增 `chrono` 依赖。** `system-info.txt` 的导出时间用 `chrono::Local::now().to_rfc3339()` 格式化为人类可读时间，spec「配置与能力变更」未列出，本计划补充。

---

## 文件结构

### 新建文件

| 文件 | 职责 |
| --- | --- |
| `src-tauri/src/core/logging.rs` | 纯函数：`redact_api_key`、`redact_text`、`normalize_log_level`（字符串归一化）。无 Tauri 依赖。 |
| `src-tauri/src/app/logging.rs` | 装配层：`logs_dir(app)`、`parse_level_filter`、`init_logging(app, level)`、`cleanup_old_logs(dir, days)`。依赖 `tauri-plugin-log`。 |
| `src-tauri/src/ui/logging.rs` | UI 桥 command：`write_frontend_log`、`export_logs`；frontend.log 追加 + 轮转；zip 打包。依赖 `tauri-plugin-dialog`、`zip`。 |
| `frontend/public/logger.js` | 纯 ES module：`createLogger(source, deps?)`、`redactText`。内存环形缓冲 + 批量 flush。三页共用。 |
| `frontend/src/lib/logger.test.ts` | logger.js 的 vitest 单测（注入 deps，不依赖 window）。 |

### 修改文件

| 文件 | 变更 |
| --- | --- |
| `src-tauri/Cargo.toml` | 加 `log`、`tauri-plugin-log`、`tauri-plugin-dialog`、`zip`、`chrono`；dev-dep 加 `tempfile`。 |
| `src-tauri/src/core/mod.rs` | 加 `pub mod logging;` |
| `src-tauri/src/core/config/types.rs` | `AppConfig` 加 `log_level` 字段 + `normalized` 归一化 + `from_env` 默认 `"info"` + 测试。 |
| `src-tauri/src/app/mod.rs` | 加 `pub mod logging;` |
| `src-tauri/src/ui/mod.rs` | 加 `pub mod logging;` |
| `src-tauri/src/lib.rs` | setup 里调 `init_logging` + `set_max_level` + `cleanup_old_logs`；Builder 加 `tauri_plugin_dialog::init()`；invoke_handler 加 `write_frontend_log`、`export_logs`。 |
| `src-tauri/src/ui/config.rs` | `save_app_config` 保存后调 `log::set_max_level`。 |
| `src-tauri/src/core/translation/service.rs` | 翻译开始/结束/取消加 `log!`。 |
| `src-tauri/src/core/translation/batch.rs` | 批次构建加 `log!`。 |
| `src-tauri/src/ui/web_popup.rs` | 翻译入口/失败加 `log!`（原文经 `redact_text` 脱敏）。 |
| `src-tauri/src/core/llm/openai_compatible.rs` | 请求开始加 `log!`（api_key 经 `redact_api_key` 脱敏）。 |
| `src-tauri/src/core/llm/claude.rs` | 同上。 |
| `src-tauri/src/core/ocr_translation.rs` | OCR 流程加 `log!`。 |
| `src-tauri/src/core/config/store.rs` | `eprintln!` 改 `log::warn!`。 |
| `src-tauri/capabilities/default.json` | 加 `dialog:allow-save`（预留前端直调；后端 command 调 dialog 不需要 capability）。 |
| `frontend/src/types/config.ts` | `AppConfig` 加 `logLevel`。 |
| `frontend/src/lib/config.ts` | `projectToAppConfig` 投影 `logLevel`。 |
| `frontend/src/lib/config.test.ts` | `base` AppConfig 加 `logLevel`。 |
| `frontend/src/lib/tauri.ts` | 加 `invokeWriteFrontendLog`、`invokeExportLogs`。 |
| `frontend/vite.config.ts` | alias 加 `@public → frontend/public`。 |
| `frontend/tsconfig.json` | `paths` 加 `@public/*`；`allowJs: true`。 |
| `frontend/src/settings/panels/AdvancedPanel.vue` | 导出按钮 `@click` 接 `invokeExportLogs` + toast；等级下拉已有 `v-model`（保存走现有 persist 链路）。 |
| `frontend/src/settings/stores/settings.ts` | `syncFromBackend` 合并 `backend.logLevel` 到 `state.advanced.logLevel`；接入 `createLogger('settings')` 打日志。 |
| `frontend/src/settings/stores/settings.test.ts` | 加 `logLevel` 同步测试。 |
| `frontend/public/translate.js` | import logger；关键流程打日志；启动 `setLevel` + 订阅 `app-config:changed` 更新。 |
| `frontend/public/overlay.html` | 内联 script import logger；框选提交/取消/IPC 错误打日志；启动 `setLevel`。 |

---

## 任务 1：Cargo.toml 依赖与构建验证

**文件：**
- 修改：`src-tauri/Cargo.toml`

- [ ] **步骤 1：编辑 Cargo.toml 加依赖**

在 `[dependencies]` 末尾（`tokio-util = "0.7"` 之后）加：

```toml
log = "0.4"
tauri-plugin-log = "2"
tauri-plugin-dialog = "2"
zip = "2"
chrono = { version = "0.4", default-features = false, features = ["clock", "std"] }
```

在 `[dev-dependencies]` 末尾加：

```toml
tempfile = "3"
```

- [ ] **步骤 2：运行 cargo build 验证依赖可解析**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED（首次会下载 tauri-plugin-log / tauri-plugin-dialog / zip / chrono / tempfile）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "chore(logging): 引入 log/tauri-plugin-log/dialog/zip/chrono 依赖"
```

---

## 任务 2：AppConfig 加 log_level 字段（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 的 `#[cfg(test)] mod tests` 末尾（`normalized_keeps_custom_shortcuts_and_empty_disabled_bindings` 之后）加：

```rust
    #[test]
    fn normalized_log_level_falls_back_to_info_for_invalid() {
        let mut config = AppConfig::from_env();
        config.log_level = "trace".to_string();
        let normalized = config.normalized();
        assert_eq!(normalized.log_level, "info");
    }

    #[test]
    fn normalized_log_level_keeps_valid_values() {
        for level in ["error", "warn", "info", "debug"] {
            let mut config = AppConfig::from_env();
            config.log_level = level.to_string();
            assert_eq!(config.normalized().log_level, level);
        }
    }

    #[test]
    fn from_env_default_log_level_is_info() {
        let config = AppConfig::from_env();
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn serializes_log_level_camel_case() {
        let config = AppConfig::from_env();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"logLevel\""), "应输出 logLevel: {json}");
    }

    #[test]
    fn deserializes_log_level_with_default() {
        let json = r#"{ "targetLang": "中文" }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少字段应可反序列化")
            .normalized();
        assert_eq!(config.log_level, "info");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib config::types::tests`
预期：FAIL，报错 `no field log_level on type AppConfig`（编译错误）。

- [ ] **步骤 3：实现 log_level 字段与归一化**

在 `AppConfig` 结构体（`collect_usage` 字段之后）加字段：

```rust
    #[serde(default = "default_log_level")]
    pub log_level: String,
```

在 `default_chain_of_thought` 函数附近加默认值函数：

```rust
fn default_log_level() -> String {
    "info".to_string()
}
```

在 `AppConfig::from_env` 的 `Self { ... }` 字面量（`collect_usage: ...` 之后）加：

```rust
            log_level: default_log_level(),
```

在 `AppConfig::normalized` 方法（`self.default_source_lang = ...` 之后）加：

```rust
        self.log_level = normalize_log_level(self.log_level);
        self
```

注意：`normalized` 原本末尾已是 `self`，把上面这行插到 `self.default_source_lang = normalize_string(...)` 之后、原 `self` 之前。

在 `normalize_chain_of_thought` 函数附近加归一化函数：

```rust
fn normalize_log_level(value: String) -> String {
    match value.trim() {
        "error" | "warn" | "info" | "debug" => value.trim().to_string(),
        _ => "info".to_string(),
    }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib config::types::tests`
预期：PASS（含新增 5 个测试 + 原有测试全过）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): AppConfig 加 log_level 字段与归一化"
```

---

## 任务 3：core/logging.rs 脱敏纯函数（TDD）

**文件：**
- 创建：`src-tauri/src/core/logging.rs`
- 修改：`src-tauri/src/core/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/core/logging.rs`，先只放测试模块：

```rust
//! 日志脱敏与等级归一化纯函数。无 Tauri 依赖，core 层可自由调用。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_api_key_keeps_first4_and_last4() {
        assert_eq!(redact_api_key("sk-abcdef12345678"), "sk-a...5678");
    }

    #[test]
    fn redact_api_key_masks_short_key_fully() {
        assert_eq!(redact_api_key("short"), "****");
        assert_eq!(redact_api_key("1234567"), "****");
    }

    #[test]
    fn redact_api_key_masks_exactly_8_chars() {
        // 等于 8 字符：前 4 + 后 4 会重叠，按短于规则全遮蔽
        assert_eq!(redact_api_key("12345678"), "****");
    }

    #[test]
    fn redact_api_key_handles_9_chars() {
        assert_eq!(redact_api_key("123456789"), "1234...6789");
    }

    #[test]
    fn redact_api_key_handles_none() {
        assert_eq!(redact_api_key(""), "****");
    }

    #[test]
    fn redact_text_info_level_returns_summary() {
        let text = "Hello, this is a long translation text.";
        let redacted = redact_text(text, "info");
        assert!(redacted.starts_with("[len=37]"));
        assert!(redacted.contains("Hello, this is a long"));
        assert!(redacted.ends_with("..."));
        assert!(!redacted.contains("translation text."));
    }

    #[test]
    fn redact_text_debug_level_returns_full() {
        let text = "Hello, this is a long translation text.";
        assert_eq!(redact_text(text, "debug"), text);
    }

    #[test]
    fn redact_text_info_short_text_includes_full_head() {
        let text = "短文本";
        let redacted = redact_text(text, "info");
        assert!(redacted.starts_with("[len=3]"));
        assert!(redacted.contains("短文本"));
    }

    #[test]
    fn redact_text_non_string_normalizes() {
        let redacted = redact_text(&42u32, "info");
        assert!(redacted.starts_with("[len=2]"));
    }
}
```

- [ ] **步骤 2：注册模块**

在 `src-tauri/src/core/mod.rs` 末尾加：

```rust
pub mod logging;
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::logging::tests`
预期：FAIL，报错 `cannot find function redact_api_key`（编译错误）。

- [ ] **步骤 4：实现脱敏函数**

在 `core/logging.rs` 顶部（`#[cfg(test)]` 之前）加实现：

```rust
//! 日志脱敏与等级归一化纯函数。无 Tauri 依赖，core 层可自由调用。

/// API Key 脱敏：前 4 + `...` + 后 4。短于 9 字符（含 8）全遮蔽 `****`。
/// 空字符串也返回 `****`。
pub fn redact_api_key(key: &str) -> String {
    let len = key.chars().count();
    if len < 9 {
        return "****".to_string();
    }
    let head: String = key.chars().take(4).collect();
    let tail: String = key.chars().skip(len - 4).collect();
    format!("{head}...{tail}")
}

/// 翻译正文脱敏：`info` 及以上记摘要（`[len=N] 前20字...`），`debug` 记原文。
/// `level` 非 `debug` 时一律按摘要处理（与 `normalize_log_level` 的回退 `info` 一致）。
pub fn redact_text(text: &dyn std::fmt::Display, level: &str) -> String {
    let full = text.to_string();
    if level == "debug" {
        return full;
    }
    let len = full.chars().count();
    let head: String = full.chars().take(20).collect();
    format!("[len={len}] {head}...")
}
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::logging::tests`
预期：PASS（9 个测试全过）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/logging.rs src-tauri/src/core/mod.rs
git commit -m "feat(logging): core 脱敏纯函数 redact_api_key/redact_text"
```

---

## 任务 4：app/logging.rs 日志目录与清理旧日志（TDD）

**文件：**
- 创建：`src-tauri/src/app/logging.rs`
- 修改：`src-tauri/src/app/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/app/logging.rs`，先只放测试模块：

```rust
//! 日志装配：目录解析、等级解析、插件初始化、旧日志清理。

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::SystemTime;
    use tempfile::tempdir;

    #[test]
    fn cleanup_removes_files_older_than_threshold() {
        let dir = tempdir().unwrap();
        let old = dir.path().join("Shizi.log.5");
        fs::write(&old, "old").unwrap();
        // 把 mtime 设为 10 天前
        let ten_days_ago = SystemTime::now() - std::time::Duration::from_secs(10 * 86400);
        let _ = filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(ten_days_ago)).ok();
        // 注意：若 filetime 不可用，下方 step4 提供不依赖 filetime 的替代实现

        let recent = dir.path().join("frontend.log");
        fs::write(&recent, "recent").unwrap();

        cleanup_old_logs(dir.path(), 7);

        assert!(!old.exists(), "旧文件应被删除");
        assert!(recent.exists(), "新文件应保留");
    }

    #[test]
    fn cleanup_ignores_non_log_files() {
        let dir = tempdir().unwrap();
        let config = dir.path().join("config.json");
        fs::write(&config, "{}").unwrap();
        let ten_days_ago = SystemTime::now() - std::time::Duration::from_secs(10 * 86400);
        let _ = filetime::set_file_mtime(&config, filetime::FileTime::from_system_time(ten_days_ago)).ok();

        cleanup_old_logs(dir.path(), 7);

        assert!(config.exists(), "非日志文件不应被清理");
    }

    #[test]
    fn cleanup_swallows_errors_on_missing_dir() {
        // 目录不存在不应 panic
        cleanup_old_logs(Path::new("/nonexistent/path/that/does/not/exist"), 7);
    }

    #[test]
    fn parse_level_filter_maps_known_levels() {
        assert_eq!(parse_level_filter("error"), log::LevelFilter::Error);
        assert_eq!(parse_level_filter("warn"), log::LevelFilter::Warn);
        assert_eq!(parse_level_filter("info"), log::LevelFilter::Info);
        assert_eq!(parse_level_filter("debug"), log::LevelFilter::Debug);
    }

    #[test]
    fn parse_level_filter_falls_back_to_info() {
        assert_eq!(parse_level_filter("trace"), log::LevelFilter::Info);
        assert_eq!(parse_level_filter(""), log::LevelFilter::Info);
        assert_eq!(parse_level_filter("garbage"), log::LevelFilter::Info);
    }
}
```

> 说明：测试用 `filetime` crate 改 mtime。若不想加依赖，改用下方步骤 4 的「等待」策略不可取（太慢）。**推荐加 `filetime` 到 dev-dependencies**：在 `Cargo.toml` `[dev-dependencies]` 加 `filetime = "0.2"`。若执行者倾向不加，可跳过 mtime 测试改用「构造一个 mtime 已旧的文件」的 OS 调用，但 Windows 上较繁。本计划采用 `filetime`。

- [ ] **步骤 2：注册模块 + 加 filetime dev-dep**

在 `src-tauri/src/app/mod.rs` 末尾加：

```rust
pub mod logging;
```

在 `src-tauri/Cargo.toml` `[dev-dependencies]` 加：

```toml
filetime = "0.2"
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::logging::tests`
预期：FAIL，报错 `cannot find function cleanup_old_logs` / `parse_level_filter`（编译错误）。

- [ ] **步骤 4：实现 cleanup_old_logs 与 parse_level_filter**

在 `app/logging.rs` 顶部（`use` 之后、`#[cfg(test)]` 之前）加实现：

```rust
//! 日志装配：目录解析、等级解析、插件初始化、旧日志清理。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tauri::Manager;

/// 日志目录：`app_config_dir()/logs/`。
pub fn logs_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("logs"))
}

/// 字符串等级 → `log::LevelFilter`，非法值回退 `Info`。
pub fn parse_level_filter(level: &str) -> log::LevelFilter {
    match level.trim() {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        _ => log::LevelFilter::Info,
    }
}

/// 启动时清理 `dir` 下 mtime 早于 `days` 天的 `*.log*` 文件。best-effort，失败静默。
pub fn cleanup_old_logs(dir: &Path, days: u64) {
    let cutoff = SystemTime::now() - Duration::from_secs(days * 86400);
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_log_file(&path) {
            continue;
        }
        let mtime = match entry.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if mtime < cutoff {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn is_log_file(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    // 匹配 Shizi.log / frontend.log / Shizi.log.1 / frontend.log.2 等
    name.ends_with(".log") || name.contains(".log.")
}
```

注意：`use std::path::{Path, PathBuf};` 在文件顶部声明了一次，步骤 1 的测试模块上方已有。实现里不要重复 `use`。最终文件顶部的 `use` 应为：

```rust
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tauri::Manager;
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib app::logging::tests`
预期：PASS（6 个测试全过）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/app/logging.rs src-tauri/src/app/mod.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(logging): 装配层日志目录解析、等级解析与旧日志清理"
```

---

## 任务 5：app/logging.rs init_logging 与 setup 接入

**文件：**
- 修改：`src-tauri/src/app/logging.rs`
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：实现 init_logging**

在 `app/logging.rs` 加 `init_logging` 函数（`cleanup_old_logs` 之后）：

```rust
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// 初始化后端日志：注册 tauri-plugin-log，写入 `app_config_dir()/logs/Shizi.log`，
/// 5MB KeepAll 轮转。注册时内部 level 设 Debug（不挡），全局 filter 由调用方
/// 用 `log::set_max_level` 设置。best-effort，失败 eprintln 兜底不阻止启动。
pub fn init_logging(app: &tauri::AppHandle, log_level: &str) {
    let dir = match logs_dir(app) {
        Some(d) => d,
        None => {
            eprintln!("日志：无法解析日志目录，跳过初始化");
            return;
        }
    };
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("日志：无法创建日志目录 {dir:?}: {error}");
        return;
    }

    let plugin = tauri_plugin_log::Builder::new()
        .level(log::LevelFilter::Debug)
        .max_file_size(5_000_000)
        .rotation_strategy(RotationStrategy::KeepAll)
        .targets(vec![Target::new(TargetKind::Folder(dir.clone()))])
        .build();

    if let Err(error) = app.plugin(plugin) {
        eprintln!("日志：注册 tauri-plugin-log 失败: {error}");
        return;
    }

    // 插件注册时会把全局 max_level 设为 Debug，这里覆盖为配置值。
    log::set_max_level(parse_level_filter(log_level));
}
```

- [ ] **步骤 2：在 lib.rs setup 里接入**

修改 `src-tauri/src/lib.rs`。在 `use` 块加 `app::logging;`（与 `app::shortcuts` 等并列，可加到 `use app::{...}` 块内）：

```rust
use app::{
    logging,
    popup_window::ensure_popup_window,
    shortcuts::{handle_global_shortcut, register_global_shortcuts_at_startup},
    state::AppState,
    tray::setup_tray,
    window::{ensure_settings_window, setup_close_to_hide},
};
```

在 `.setup(|app| { ... })` 闭包内，`ConfigStore::load` 与 `AppState::new` 之后、`register_global_shortcuts_at_startup` 之前，插入日志初始化。定位锚点：在 `app.manage(AppState::new(config_store));` 之后：

```rust
            // 日志初始化（best-effort，不阻止启动）
            let log_level = app
                .state::<AppState>()
                .config_store
                .get()
                .map(|c| c.log_level)
                .unwrap_or_else(|_| "info".to_string());
            logging::init_logging(app.handle(), &log_level);
            if let Some(dir) = logging::logs_dir(app.handle()) {
                logging::cleanup_old_logs(&dir, 7);
            }
            log::info!("应用启动，日志等级: {}", log_level);
```

- [ ] **步骤 3：运行 cargo build 验证**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED。若 `app.plugin(plugin)` 报方法不存在，回查 Tauri 版本（应 2.11.3，`AppHandle::plugin` 在 app.rs:527）。

- [ ] **步骤 4：运行全部测试确认无回归**

运行：`cd src-tauri && cargo test`
预期：PASS（所有现有测试 + 新增测试）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/logging.rs src-tauri/src/lib.rs
git commit -m "feat(logging): 装配层 init_logging 并在 setup 初始化后端日志"
```

---

## 任务 6：ui/logging.rs write_frontend_log command（TDD）

**文件：**
- 创建：`src-tauri/src/ui/logging.rs`
- 修改：`src-tauri/src/ui/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/ui/logging.rs`，先放测试模块与待测函数签名桩（桩返回 `unimplemented!()`）：

```rust
//! 前端日志 command：直接 std::fs append 写 frontend.log，不走 log facade，
//! 与后端 Shizi.log 物理隔离。

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::app::logging::logs_dir;
use crate::app::state::AppState;
use crate::core::logging::redact_api_key;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendLogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
    pub source: String,
    #[serde(default)]
    pub meta: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn append_frontend_log_writes_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        append_frontend_log(&path, "hello", 5_000_000).unwrap();
        append_frontend_log(&path, "world", 5_000_000).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "hello\nworld\n");
    }

    #[test]
    fn append_rotates_when_exceeding_max_size() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        // 用小 max_size 快速验证：95 字节 + 换行 = 96，再写 "overflow"(8+1=9) → 96+9=105 > 100 触发轮转
        let big = "x".repeat(95);
        append_frontend_log(&path, &big, 100).unwrap();
        append_frontend_log(&path, "overflow", 100).unwrap();
        assert!(rotated_path(&path, 1).exists(), "应产生 frontend.log.1");
        let current = fs::read_to_string(&path).unwrap();
        assert_eq!(current, "overflow\n");
        let rotated = fs::read_to_string(rotated_path(&path, 1)).unwrap();
        assert!(rotated.contains(&big));
    }

    #[test]
    fn append_rotation_shifts_existing_backups() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("frontend.log");
        // 预置 .1
        fs::write(rotated_path(&path, 1), "old1").unwrap();
        let big = "x".repeat(95);
        append_frontend_log(&path, &big, 100).unwrap();
        append_frontend_log(&path, "overflow", 100).unwrap();
        // 原 .1 应被推到 .2
        assert!(rotated_path(&path, 2).exists());
        assert_eq!(fs::read_to_string(rotated_path(&path, 2)).unwrap(), "old1");
    }

    #[test]
    fn should_log_filters_by_level() {
        assert!(should_log("error", log::LevelFilter::Error));
        assert!(should_log("error", log::LevelFilter::Info));
        assert!(!should_log("debug", log::LevelFilter::Info));
        assert!(!should_log("info", log::LevelFilter::Warn));
        assert!(!should_log("unknown", log::LevelFilter::Debug));
    }
}
```

- [ ] **步骤 2：注册模块**

在 `src-tauri/src/ui/mod.rs` 加（按字母序排在 `ocr_popup` 前）：

```rust
pub mod logging;
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib ui::logging::tests`
预期：FAIL，报错 `cannot find function append_frontend_log` 等（编译错误，因函数未实现）。

- [ ] **步骤 4：实现 append/rotate/should_log 与 write_frontend_log**

在 `ui/logging.rs` 的 `FrontendLogEntry` 定义之后、`#[cfg(test)]` 之前加实现：

```rust
const FRONTEND_LOG_MAX_SIZE: u64 = 5_000_000;

/// 追加一行到 frontend.log，超 5MB 时轮转。best-effort：IO 失败返回 Err。
pub fn append_frontend_log(path: &Path, line: &str, max_size: u64) -> std::io::Result<()> {
    if path.exists() {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() + line.len() as u64 + 1 > max_size {
                rotate_logs(path);
            }
        }
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn rotate_logs(path: &Path) {
    // 找当前最大编号
    let mut max_n = 0;
    let mut n = 1;
    loop {
        if rotated_path(path, n).exists() {
            max_n = n;
            n += 1;
        } else {
            break;
        }
    }
    // 从高到低重命名：.max_n → .max_n+1 ... .1 → .2
    for i in (1..=max_n).rev() {
        let from = rotated_path(path, i);
        let to = rotated_path(path, i + 1);
        let _ = std::fs::rename(&from, &to);
    }
    // 当前文件 → .1
    let _ = std::fs::rename(path, rotated_path(path, 1));
}

fn rotated_path(path: &Path, n: u32) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), n))
}

/// 按 config 当前 level 过滤前端 entry（双保险，前端已过滤）。
pub fn should_log(entry_level: &str, filter: log::LevelFilter) -> bool {
    let level = match entry_level {
        "error" => log::Level::Error,
        "warn" => log::Level::Warn,
        "info" => log::Level::Info,
        "debug" => log::Level::Debug,
        _ => return false,
    };
    level <= filter
}

#[tauri::command]
pub async fn write_frontend_log(
    entries: Vec<FrontendLogEntry>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let filter = crate::app::logging::parse_level_filter(&config.log_level);
    let dir = logs_dir(&app).ok_or_else(|| "无法解析日志目录".to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("frontend.log");
    for entry in entries {
        if !should_log(&entry.level, filter) {
            continue;
        }
        let line = format!(
            "{} [{}] {} {}",
            entry.timestamp,
            entry.level.to_uppercase(),
            entry.source,
            entry.message
        );
        // best-effort：单条失败跳过，不崩。
        let _ = append_frontend_log(&path, &line, FRONTEND_LOG_MAX_SIZE);
    }
    Ok(())
}
```

注意：文件顶部 `use` 已有 `redact_api_key`，本任务暂未用到（export_logs 用），保留以避免步骤 7 再改 use。若编译告警 unused import，可在步骤 7 前临时移除，步骤 7 加回。**为保持计划原子性，本任务移除 `redact_api_key` 的 use，步骤 7 再加。** 即本任务文件顶部 `use` 为：

```rust
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::app::logging::logs_dir;
use crate::app::state::AppState;
```

- [ ] **步骤 5：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib ui::logging::tests`
预期：PASS（4 个测试全过）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/ui/logging.rs src-tauri/src/ui/mod.rs
git commit -m "feat(logging): write_frontend_log command 直接追加写 frontend.log"
```

---

## 任务 7：ui/logging.rs export_logs command（TDD）

**文件：**
- 修改：`src-tauri/src/ui/logging.rs`

- [ ] **步骤 1：编写失败的测试**

在 `ui/logging.rs` 的 `#[cfg(test)] mod tests` 末尾加：

```rust
    #[test]
    fn config_snapshot_redacts_api_keys() {
        use crate::core::config::AppConfig;
        let mut config = AppConfig::from_env();
        config.services[0].api_key = Some("sk-abcdef12345678".to_string());
        let json = config_snapshot_json(&config);
        assert!(json.contains("sk-a...5678"), "apiKey 应脱敏: {json}");
        assert!(!json.contains("abcdef12345678"), "原始 key 不应出现: {json}");
    }

    #[test]
    fn config_snapshot_preserves_other_fields() {
        use crate::core::config::AppConfig;
        let config = AppConfig::from_env();
        let json = config_snapshot_json(&config);
        assert!(json.contains("\"targetLang\""));
        assert!(json.contains("\"logLevel\""));
    }

    #[test]
    fn system_info_includes_version_os_and_level() {
        use crate::core::config::AppConfig;
        let config = AppConfig::from_env();
        let info = system_info(&config);
        assert!(info.contains("Version:"));
        assert!(info.contains("OS:"));
        assert!(info.contains("Log level:"));
        assert!(info.contains(&config.log_level));
    }

    #[test]
    fn export_zip_bundles_log_files_and_snapshot() {
        use crate::core::config::AppConfig;
        use std::io::{Read, Seek, SeekFrom};
        use zip::ZipArchive;

        let dir = tempdir().unwrap();
        // 造两个日志文件
        fs::write(dir.path().join("Shizi.log"), "backend line\n").unwrap();
        fs::write(dir.path().join("frontend.log"), "frontend line\n").unwrap();
        fs::write(dir.path().join("config.json"), "{}").unwrap(); // 非日志，应被忽略

        let zip_path = dir.path().join("export.zip");
        let config = AppConfig::from_env();
        write_export_zip(&zip_path, dir.path(), &config).unwrap();

        let mut archive = ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "Shizi.log"), "{names:?}");
        assert!(names.iter().any(|n| n == "frontend.log"), "{names:?}");
        assert!(names.iter().any(|n| n == "config-snapshot.json"), "{names:?}");
        assert!(names.iter().any(|n| n == "system-info.txt"), "{names:?}");
        assert!(!names.iter().any(|n| n == "config.json"), "非日志文件不应入包");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib ui::logging::tests`
预期：FAIL，报错 `cannot find function config_snapshot_json` / `system_info` / `write_export_zip`。

- [ ] **步骤 3：实现 config_snapshot_json / system_info / write_export_zip / export_logs**

文件顶部 `use` 块补 `redact_api_key` 与 zip 相关：

```rust
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::app::logging::logs_dir;
use crate::app::state::AppState;
use crate::core::config::AppConfig;
use crate::core::logging::redact_api_key;
```

在 `write_frontend_log` 之后加：

```rust
/// 把 AppConfig 序列化为 JSON，每个 service 的 apiKey 用 redact_api_key 脱敏。
pub fn config_snapshot_json(config: &AppConfig) -> String {
    let mut value = match serde_json::to_value(config) {
        Ok(v) => v,
        Err(_) => return "{}".to_string(),
    };
    if let Some(services) = value.get_mut("services").and_then(|s| s.as_array_mut()) {
        for svc in services {
            if let Some(key) = svc.get("apiKey").and_then(|k| k.as_str()) {
                svc["apiKey"] = serde_json::Value::String(redact_api_key(key));
            }
        }
    }
    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// 生成 system-info.txt 内容。
pub fn system_info(config: &AppConfig) -> String {
    let now = chrono::Local::now().to_rfc3339();
    format!(
        "Shizi log export\\nVersion: {}\\nOS: {}\\nExport time: {}\\nLog level: {}\\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        now,
        config.log_level
    )
}

/// 把 `log_dir` 下所有 `*.log*` + config snapshot + system info 打包到 `zip_path`。
pub fn write_export_zip(
    zip_path: &Path,
    log_dir: &Path,
    config: &AppConfig,
) -> std::io::Result<()> {
    let file = std::fs::File::create(zip_path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default();

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !(name.ends_with(".log") || name.contains(".log.")) {
                continue;
            }
            zip.start_file(&name, opts)?;
            let bytes = std::fs::read(&path)?;
            zip.write_all(&bytes)?;
        }
    }

    zip.start_file("config-snapshot.json", opts)?;
    zip.write_all(config_snapshot_json(config).as_bytes())?;

    zip.start_file("system-info.txt", opts)?;
    zip.write_all(system_info(config).as_bytes())?;

    zip.finish()?;
    Ok(())
}

#[tauri::command]
pub async fn export_logs(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let save_path = app
        .dialog()
        .file()
        .add_filter("ZIP", &["zip"])
        .set_file_name(format!("shizi-logs-{}.zip", chrono::Local::now().format("%Y%m%d-%H%M%S")))
        .save_file()
        .await
        .ok_or_else(|| "用户取消导出".to_string())?
        .into_path()
        .map_err(|_| "无效的保存路径".to_string())?;

    let config = state.config_store.get().map_err(|e| e.to_string())?;
    let dir = logs_dir(&app).ok_or_else(|| "无法解析日志目录".to_string())?;

    write_export_zip(&save_path, &dir, &config).map_err(|e| e.to_string())?;

    Ok(save_path.to_string_lossy().to_string())
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib ui::logging::tests`
预期：PASS（8 个测试全过）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/ui/logging.rs
git commit -m "feat(logging): export_logs command 打包 zip 含日志/配置快照/系统信息"
```

---

## 任务 8：save_app_config 运行时切换日志等级

**文件：**
- 修改：`src-tauri/src/ui/config.rs`

- [ ] **步骤 1：在 save_app_config 里加 set_max_level**

`save_app_config` 当前在 `config.rs:21-47`。在 `let saved_config = state.config_store.save(config)...` 之后、`app.emit("app-config:changed", &saved_config)` 之前，插入：

```rust
    // 运行时即时切换日志等级（tauri-plugin-log 注册时内部 level 为 Debug 不挡，
    // 全局 set_max_level 独占控制权，无需重启插件）。
    log::set_max_level(crate::app::logging::parse_level_filter(&saved_config.log_level));
```

修改后 `save_app_config` 完整形如：

```rust
#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, ShortcutBindingError> {
    let old_config = state
        .config_store
        .get()
        .map_err(|error| ShortcutBindingError::global(format!("无法读取旧配置: {error}")))?;
    let config = config.normalized();

    replace_global_shortcuts(&app, &old_config, &config)?;

    let saved_config = state
        .config_store
        .save(config)
        .map_err(|error| ShortcutBindingError::global(format!("无法保存配置: {error}")))?;

    log::set_max_level(crate::app::logging::parse_level_filter(&saved_config.log_level));

    let _ = state.set_shortcut_conflicts(Vec::new());

    app.emit("app-config:changed", &saved_config)
        .map_err(|error| ShortcutBindingError::global(format!("无法广播配置变更: {error}")))?;

    Ok(saved_config)
}
```

- [ ] **步骤 2：运行 cargo build 验证**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED。

- [ ] **步骤 3：运行全部测试确认无回归**

运行：`cd src-tauri && cargo test`
预期：PASS。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/config.rs
git commit -m "feat(logging): save_app_config 保存后即时切换全局日志等级"
```

---

## 任务 9：lib.rs 注册 command + dialog 插件 + capabilities

**文件：**
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri/capabilities/default.json`

- [ ] **步骤 1：lib.rs 注册 dialog 插件与新 command**

在 `lib.rs` 的 `use ui::{...}` 块加 `logging::{export_logs, write_frontend_log}`：

```rust
use ui::{
    config::{get_app_config, get_shortcut_conflicts, open_settings, save_app_config},
    logging::{export_logs, write_frontend_log},
    ocr_popup::trigger_ocr_translation,
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    service_probe::{list_service_models, validate_service_credential},
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};
```

在 `tauri::Builder::default()` 链的 `.plugin(tauri_plugin_global_shortcut::...)` 之后加 dialog 插件：

```rust
        .plugin(tauri_plugin_dialog::init())
```

在 `.invoke_handler(tauri::generate_handler![...])` 的命令列表末尾（`show_overlay` 之后）加：

```rust
            write_frontend_log,
            export_logs,
```

- [ ] **步骤 2：capabilities 加 dialog:allow-save**

`src-tauri/capabilities/default.json` 的 `permissions` 数组加 `"dialog:allow-save"`：

```json
{
  "identifier": "default",
  "description": "Default capabilities",
  "windows": ["main", "settings", "screenshot-overlay"],
  "permissions": [
    "core:default",
    "core:window:allow-set-always-on-top",
    "core:window:allow-set-size",
    "core:window:allow-hide",
    "global-shortcut:default",
    "dialog:allow-save"
  ]
}
```

> 说明：`export_logs` 在后端 command 内调 `app.dialog()`，不受前端 capability 限制；此条为预留前端直调 save 对话框，无害。

- [ ] **步骤 3：运行 cargo build 验证**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/capabilities/default.json
git commit -m "feat(logging): 注册 dialog 插件与 write_frontend_log/export_logs command"
```

---

## 任务 10：后端翻译流程接入日志

**文件：**
- 修改：`src-tauri/src/core/translation/service.rs`
- 修改：`src-tauri/src/core/translation/batch.rs`
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：service.rs 翻译开始/结束/取消加日志**

在 `core/translation/service.rs` 的 `translate_with` 方法开头（`let full_text = ...` 之前）加：

```rust
        log::info!(
            "翻译开始: service={} protocol={} session={}",
            request.service.service_name,
            request.service.protocol,
            request.session_id.0
        );
```

在 `if cancel.is_cancelled() { ... }` 分支的 `emit(TranslationEvent::Cancelled ...)` 之前加：

```rust
            log::warn!(
                "翻译取消: service={} session={}",
            request.service.service_name,
            request.session_id.0
            );
```

在 `else { ... emit(TranslationEvent::Finished ...) }` 分支的 emit 之前加：

```rust
            log::info!(
                "翻译完成: service={} session={} len={}",
                request.service.service_name,
                request.session_id.0,
                full_text.chars().count()
            );
```

- [ ] **步骤 2：batch.rs 批次构建加日志**

在 `core/translation/batch.rs` 的 `build_batch_requests` 返回 `Ok(requests)` 之前加：

```rust
    log::info!(
        "构建翻译批次: batch_id={} services={}",
        batch_id,
        requests.iter().map(|r| r.service.service_instance_id.as_str()).collect::<Vec<_>>().join(",")
    );
```

- [ ] **步骤 3：web_popup.rs 翻译入口与失败加日志（原文脱敏）**

在 `ui/web_popup.rs` 顶部 `use crate::{...}` 块的 `core::{...}` 内加 `logging::redact_text`：

```rust
use crate::{
    app::{popup_window, state::AppState},
    core::{
        config::{AppConfig, ServiceInstanceConfig},
        llm::provider_for_service,
        logging::redact_text,
        translation::{
            batch, TranslationEvent, TranslationInput, TranslationService,
            TranslationServiceMeta, TranslationSessionId,
        },
    },
};
```

在 `start_translation_from_input` 方法，`let requests = batch::build_batch_requests(...)?;` 之后加（用配置的 log_level 决定脱敏程度）：

```rust
    let log_level = config.log_level.clone();
    log::info!(
        "翻译入口: source_type={} {}",
        input.kind(),
        redact_text(&input.text(), &log_level)
    );
```

在 spawn 任务里，`match provider_for_service(&service_config)` 的 `Err(message) => { ... emit Failed ... }` 分支里，emit 之前加：

```rust
                        log::error!(
                            "provider 初始化失败: service={} err={}",
                            failed_service.service_name,
                            message
                        );
```

在 `if let Err(error) = result { ... emit Failed ... }` 分支里，emit 之前加：

```rust
                        log::error!(
                            "翻译失败: service={} session={} retryable={} err={}",
                            failed_service.service_name,
                            failed_session_id.0,
                            error.retryable(),
                            error
                        );
```

- [ ] **步骤 4：运行 cargo build 验证**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED。

- [ ] **步骤 5：运行全部测试确认无回归**

运行：`cd src-tauri && cargo test`
预期：PASS（日志为 side effect，不影响测试断言）。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/core/translation/service.rs src-tauri/src/core/translation/batch.rs src-tauri/src/ui/web_popup.rs
git commit -m "feat(logging): 翻译流程关键节点接入日志（原文脱敏）"
```

---

## 任务 11：后端 LLM / OCR / config 接入日志

**文件：**
- 修改：`src-tauri/src/core/llm/openai_compatible.rs`
- 修改：`src-tauri/src/core/llm/claude.rs`
- 修改：`src-tauri/src/core/ocr_translation.rs`
- 修改：`src-tauri/src/core/config/store.rs`

- [ ] **步骤 1：openai_compatible.rs 请求开始加日志（api_key 脱敏）**

在 `core/llm/openai_compatible.rs` 的 `stream_translate` 方法，`let api_key = self.config.api_key.as_deref().ok_or(...)?;` 之后加：

```rust
        log::info!(
            "OpenAI 请求: endpoint={} model={} key={}",
            self.endpoint(),
            self.config.model,
            crate::core::logging::redact_api_key(api_key)
        );
```

在 `parse_error_response` 返回前（`if retryable { ... } else { ... }` 之前）加：

```rust
        log::warn!("OpenAI 响应非 2xx: status={} retryable={}", status, retryable);
```

- [ ] **步骤 2：claude.rs 请求开始加日志（api_key 脱敏）**

先读 `src-tauri/src/core/llm/claude.rs` 找到 `stream_translate` 里使用 api_key 的位置。在该 `let api_key = ...` 之后加同样模式的日志（endpoint 用 `self.config` 的 base_url，model 用 `self.config.model`）：

```rust
        log::info!(
            "Claude 请求: endpoint={} model={} key={}",
            <调用 claude 的 endpoint 构造方法或 self.config.base_url>,
            self.config.model,
            crate::core::logging::redact_api_key(api_key)
        );
```

> 执行者需先读 `claude.rs` 确认字段名（`ClaudeConfig` 的 `base_url`/`model`/`api_key`）与 endpoint 构造方式，照搬 `openai_compatible.rs` 的 `self.endpoint()` 模式。若 claude.rs 无独立 `endpoint()` 方法，用 `format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'))` 或该文件已有的等价构造。

- [ ] **步骤 3：ocr_translation.rs OCR 流程加日志**

先读 `src-tauri/src/core/ocr_translation.rs`，在以下关键节点加日志（函数名以实际为准）：
- 抓帧成功后：`log::info!("OCR 抓帧: {}x{} scale={}", width, height, scale);`
- 识别完成：`log::info!("OCR 识别: 文本长度={}", text.chars().count());`
- 进入翻译链路：`log::info!("OCR 翻译入口: {}", redact_text(&recognized_text, &log_level));`（若拿不到 log_level，用 `redact_text(&text, "info")` 固定摘要）
- 任何错误分支：`log::error!("OCR 失败: {error}");`

执行者读文件后按实际函数签名插入，每个节点一行 `log!`，不改动业务逻辑。

- [ ] **步骤 4：config/store.rs eprintln 改 log::warn**

`core/config/store.rs:56` 当前：

```rust
                    eprintln!("配置文件解析失败，使用默认配置：{err}");
```

改为：

```rust
                    log::warn!("配置文件解析失败，使用默认配置：{err}");
```

> 注意：`ConfigStore::load` 在 `lib.rs` setup 里早于 `init_logging` 调用。此时 logger 未注册，`log::warn!` 会落到 `log` crate 的默认空 logger（无输出）。这符合 best-effort：启动早期配置解析失败本就罕见，且 `init_logging` 之后的所有日志正常落盘。若需启动早期也可见，保留 `eprintln!` 兜底也可——本计划选择 `log::warn!` 以统一门面，接受启动早期不可见。

- [ ] **步骤 5：运行 cargo build 验证**

运行：`cd src-tauri && cargo build`
预期：BUILD SUCCEEDED。

- [ ] **步骤 6：运行全部测试确认无回归**

运行：`cd src-tauri && cargo test`
预期：PASS。

- [ ] **步骤 7：Commit**

```bash
git add src-tauri/src/core/llm/openai_compatible.rs src-tauri/src/core/llm/claude.rs src-tauri/src/core/ocr_translation.rs src-tauri/src/core/config/store.rs
git commit -m "feat(logging): LLM/OCR/config 接入日志（api_key 脱敏）"
```

---

## 任务 12：前端 logger.js 与单测（TDD）

**文件：**
- 创建：`frontend/public/logger.js`
- 创建：`frontend/src/lib/logger.test.ts`

- [ ] **步骤 1：编写失败的测试**

创建 `frontend/src/lib/logger.test.ts`：

```ts
import { describe, it, expect, vi } from 'vitest'
import { createLogger, redactText, clampBuffer } from '../../public/logger.js'

const makeDeps = () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
  now: () => '2026-07-08T00:00:00.000Z',
  addEventListener: vi.fn(),
  setTimeout: vi.fn(() => 'timer-id') as unknown as (fn: () => void, ms: number) => unknown,
  clearTimeout: vi.fn(),
})

describe('clampBuffer', () => {
  it('超限丢弃最旧', () => {
    const buf = Array.from({ length: 1005 }, (_, i) => ({ msg: `msg-${i}` }))
    clampBuffer(buf, 1000)
    expect(buf).toHaveLength(1000)
    expect(buf[0].msg).toBe('msg-5')
    expect(buf[999].msg).toBe('msg-1004')
  })

  it('未超限不变', () => {
    const buf = [{ msg: 'a' }, { msg: 'b' }]
    clampBuffer(buf, 1000)
    expect(buf).toHaveLength(2)
  })
})

describe('createLogger', () => {
  it('按 level 过滤：info 下 debug 不入队', async () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('info')
    logger.debug('dropped')
    logger.info('kept')
    expect(deps.invoke).not.toHaveBeenCalled()
    await logger.flush()
    expect(deps.invoke).toHaveBeenCalledTimes(1)
    const entries = (deps.invoke.mock.calls[0][1] as { entries: Array<{ message: string }> }).entries
    expect(entries).toHaveLength(1)
    expect(entries[0].message).toBe('kept')
  })

  it('debug 等级下 debug 入队', async () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('debug')
    logger.debug('dbg')
    await logger.flush()
    const entries = (deps.invoke.mock.calls[0][1] as { entries: Array<{ message: string }> }).entries
    expect(entries[0].message).toBe('dbg')
  })

  it('满 50 条立即 flush', () => {
    const deps = makeDeps()
    const logger = createLogger('test', deps)
    logger.setLevel('error')
    for (let i = 0; i < 50; i++) logger.error(`m-${i}`)
    expect(deps.invoke).toHaveBeenCalledTimes(1)
  })

  it('invoke 失败重试一次后丢弃', async () => {
    const deps = makeDeps()
    deps.invoke = vi.fn().mockRejectedValue(new Error('boom'))
    const logger = createLogger('test', deps)
    logger.setLevel('error')
    logger.error('x')
    await logger.flush()
    expect(deps.invoke).toHaveBeenCalledTimes(2)
  })

  it('redactText info 摘要、debug 全文', () => {
    const text = 'Hello, this is a long translation text.'
    expect(redactText(text, 'info')).toContain('[len=37]')
    expect(redactText(text, 'info')).not.toContain('translation text.')
    expect(redactText(text, 'debug')).toBe(text)
  })

  it('addEventListener 注册 visibilitychange 与 beforeunload', () => {
    const deps = makeDeps()
    createLogger('test', deps)
    const types = deps.addEventListener.mock.calls.map((c) => c[0])
    expect(types).toContain('visibilitychange')
    expect(types).toContain('beforeunload')
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test`
预期：FAIL，报错 `Failed to resolve import "../../public/logger.js"`（文件不存在）。

- [ ] **步骤 3：实现 logger.js**

创建 `frontend/public/logger.js`：

```js
// 纯 ES module，三页共用。照 translate-card-sync.js 先例：无依赖。
// 测试与 settings 页通过注入 deps 或 @public alias 引入。

const LEVELS = { error: 0, warn: 1, info: 2, debug: 3 }
const BUFFER_LIMIT = 1000
const FLUSH_COUNT = 50
const FLUSH_INTERVAL_MS = 2000

function defaultDeps() {
  const tauri = (typeof window !== 'undefined' && window.__TAURI__) || {}
  return {
    invoke: tauri?.core?.invoke,
    now: () => new Date().toISOString(),
    addEventListener: typeof window !== 'undefined' ? window.addEventListener.bind(window) : undefined,
    setTimeout: typeof window !== 'undefined' ? window.setTimeout.bind(window) : undefined,
    clearTimeout: typeof window !== 'undefined' ? window.clearTimeout.bind(window) : undefined,
    visibilityState: typeof document !== 'undefined' ? () => document.visibilityState : () => 'visible',
  }
}

export function redactText(text, level) {
  const full = typeof text === 'string' ? text : String(text ?? '')
  if (level === 'debug') return full
  const len = full.length
  const head = full.slice(0, 20)
  return `[len=${len}] ${head}...`
}

/** 截断缓冲：超 limit 丢弃最旧（FIFO）。导出供单测。 */
export function clampBuffer(buffer, limit) {
  if (buffer.length > limit) buffer.splice(0, buffer.length - limit)
}

export function createLogger(source, deps) {
  const d = { ...defaultDeps(), ...(deps || {}) }
  let level = 'info'
  const buffer = []
  let flushTimer = null
  let flushing = false

  function shouldLog(msgLevel) {
    return (LEVELS[msgLevel] ?? 2) <= (LEVELS[level] ?? 2)
  }

  function enqueue(entry) {
    buffer.push(entry)
    clampBuffer(buffer, BUFFER_LIMIT)
    if (buffer.length >= FLUSH_COUNT) {
      flush()
    } else if (!flushTimer && d.setTimeout) {
      flushTimer = d.setTimeout(flush, FLUSH_INTERVAL_MS)
    }
  }

  // 成功才移除已提交条目（splice 在 then 里）；失败重试一次，仍失败丢弃该批，
  // buffer 保留剩余条目继续累积。flushing 锁防止并发 flush 重复提交。
  function flush() {
    if (flushTimer && d.clearTimeout) { d.clearTimeout(flushTimer); flushTimer = null }
    if (buffer.length === 0 || !d.invoke || flushing) return Promise.resolve()
    flushing = true
    const batch = buffer.slice(0, FLUSH_COUNT)
    return Promise.resolve(d.invoke('write_frontend_log', { entries: batch }))
      .then(() => { buffer.splice(0, batch.length); flushing = false })
      .catch(() => Promise.resolve(d.invoke('write_frontend_log', { entries: batch }))
        .then(() => { buffer.splice(0, batch.length); flushing = false })
        .catch(() => { flushing = false }))
  }

  function log(msgLevel, message, meta) {
    if (!shouldLog(msgLevel)) return
    enqueue({
      level: msgLevel,
      message: typeof message === 'string' ? message : String(message),
      timestamp: d.now(),
      source,
      meta: meta ?? undefined,
    })
  }

  if (d.addEventListener) {
    d.addEventListener('visibilitychange', () => {
      if (d.visibilityState && d.visibilityState() === 'hidden') flush()
    })
    d.addEventListener('beforeunload', flush)
  }

  return {
    get level() { return level },
    setLevel(newLevel) { level = newLevel },
    debug: (msg, meta) => log('debug', msg, meta),
    info: (msg, meta) => log('info', msg, meta),
    warn: (msg, meta) => log('warn', msg, meta),
    error: (msg, meta) => log('error', msg, meta),
    redactText: (text) => redactText(text, level),
    flush,
  }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`npm run test`
预期：PASS（7 个测试全过）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/public/logger.js frontend/src/lib/logger.test.ts
git commit -m "feat(logging): 前端 logger.js 纯 ES module 与单测"
```

---

## 任务 13：前端类型 / 配置投影 / tauri 桥 / alias（TDD）

**文件：**
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/lib/tauri.ts`
- 修改：`frontend/vite.config.ts`
- 修改：`frontend/tsconfig.json`

- [ ] **步骤 1：types/config.ts 加 logLevel**

在 `frontend/src/types/config.ts` 的 `AppConfig` 接口（`collectUsage: boolean;` 之后）加：

```ts
  logLevel: LogLevel;
```

并在文件顶部 `ServiceProtocolId` 之前加类型：

```ts
export type LogLevel = 'error' | 'warn' | 'info' | 'debug';
```

- [ ] **步骤 2：config.ts projectToAppConfig 投影 logLevel**

在 `frontend/src/lib/config.ts` 的 `projectToAppConfig` 返回对象（`collectUsage: state.advanced.collectUsage,` 之后）加：

```ts
    logLevel: state.advanced.logLevel,
```

- [ ] **步骤 3：config.test.ts base AppConfig 加 logLevel**

`frontend/src/lib/config.test.ts` 的 `validateConfig` describe 里有 `const base: AppConfig = { ... }`，加 `logLevel: 'info',`（与 `collectUsage: true,` 同级）：

```ts
  const base: AppConfig = {
    targetLang: '中文',
    defaultSourceLang: 'auto',
    autoCopy: true,
    restoreClipboard: true,
    services: [],
    popupPrecreate: true,
    overlayPrecreate: true,
    collectUsage: true,
    logLevel: 'info',
    shortcuts: {},
  };
```

并在 `describe('projectToAppConfig')` 的第一个 it 里补断言：

```ts
    expect(config.logLevel).toBe('info');
```

- [ ] **步骤 4：tauri.ts 加 invokeWriteFrontendLog / invokeExportLogs**

在 `frontend/src/lib/tauri.ts` 末尾加：

```ts
export interface FrontendLogEntry {
  level: 'error' | 'warn' | 'info' | 'debug';
  message: string;
  timestamp: string;
  source: string;
  meta?: unknown;
}

export async function invokeWriteFrontendLog(entries: FrontendLogEntry[]): Promise<void> {
  return requireInvoke()<void>('write_frontend_log', { entries });
}

export async function invokeExportLogs(): Promise<string> {
  return requireInvoke()<string>('export_logs');
}
```

- [ ] **步骤 5：vite.config.ts 加 @public alias**

在 `frontend/vite.config.ts` 的 `resolve.alias` 加：

```ts
  resolve: {
    alias: {
      '@': resolve(frontendDir, 'src'),
      '@public': resolve(frontendDir, 'public'),
    },
  },
```

- [ ] **步骤 6：tsconfig.json 加 paths 与 allowJs**

`frontend/tsconfig.json` 的 `compilerOptions` 加 `"allowJs": true`，`paths` 加 `"@public/*": ["./public/*"]`：

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "jsx": "preserve",
    "esModuleInterop": true,
    "skipLibCheck": true,
    "noEmit": true,
    "allowJs": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "types": ["vite/client"],
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"],
      "@public/*": ["./public/*"]
    }
  },
  "include": ["src/**/*.ts", "src/**/*.vue", "settings.html"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

- [ ] **步骤 7：运行 typecheck + test 验证**

运行：`npm run typecheck`
预期：PASS（allowJs 让 `@public/logger.js` 与 `../../public/logger.js` 的 JS import 可解析）。

运行：`npm run test`
预期：PASS（含 logger.test.ts + config.test.ts）。

- [ ] **步骤 8：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/lib/tauri.ts frontend/vite.config.ts frontend/tsconfig.json
git commit -m "feat(logging): 前端 logLevel 类型/投影/桥与 @public alias"
```

---

## 任务 14：AdvancedPanel 导出按钮与等级保存

**文件：**
- 修改：`frontend/src/settings/panels/AdvancedPanel.vue`

- [ ] **步骤 1：接导出按钮 + toast**

在 `AdvancedPanel.vue` 的 `<script setup>` 加导入与处理函数。先在顶部 import 区加：

```ts
import { invokeExportLogs } from '@/lib/tauri'
import { toast } from '@/lib/toast'
```

在 `const buildChannel = 'dev'` 之后加：

```ts
const exporting = ref(false)

async function handleExportLogs() {
  if (exporting.value) return
  exporting.value = true
  try {
    const path = await invokeExportLogs()
    toast.success('日志已导出', path)
  } catch (e) {
    const msg = String(e)
    if (msg.includes('取消')) {
      // 用户取消，不提示错误
    } else {
      toast.error('导出失败', msg)
    }
  } finally {
    exporting.value = false
  }
}
```

- [ ] **步骤 2：模板绑定 @click**

模板里「导出日志」的 `<Button>` 加 `@click="handleExportLogs"` 与 `:disabled="exporting"`：

```vue
      <Button variant="outline" size="sm" :disabled="exporting" @click="handleExportLogs">
        <Download class="h-3.5 w-3.5" />
        导出
      </Button>
```

> 等级下拉 `v-model="state.advanced.logLevel"` 已存在，等级保存走现有 `useSettings` 的自动 persist 链路（`watch(state, ...)` → `invokeSaveAppConfig`），后端 `save_app_config` 已在任务 8 接 `set_max_level`，无需额外代码。

- [ ] **步骤 3：运行 typecheck 验证**

运行：`npm run typecheck`
预期：PASS。

- [ ] **步骤 4：Commit**

```bash
git add frontend/src/settings/panels/AdvancedPanel.vue
git commit -m "feat(logging): AdvancedPanel 导出按钮接入 export_logs"
```

---

## 任务 15：settings store 同步 logLevel（TDD）

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`

- [ ] **步骤 1：编写失败的测试**

先读 `frontend/src/settings/stores/settings.test.ts` 了解现有测试风格。在该文件末尾加（导入 `mergeBackendIntoServices` 等已导出的辅助函数按现有风格）：

```ts
import { describe, it, expect } from 'vitest'
import { mergeBackendIntoServices } from './settings'

describe('syncFromBackend logLevel', () => {
  it('后端 logLevel 覆盖前端 advanced.logLevel', () => {
    // syncFromBackend 依赖 Tauri invoke，无法直接单测；
    // 这里测 mergeBackendIntoServices 不涉及 logLevel，logLevel 同步逻辑
    // 在 syncFromBackend 内联，改用下方纯函数测试。
    expect(true).toBe(true)
  })
})
```

> 说明：`syncFromBackend` 依赖 `invokeGetAppConfig`（Tauri invoke），在 vitest node 环境无法直接测。**采用替代方案**：把 logLevel 同步逻辑抽成纯函数 `applyBackendLogLevel(local: LogLevel, backend: string | undefined): LogLevel`，导出后单测。步骤 2 实现该纯函数。

替换上面占位测试为：

```ts
import { applyBackendLogLevel } from './settings'

describe('applyBackendLogLevel', () => {
  it('后端有效值覆盖前端', () => {
    expect(applyBackendLogLevel('info', 'debug')).toBe('debug')
    expect(applyBackendLogLevel('debug', 'error')).toBe('error')
  })

  it('后端 undefined 保留前端', () => {
    expect(applyBackendLogLevel('info', undefined)).toBe('info')
  })

  it('后端非法值保留前端', () => {
    expect(applyBackendLogLevel('warn', 'trace')).toBe('warn')
    expect(applyBackendLogLevel('warn', '')).toBe('warn')
  })
})
```

- [ ] **步骤 2：运行测试验证失败**

运行：`npm run test`
预期：FAIL，报错 `applyBackendLogLevel is not exported`。

- [ ] **步骤 3：实现 applyBackendLogLevel 并在 syncFromBackend 调用**

在 `frontend/src/settings/stores/settings.ts` 加导出纯函数（放在 `applyShortcutConflicts` 之后）：

```ts
const VALID_LOG_LEVELS: readonly LogLevel[] = ['error', 'warn', 'info', 'debug']

/** 后端 logLevel 有效则覆盖前端，否则保留前端。 */
export const applyBackendLogLevel = (
  local: LogLevel,
  backend: string | undefined,
): LogLevel =>
  backend && VALID_LOG_LEVELS.includes(backend as LogLevel)
    ? (backend as LogLevel)
    : local
```

并在 `syncFromBackend` 的「后端非空」分支（`state.services = mergeBackendIntoServices(...)` 那一段）末尾、`commitBaseline()` 之前加：

```ts
    state.advanced.logLevel = applyBackendLogLevel(
      state.advanced.logLevel,
      backend.logLevel,
    )
```

`backend` 变量在 `syncFromBackend` 里是 `AppConfig`（已含 `logLevel` 字段，任务 13 已加类型）。

- [ ] **步骤 4：运行测试验证通过**

运行：`npm run test`
预期：PASS。

- [ ] **步骤 5：typecheck 验证**

运行：`npm run typecheck`
预期：PASS。

- [ ] **步骤 6：Commit**

```bash
git add frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts
git commit -m "feat(logging): settings store 同步后端 logLevel"
```

---

## 任务 16：translate.js 与 overlay.html 接入 logger

**文件：**
- 修改：`frontend/public/translate.js`
- 修改：`frontend/public/overlay.html`

- [ ] **步骤 1：translate.js 接入 logger**

在 `frontend/public/translate.js` 顶部 `import { syncServiceCards } ...` 之后加：

```js
import { createLogger } from './logger.js';
const logger = createLogger('translate');
```

在 `if (listen) { ... }` 块的 `listen('app-config:changed', ...)` 回调里，`refreshCardsFromConfig(event.payload)` 之前加：

```js
    if (event.payload?.logLevel) logger.setLevel(event.payload.logLevel);
```

在 `initCards` 函数里，`refreshCardsFromConfig(await invoke('get_app_config'))` 改为：

```js
async function initCards() {
  if (!invoke) return;
  try {
    const config = await invoke('get_app_config');
    if (config?.logLevel) logger.setLevel(config.logLevel);
    refreshCardsFromConfig(config);
  } catch {
    return;
  }
}
```

在关键流程加日志：
- `startManualTranslation` 的 `catch`：`logger.error('手动翻译失败', String(error));`（在 `showToast` 之后）
- `cancelTranslation` 的 `catch`：`logger.warn('取消翻译失败', String(error));`
- `retryTranslation` 的 `catch`：`logger.error('重试失败', String(error));`
- `renderTranslationEvent` 的 `case 'failed'`：`logger.warn('翻译失败', { session: payload.sessionId, message: payload.message });`
- `renderTranslationEvent` 的 `case 'started'` 且 `isNewBatch`：`logger.info('翻译开始', { batch: batchId });`

- [ ] **步骤 2：overlay.html 接入 logger**

`frontend/public/overlay.html` 的内联 `<script>` 是非 module（`<script>`）。改为 `<script type="module">`，并在顶部加 import：

```html
  <script type="module">
    import { createLogger } from './logger.js';
    const logger = createLogger('overlay');
    const invoke = window.__TAURI__.core.invoke;
    // ... 原有代码
```

在关键节点加日志：
- `init()` 的 `catch`（底部 `init().catch(() => invoke('cancel_capture'))`）：改为 `init().catch((e) => { logger.error('overlay 初始化失败', String(e)); invoke('cancel_capture'); });`
- `mousedown` 右键/中键取消分支：`logger.info('框选取消：非左键');`
- `mouseup` 提交分支：`logger.info('框选提交', { x, y, w, h });`
- `mouseup` 取消分支（`!moving || w < 3 || h < 3`）：`logger.info('框选取消：区域过小');`
- `keydown` Escape：`logger.info('框选取消：Escape');`

启动时 setLevel：在 `init()` 里 `const meta = await invoke('get_capture_frame_meta');` 之前加：

```js
      try {
        const config = await invoke('get_app_config');
        if (config?.logLevel) logger.setLevel(config.logLevel);
      } catch (e) {
        logger.warn('读取配置失败', String(e));
      }
```

- [ ] **步骤 3：运行前端构建验证**

运行：`npm run build`
预期：BUILD SUCCEEDED（translate.js / overlay.html 是静态资源不参与 Vite 构建，但 build 不应报错）。

运行：`npm run typecheck`
预期：PASS（translate.js / overlay.html 不在 typecheck 范围）。

- [ ] **步骤 4：Commit**

```bash
git add frontend/public/translate.js frontend/public/overlay.html
git commit -m "feat(logging): translate.js 与 overlay.html 接入 logger"
```

---

## 任务 17：settings 页接入 logger

**文件：**
- 修改：`frontend/src/settings/stores/settings.ts`

- [ ] **步骤 1：settings.ts 接入 logger**

在 `frontend/src/settings/stores/settings.ts` 顶部 import 区加：

```ts
import { createLogger } from '@public/logger.js'
```

在 `const STORAGE_KEY = ...` 之后加：

```ts
const logger = createLogger('settings')
```

在关键流程加日志：
- `persist` 的 `catch (e)`：`logger.error('保存配置失败', String(e));`（在 `toast.error` 之后）
- `persist` 的 `if (err)`（validateConfig 失败）：`logger.warn('配置校验失败', err);`
- `syncFromBackend` 的 `try { backend = await invokeGetAppConfig() }` 的 `catch`：`logger.warn('从后端同步配置失败');`
- `syncFromBackend` 后端空分支 `await invokeSaveAppConfig(projectToAppConfig(state))` 的 `catch`：`logger.warn('推送配置到后端失败');`

- [ ] **步骤 2：运行 typecheck 验证**

运行：`npm run typecheck`
预期：PASS（`@public/logger.js` 经 allowJs 解析）。

运行：`npm run test`
预期：PASS。

- [ ] **步骤 3：Commit**

```bash
git add frontend/src/settings/stores/settings.ts
git commit -m "feat(logging): settings store 接入 logger"
```

---

## 任务 18：构建验证 + 文档同步

**文件：**
- 修改：`CLAUDE.md`、`AGENTS.md`、`README.md`、`docs/roadmap`、`docs/superpowers/specs/2026-07-08-advanced-logging-design.md`

- [ ] **步骤 1：全量构建验证**

运行：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
npm run typecheck
npm run test
```

预期：全部 PASS / SUCCEEDED。

- [ ] **步骤 2：手动验证（npm run tauri dev）**

逐项验证 spec 验收标准：
- 翻译流程产生 `Shizi.log` 与 `frontend.log` 两个独立文件，内容不互相混入。
- 修改日志等级并保存后即时生效（后端 `set_max_level` + 前端 `logger.setLevel`），无需重启。
- API Key 在两份日志中始终脱敏（前 4 + 后 4）。
- `info` 级别翻译正文只记摘要；`debug` 级别记全文。
- 单文件超 5MB 自动轮转，产生 `.1`/`.2`… 备份。
- 启动时清理 mtime > 7 天的日志文件（改文件 mtime 模拟）。
- 导出 zip 含 `Shizi.log*` + `frontend.log*` + `config-snapshot.json`（apiKey 脱敏）+ `system-info.txt`。
- 日志系统任何环节失败不影响翻译、截图、快捷键等主流程。

- [ ] **步骤 3：同步 CLAUDE.md 与 AGENTS.md**

在「架构关键点」的「前后端通信」节，补 `write_frontend_log` / `export_logs` 两个 command。在「配置存储」节，补 `logLevel` 字段与日志目录 `app_config_dir()/logs/` 说明。在「架构关键点」新增「日志系统」小节，说明前后端物理隔离、5MB 轮转、7 天清理、脱敏、导出 zip、best-effort 不影响主流程。**AGENTS.md 同步同样内容**（CLAUDE.md 与 AGENTS.md 保持同步是开发说明第 1 条硬门禁）。

- [ ] **步骤 4：同步 README**

在「当前能力」补日志能力（前后端独立日志、运行时等级切换、脱敏、轮转、导出）。在「限制」补：后端日志文件名为 `Shizi.log`（tauri-plugin-log 默认，不支持自定义）；API Key 明文保存（MVP）。

- [ ] **步骤 5：同步 roadmap 与 spec 回填**

`docs/roadmap` 中日志系统相关条目标记完成。`docs/superpowers/specs/2026-07-08-advanced-logging-design.md` 的「后端设计」节，把 `backend.log` 回填为实际 `Shizi.log` 并加注「tauri-plugin-log 默认文件名，不支持自定义」。

- [ ] **步骤 6：Commit**

```bash
git add CLAUDE.md AGENTS.md README.md docs/roadmap docs/superpowers/specs/2026-07-08-advanced-logging-design.md
git commit -m "docs(logging): 同步日志能力文档与 spec 文件名回填"
```

---

## 自检

### 1. 规格覆盖度

逐条对照 spec「必须实现」：

- ✅ 后端日志：core 层 `log` 门面 + 装配层 `tauri-plugin-log` → 任务 3/4/5/10/11
- ✅ 前端日志：`frontend/public/logger.js` + `write_frontend_log` command → 任务 6/12
- ✅ 分开保存：`Shizi.log`（tauri-plugin-log）与 `frontend.log`（std::fs append）物理隔离 → 任务 5/6
- ✅ 运行时等级切换即时生效 → 任务 8（后端 set_max_level）+ 任务 12/15/16（前端 setLevel + 订阅 app-config:changed）
- ✅ API Key 永远脱敏 + 翻译正文 info 摘要/debug 全文 → 任务 3（redact 纯函数）+ 任务 10/11（后端调用）+ 任务 12（前端 redactText）
- ✅ 5MB KeepAll 轮转 + 启动清理 7 天 → 任务 5（init_logging max_file_size + cleanup_old_logs）
- ✅ 导出 zip 含日志 + config-snapshot（脱敏）+ system-info → 任务 7

spec「明确不做」：按日期轮转（未做）、远程上报（未做）、tracing span（未做）、重构 config/store.rs 的 tauri 依赖（未做）、应用内查看器（未做）——均遵守。

spec「数据模型」`log_level` 字段 + 归一化 + camelCase + 纳入 save_app_config/app-config:changed → 任务 2/8/13/15。

spec「测试与验证」Rust 单测（redact、cleanup、write_frontend_log 轮转、AppConfig 归一化）→ 任务 2/3/4/6/7；前端 vitest（logger 等级/缓冲/flush/redactText、AdvancedPanel 等级/导出）→ 任务 12（logger）/任务 14（AdvancedPanel 接导出，靠 typecheck + 手动）/任务 15（logLevel 同步）；构建验证 → 任务 18。

### 2. 占位符扫描

- 任务 11 步骤 2/3 对 `claude.rs` 与 `ocr_translation.rs` 给了「先读文件确认字段名再照模式插入」的指示——这**不是占位符**，因为这两个文件的精确字段名需读后确认，计划给了明确的插入模式（`log::info!("Claude 请求: endpoint={} model={} key={}", ..., redact_api_key(api_key))`）与锚点（`let api_key = ...` 之后）。执行者读文件后照模式插入一行 `log!`，不涉及业务逻辑改动。
- 无「TODO」「待定」「类似任务 N」「为上述代码编写测试（无实际测试）」等红旗。

### 3. 类型一致性

- `FrontendLogEntry`：Rust（任务 6）`{ level: String, message: String, timestamp: String, source: String, meta: Option<serde_json::Value> }` 与前端（任务 13 `tauri.ts`）`{ level, message, timestamp, source, meta? }` 对齐，camelCase 一致。
- `write_frontend_log` / `export_logs` command 名在任务 6/7（Rust `#[tauri::command]`）与任务 13（前端 invoke）一致。
- `parse_level_filter` / `logs_dir` / `cleanup_old_logs` 在任务 4 定义，任务 5/6/7/8 调用，签名一致。
- `redact_api_key` / `redact_text` 在任务 3 定义，任务 7/10/11 调用，签名一致（`redact_text` 接 `&dyn Display`）。
- `applyBackendLogLevel` 在任务 15 定义并测试，`syncFromBackend` 内调用一致。
- `createLogger(source, deps?)` 在任务 12 定义，任务 16/17 调用一致。
- `LogLevel` 类型：前端 `types/config.ts`（任务 13）与 `settings/types.ts`（已有 `LogLevel`）一致；`AdvancedSettings.logLevel` 已存在，任务 15 操作 `state.advanced.logLevel`。
- `AppConfig.logLevel`：Rust `log_level`（任务 2，camelCase 序列化为 `logLevel`）与前端 `AppConfig.logLevel`（任务 13）对齐。
