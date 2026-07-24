# Windows 翻译弹窗双后端（WebView | WinUI/原生）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 仅翻译弹窗在 Windows 上可选 WebView / 原生（WinUI 取向）双后端，设置页可切换（重启生效），核心业务仍走 Rust；设置/OCR/overlay 继续 WebView。

**架构：** 同进程内 `PopupBackend` trait + 调度器；`WebviewPopupBackend` 包装现有 `popup_window`；`WinuiPopupBackend`（`cfg(windows)` + feature）实现原生弹窗。翻译事件经统一 `PopupViewModel` 管道：WebView 仍 emit `translation:event`，WinUI 侧归并为快照并绑定 UI。配置字段 `popupUiBackend`，默认 `webview`，切换需重启；WinUI 初始化失败降级 webview。

**技术栈：** Rust / Tauri 2、`windows` crate、Windows App SDK Bootstrap、Vue 设置页、cargo test / vitest

**规格来源：** `docs/superpowers/specs/2026-07-24-winui-popup-backend-design.md`

---

## 与 spec 的实现澄清（写死未决项）

### 1. WinUI crate / 声明式封装（最终选型）

| 项 | 决定 |
|----|------|
| 语言 / 运行时 | **纯 Rust**，**禁止** .NET / C# 工程 |
| 系统依赖 | 现有 `windows` crate 扩展；**Windows App SDK Bootstrap** 检测/初始化 Runtime |
| 声明式 UI 库 | **v1 不采用**（无 sycamore-winui / dioxus-desktop 等替代栈） |
| UI 构建方式 | Rust **命令式** 创建控件树 |
| 表面策略 | **两段式，M2-Task 0 spike 锁定其一** |

**M2-Task 0 spike（timebox ≤ 1 个有效工作日，结果必须 commit 记录）：**

- **路径 A（优先）**：Bootstrap Windows App Runtime → 创建 `Microsoft.UI.Xaml` 窗口 → 命令式 `StackPanel` / `TextBlock` / `ScrollViewer` 等拼出源文 + 一卡。
- **路径 B（spike 失败锁定）**：同一 `PopupBackend` 契约下，用 **Win32 分层窗口 + DWM 圆角/阴影**（已在 `windows` 0.58 能力内）实现同等主路径；配置枚举值仍为 `winui`。架构文档写清：「原生弹窗（WinAppSDK bootstrap 检测 + Win32 表面；XAML 宿主待生态成熟再迁）」。

**硬约束：** 无论 A/B，**不得**把翻译协议、配置持久化、历史写入放进 UI 层；失败必须能降级 `webview`。

### 2. `popup-winui` feature 与发布矩阵

```toml
# src-tauri/Cargo.toml
[features]
default = ["popup-winui"]
popup-winui = []
```

| 场景 | 行为 |
|------|------|
| Windows + `popup-winui`（默认） | 编译 `WinuiPopupBackend`；配置可选 `winui` |
| Windows + `--no-default-features` | 仅 webview；配置写 `winui` 时运行时强制 webview + warn 日志 |
| 非 Windows | `#[cfg(windows)]` 裁掉 winui 实现；配置可读写，行为恒 webview |
| CI `backend` job | `cargo test` / `cargo build` **带 default features**（即含 popup-winui） |
| `tauri build` / release / nightly | 继承 default features，产物可切换 winui |
| 本机加速编译 | 允许 `cargo test --no-default-features` 做纯逻辑测 |

### 3. 内存对比工具与门槛

| 项 | 决定 |
|----|------|
| 场景 | 仅托盘 + 弹窗预建并 hide，静置 ≥30s 后稳态 |
| 指标 | 进程 **Working Set** 与 **Private Bytes**（PowerShell 或任务管理器「专用工作集」） |
| 记录 | M4 写入 `docs/agent/architecture-notes.md` 小节「弹窗 backend 内存对照」（表格：日期、版本、backend、WS、Private） |
| CI 门槛 | **无数值 gate**；人工期望 winui 常驻 **不差于** webview，目标明显更低 |
| 排除 | 打开设置/OCR 后的峰值不计入「弹窗常驻」对比 |

示例采集命令（M4 手动执行，写入文档）：

```powershell
# 两后端各测一轮；进程名以实际 exe 为准
Get-Process shizi -ErrorAction SilentlyContinue |
  Select-Object Name, Id,
    @{N='WS_MB';E={[math]::Round($_.WorkingSet64/1MB,1)}},
    @{N='PM_MB';E={[math]::Round($_.PrivateMemorySize64/1MB,1)}}
```

### 4. Windows App Runtime 安装引导

| 项 | 决定 |
|----|------|
| 分发模型 | **框架依赖**（优先），不强制 MSIX / 自包含 |
| 检测时机 | 配置为 `winui` 且 feature 启用时，`ensure_created` 前 Bootstrap 初始化 |
| 失败 UX | 记日志；**本次进程降级 webview**；**一次性**系统 dialog：「需要 Windows App Runtime」→ 按钮「打开下载页」调用既有 `open_url`（https 官方安装文档）/「稍后」 |
| 下载 URL（写死） | `https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads` |
| NSIS | v1 **不改** installer 脚本；README / 架构文档写清依赖 |
| 自包含 Runtime | **非 v1** |

### 5. 切换与调度其它硬规则

1. `popupUiBackend` 默认 `"webview"`；缺省字段反序列化为 webview。
2. 切换 **重启生效**：设置页改值保存后 toast「重启后生效」；**不**在 `save_app_config` 内热拆 backend。
3. 启动时：`resolve_popup_backend_kind(config) → ActivePopupBackend` 一次选定；WinUI ensure 失败则同进程内改为 Webview 并 `manage` 降级标记。
4. 调用点统一走 `app/popup_host.rs`（或 `popup_backend/mod.rs` 导出的 facade），**禁止**业务路径直接 `get_webview_window("main")` 控制显隐（Webview 实现内部除外）。
5. `windowPrecreate.*.popup` 作用于当前激活 backend 的 `ensure_created`。
6. 非 Windows：`winui` 配置静默当 webview，不弹 dialog。

### 6. 本轮明确不做

- 设置/OCR/overlay 迁原生  
- macOS/Linux 原生弹窗  
- 子弹窗子进程  
- backend 热切换  
- .NET / C#  
- 应用内安装 Runtime / 改 NSIS 内嵌 bootstrapper  
- 与 WebView 像素级视觉一致  

---

## 文件结构

| 文件 | 职责 |
|------|------|
| 修改 `src-tauri/Cargo.toml` | `features.default/popup-winui`；必要时扩展 `windows` features |
| 创建 `src-tauri/src/app/popup_backend/mod.rs` | 模块导出、facade、`resolve_kind` |
| 创建 `src-tauri/src/app/popup_backend/types.rs` | `PopupUiBackendKind`、`PopupPositionMode`（迁入）、`PopupUserAction`、`PopupViewModel`、卡片状态 |
| 创建 `src-tauri/src/app/popup_backend/trait_api.rs` | `PopupBackend` trait |
| 创建 `src-tauri/src/app/popup_backend/host.rs` | 进程级 `PopupHost`（持有 `Box<dyn PopupBackend>` + 降级状态） |
| 创建 `src-tauri/src/app/popup_backend/view_model.rs` | `TranslationEvent` → `PopupViewModel` 归并（纯函数 + 单测） |
| 创建 `src-tauri/src/app/popup_backend/webview.rs` | `WebviewPopupBackend`（包装现 `popup_window`） |
| 创建 `src-tauri/src/app/popup_backend/winui/mod.rs` | `#[cfg(all(windows, feature = "popup-winui"))]` 入口 |
| 创建 `src-tauri/src/app/popup_backend/winui/backend.rs` | `WinuiPopupBackend` |
| 创建 `src-tauri/src/app/popup_backend/winui/bootstrap.rs` | App Runtime 检测 / Bootstrap |
| 创建 `src-tauri/src/app/popup_backend/winui/ui.rs` | 窗口与控件（路径 A 或 B） |
| 修改 `src-tauri/src/app/popup_window.rs` | 保留定位纯函数与 WebView 建窗；对外 show/hide 改为由 backend 调用或 re-export 兼容期 |
| 修改 `src-tauri/src/app/mod.rs` | `pub mod popup_backend` |
| 修改 `src-tauri/src/app/state.rs` | 可选：不强制塞 backend；backend 放 `app.manage(PopupHost)` |
| 修改 `src-tauri/src/lib.rs` | setup 创建 `PopupHost`；预建走 host；退出 `destroy` |
| 修改 `src-tauri/src/ui/web_popup.rs` | show 走 host；emit 事件同时 `host.publish_from_event` |
| 修改 `src-tauri/src/app/window.rs` / `shortcuts.rs` / `tray.rs` | show/hide 走 host |
| 修改 `src-tauri/src/ui/ocr_popup.rs` / `ocr_window.rs` | `hide` 走 host |
| 修改 `src-tauri/src/core/config/types.rs` | `popup_ui_backend` 字段 + 归一化 + 单测 |
| 修改 `frontend/src/types/config.ts` | `popupUiBackend` |
| 修改 `frontend/src/lib/config.ts` + `config.test.ts` | 投影 |
| 修改 `frontend/src/settings/types.ts` / `stores/settings.ts` + tests | general 字段 |
| 修改 `frontend/src/settings/panels/GeneralPanel.vue` | Windows 展示切换 + 重启提示 |
| 修改 `frontend/src/i18n/locales/*.json`（8） | 文案 key |
| 修改 `.github/workflows/ci.yml` | 确认 Windows job 带 default features；必要时装 Windows App SDK 构建组件 |
| 修改 `docs/agent/architecture-notes.md` / `README.md` / `AGENTS.md` / `CLAUDE.md` | 收尾文档 |
| 修改 spec 验收勾选（实现完成后） | `docs/superpowers/specs/2026-07-24-winui-popup-backend-design.md` |

**刻意不改：** `pot-desktop/`、设置/OCR WebView 生命周期、翻译 core 协议、历史库 schema。

---

## 里程碑映射

| 里程碑 | 任务 |
|--------|------|
| **M1** 抽出 PopupBackend，WebView 零变化 | 任务 1–5 |
| **M2** 原生最小壳 | 任务 6–9 |
| **M3** 主路径对齐 + 设置切换 + 降级 | 任务 10–14 |
| **M4** 视觉/内存/CI/文档 | 任务 15–17 |

---

## 任务 1：配置字段 `popupUiBackend`（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 测试：同文件 `#[cfg(test)]`

- [ ] **步骤 1：编写失败的测试**

```rust
#[test]
fn app_config_defaults_popup_ui_backend_webview() {
    let config = AppConfig::default();
    assert_eq!(config.popup_ui_backend, "webview");
}

#[test]
fn app_config_missing_popup_ui_backend_deserializes_to_webview() {
    let json = r#"{"targetLang":"zh-CN","services":[],"ocrServices":[]}"#;
    let config: AppConfig = serde_json::from_str(json).expect("deserialize");
    let config = config.normalized();
    assert_eq!(config.popup_ui_backend, "webview");
}

#[test]
fn app_config_popup_ui_backend_roundtrip_camel_case() {
    let mut config = AppConfig::default();
    config.popup_ui_backend = "winui".into();
    let json = serde_json::to_string(&config).expect("ser");
    assert!(json.contains("\"popupUiBackend\":\"winui\""), "got {json}");
    let back: AppConfig = serde_json::from_str(&json).expect("de");
    assert_eq!(back.popup_ui_backend, "winui");
}

#[test]
fn normalized_rejects_unknown_popup_ui_backend() {
    let mut config = AppConfig::default();
    config.popup_ui_backend = "qt".into();
    let n = config.normalized();
    assert_eq!(n.popup_ui_backend, "webview");
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cd src-tauri && cargo test app_config_defaults_popup_ui_backend_webview -- --nocapture
```

预期：FAIL（无字段 / 编译错误）。

- [ ] **步骤 3：最少实现**

在 `AppConfig` 增加：

```rust
#[serde(default = "default_popup_ui_backend")]
pub popup_ui_backend: String,
```

```rust
fn default_popup_ui_backend() -> String {
    "webview".to_string()
}

fn normalize_popup_ui_backend(value: String) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "winui" => "winui".to_string(),
        _ => "webview".to_string(),
    }
}
```

在 `AppConfig::default` 填 `"webview"`；在 `normalized` 调用 `normalize_popup_ui_backend`。

- [ ] **步骤 4：运行测试验证通过**

```bash
cd src-tauri && cargo test popup_ui_backend -- --nocapture
```

预期：相关测试 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): 新增 popupUiBackend 配置字段"
```

---

## 任务 2：`PopupViewModel` 归并纯函数（TDD）

**文件：**
- 创建：`src-tauri/src/app/popup_backend/view_model.rs`
- 创建：`src-tauri/src/app/popup_backend/types.rs`（ViewModel / Card 类型）
- 创建：`src-tauri/src/app/popup_backend/mod.rs`（`pub mod`）
- 修改：`src-tauri/src/app/mod.rs`（`pub mod popup_backend`）

- [ ] **步骤 1：定义类型并写失败测试**

`types.rs` 核心类型（字段可按实现微调，但任务间保持同一命名）：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupPositionMode {
    #[default]
    NearCursor,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupUiBackendKind {
    Webview,
    Winui,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupCardStatus {
    Pending,
    Translating,
    Finished,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PopupCardVm {
    pub service_instance_id: String,
    pub service_name: String,
    pub service_type: String,
    pub protocol: String,
    pub model_name: String,
    pub status: PopupCardStatus,
    pub text: String,
    pub error_message: String,
    pub usage_input: Option<u32>,
    pub usage_output: Option<u32>,
    pub detected_source_lang: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PopupViewModel {
    pub session_id: Option<String>,
    pub source_text: String,
    pub source_type: String,
    pub source_lang: String,
    pub target_lang: String,
    pub is_translating: bool,
    pub cards: Vec<PopupCardVm>,
}

#[derive(Debug, Clone)]
pub enum PopupUserAction {
    Close,
    CancelTranslation,
    Retry { service_instance_id: Option<String> },
    CopyResult { service_instance_id: String },
    OpenSettings,
    SetSessionLanguages { source_lang: String, target_lang: String },
}
```

`view_model.rs`：

```rust
use crate::core::translation::TranslationEvent;
use super::types::{PopupCardStatus, PopupCardVm, PopupViewModel};

pub fn apply_translation_event(vm: &mut PopupViewModel, event: &TranslationEvent) {
    // 实现见步骤 3
    let _ = (vm, event);
}
```

测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::translation::{TokenUsage, TranslationEvent, TranslationServiceMeta, TranslationSessionId};

    fn meta(id: &str) -> TranslationServiceMeta {
        TranslationServiceMeta {
            service_instance_id: id.into(),
            service_name: "Mock".into(),
            service_type: "llm".into(),
            protocol: "mock".into(),
            model_name: "m".into(),
        }
    }

    #[test]
    fn started_sets_session_and_card_translating() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hello".into(),
                source_type: "selectedText".into(),
            },
        );
        assert_eq!(vm.source_text, "hello");
        assert!(vm.is_translating);
        assert_eq!(vm.cards.len(), 1);
        assert_eq!(vm.cards[0].status, PopupCardStatus::Translating);
        assert_eq!(vm.cards[0].text, "");
    }

    #[test]
    fn delta_appends_text() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hi".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                text: "你".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                text: "好".into(),
            },
        );
        assert_eq!(vm.cards[0].text, "你好");
    }

    #[test]
    fn finished_sets_full_text_and_usage() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "hi".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Finished {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                full_text: "你好".into(),
                usage: Some(TokenUsage { input_tokens: 1, output_tokens: 2 }),
                detected_source_lang: Some("en".into()),
            },
        );
        assert_eq!(vm.cards[0].status, PopupCardStatus::Finished);
        assert_eq!(vm.cards[0].text, "你好");
        assert_eq!(vm.cards[0].usage_input, Some(1));
        assert_eq!(vm.cards[0].detected_source_lang.as_deref(), Some("en"));
    }

    #[test]
    fn stale_session_delta_is_ignored() {
        let mut vm = PopupViewModel::default();
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Started {
                session_id: TranslationSessionId("b1:svc".into()),
                service: meta("svc"),
                source_text: "a".into(),
                source_type: "manualText".into(),
            },
        );
        apply_translation_event(
            &mut vm,
            &TranslationEvent::Delta {
                session_id: TranslationSessionId("old:svc".into()),
                service: meta("svc"),
                text: "丢弃".into(),
            },
        );
        assert_eq!(vm.cards[0].text, "");
    }
}
```

- [ ] **步骤 2：运行测试验证失败**

```bash
cd src-tauri && cargo test apply_translation_event -- --nocapture
```

预期：FAIL。

- [ ] **步骤 3：实现 `apply_translation_event`**

规则对齐前端 `useTranslationEvents`：

- `Started`：若 batch 前缀（`session_id` 中 `:` 前）变化 → 清空 cards 文本状态、`is_translating=true`、写 `source_text/source_type`；确保对应 card 为 `Translating`、text 清空。
- `Delta`：session 匹配才 append。
- `Finished` / `Failed` / `Cancelled`：更新对应 card；当所有 card 非 Translating/Pending 时 `is_translating=false`（与前端 batch 状态类似即可，允许简化：任一次终态后若无 Translating 则 false）。

- [ ] **步骤 4：测试通过**

```bash
cd src-tauri && cargo test popup_backend::view_model -- --nocapture
```

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_backend src-tauri/src/app/mod.rs
git commit -m "feat(popup): 新增 PopupViewModel 与 translation 事件归并"
```

---

## 任务 3：`PopupBackend` trait + `PopupHost` + mock 调度测试（TDD）

**文件：**
- 创建：`src-tauri/src/app/popup_backend/trait_api.rs`
- 创建：`src-tauri/src/app/popup_backend/host.rs`
- 修改：`mod.rs`

- [ ] **步骤 1：写 mock 测试（失败）**

```rust
// host.rs 或 host 旁 tests
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct MockBackend {
        log: Arc<Mutex<Vec<&'static str>>>,
        visible: bool,
        alive: bool,
    }

    impl PopupBackend for MockBackend {
        fn kind(&self) -> PopupUiBackendKind { PopupUiBackendKind::Webview }
        fn ensure_created(&mut self) -> Result<(), String> {
            self.alive = true;
            self.log.lock().unwrap().push("ensure");
            Ok(())
        }
        fn show(&mut self, _mode: PopupPositionMode) -> Result<(), String> {
            self.visible = true;
            self.log.lock().unwrap().push("show");
            Ok(())
        }
        fn hide(&mut self) {
            self.visible = false;
            self.log.lock().unwrap().push("hide");
        }
        fn destroy(&mut self) {
            self.alive = false;
            self.visible = false;
            self.log.lock().unwrap().push("destroy");
        }
        fn is_visible(&self) -> bool { self.visible }
        fn is_alive(&self) -> bool { self.alive }
        fn publish(&mut self, _vm: &PopupViewModel) {
            self.log.lock().unwrap().push("publish");
        }
    }

    #[test]
    fn host_hide_is_idempotent() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut host = PopupHost::from_backend(Box::new(MockBackend {
            log: log.clone(),
            visible: false,
            alive: false,
        }));
        host.ensure_created().unwrap();
        host.show(PopupPositionMode::NearCursor).unwrap();
        host.hide();
        host.hide();
        assert!(!host.is_visible());
        let ops = log.lock().unwrap().clone();
        assert_eq!(ops.iter().filter(|x| **x == "hide").count(), 2);
    }

    #[test]
    fn resolve_kind_winui_without_feature_falls_back_webview() {
        // 非 windows 或无 feature 时
        assert_eq!(
            resolve_popup_backend_kind("winui", /* feature_enabled */ false, /* is_windows */ true),
            PopupUiBackendKind::Webview
        );
        assert_eq!(
            resolve_popup_backend_kind("winui", true, false),
            PopupUiBackendKind::Webview
        );
        assert_eq!(
            resolve_popup_backend_kind("winui", true, true),
            PopupUiBackendKind::Winui
        );
        assert_eq!(
            resolve_popup_backend_kind("webview", true, true),
            PopupUiBackendKind::Webview
        );
    }
}
```

- [ ] **步骤 2：运行确认失败**

```bash
cd src-tauri && cargo test resolve_kind_winui -- --nocapture
```

- [ ] **步骤 3：实现 trait 与 host**

```rust
// trait_api.rs
pub trait PopupBackend: Send {
    fn kind(&self) -> PopupUiBackendKind;
    fn ensure_created(&mut self) -> Result<(), String>;
    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String>;
    fn hide(&mut self);
    fn destroy(&mut self);
    fn is_visible(&self) -> bool;
    fn is_alive(&self) -> bool;
    fn publish(&mut self, vm: &PopupViewModel);
}

// host.rs
pub struct PopupHost {
    backend: Box<dyn PopupBackend>,
    view_model: PopupViewModel,
    degraded_from_winui: bool,
}

impl PopupHost {
    pub fn from_backend(backend: Box<dyn PopupBackend>) -> Self { /* ... */ }
    pub fn ensure_created(&mut self) -> Result<(), String> { self.backend.ensure_created() }
    pub fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> { /* ensure + show */ }
    pub fn hide(&mut self) { self.backend.hide() }
    pub fn destroy(&mut self) { self.backend.destroy() }
    pub fn is_visible(&self) -> bool { self.backend.is_visible() }
    pub fn publish_from_event(&mut self, event: &TranslationEvent) {
        apply_translation_event(&mut self.view_model, event);
        self.backend.publish(&self.view_model);
    }
    pub fn kind(&self) -> PopupUiBackendKind { self.backend.kind() }
}

pub fn resolve_popup_backend_kind(
    config_value: &str,
    feature_enabled: bool,
    is_windows: bool,
) -> PopupUiBackendKind {
    if config_value == "winui" && feature_enabled && is_windows {
        PopupUiBackendKind::Winui
    } else {
        PopupUiBackendKind::Webview
    }
}
```

`feature_enabled` 编译期：

```rust
pub const POPUP_WINUI_FEATURE: bool = cfg!(all(windows, feature = "popup-winui"));
```

- [ ] **步骤 4：测试通过 + Commit**

```bash
cd src-tauri && cargo test popup_backend::host -- --nocapture
git add src-tauri/src/app/popup_backend
git commit -m "feat(popup): 新增 PopupBackend trait 与 PopupHost 调度"
```

---

## 任务 4：`WebviewPopupBackend` 包装现网弹窗

**文件：**
- 创建：`src-tauri/src/app/popup_backend/webview.rs`
- 修改：`popup_window.rs`（保持 `build_popup` / `compute_popup_position` / 测试；公共 API 可保留为薄包装以降低一次性 diff）

- [ ] **步骤 1：实现 Webview backend**

```rust
pub struct WebviewPopupBackend {
    app: tauri::AppHandle,
}

impl WebviewPopupBackend {
    pub fn new(app: tauri::AppHandle) -> Self { Self { app } }
}

impl PopupBackend for WebviewPopupBackend {
    fn kind(&self) -> PopupUiBackendKind { PopupUiBackendKind::Webview }

    fn ensure_created(&mut self) -> Result<(), String> {
        crate::app::popup_window::ensure_popup_exists(&self.app).map(|_| ())
    }

    fn show(&mut self, mode: PopupPositionMode) -> Result<(), String> {
        // 映射 PopupPositionMode → popup_window::PopupPositionMode（可先统一类型到 popup_backend::types 再让 popup_window re-export）
        crate::app::popup_window::show_popup_blocking(
            &self.app,
            &crate::core::config::AppConfig::default(), // show 路径现不依赖 config 内容
            map_mode(mode),
        )
    }

    fn hide(&mut self) {
        crate::app::popup_window::hide_popup(&self.app);
    }

    fn destroy(&mut self) {
        if let Some(w) = self.app.get_webview_window(crate::app::popup_window::POPUP_LABEL) {
            let _ = w.close(); // 或 destroy；需验证 Tauri 2 API：关闭后可重建
        }
    }

    fn is_visible(&self) -> bool {
        self.app
            .get_webview_window(crate::app::popup_window::POPUP_LABEL)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    }

    fn is_alive(&self) -> bool {
        self.app.get_webview_window(crate::app::popup_window::POPUP_LABEL).is_some()
    }

    fn publish(&mut self, _vm: &PopupViewModel) {
        // WebView 路径继续靠 translation:event；此处 no-op 或预留
    }
}
```

**注意：** 现网 `show_popup` 对「首次创建」用独立线程防死锁。`WebviewPopupBackend::show` 应复用该策略：若窗不存在，`thread::spawn` 调 `show_popup_blocking`，与现网一致。

- [ ] **步骤 2：类型统一 `PopupPositionMode`**

优先：把 `popup_window::PopupPositionMode` 改为 `pub use crate::app::popup_backend::types::PopupPositionMode`，全库只保留一份，避免双定义漂移。

- [ ] **步骤 3：单测仍绿**

```bash
cd src-tauri && cargo test compute_popup_position -- --nocapture
cd src-tauri && cargo test popup_backend -- --nocapture
```

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_backend src-tauri/src/app/popup_window.rs
git commit -m "feat(popup): WebviewPopupBackend 包装现有弹窗"
```

---

## 任务 5：接入 `PopupHost` 到启动与调用点（行为零变化）

**文件：**
- 修改：`lib.rs`、`ui/web_popup.rs`、`app/window.rs`、`app/shortcuts.rs`、`app/tray.rs`、`ui/ocr_popup.rs`、`ui/ocr_window.rs`

- [ ] **步骤 1：setup 创建 host**

```rust
// lib.rs setup 内，在 config 读出后：
let kind = popup_backend::resolve_popup_backend_kind(
    &config.popup_ui_backend,
    popup_backend::POPUP_WINUI_FEATURE,
    cfg!(windows),
);
// M1 阶段：即使 kind==Winui 也先强制 Webview（WinUI 实现尚未就绪时）
// 或：Winui 分支暂 Err → 在任务 8 接真实现
let backend = popup_backend::create_backend(app.handle(), kind);
app.manage(std::sync::Mutex::new(popup_backend::PopupHost::from_backend(backend)));

// 预建：
if config.window_precreate.for_launch(is_autostart).popup {
    if let Ok(mut host) = app.state::<Mutex<PopupHost>>().lock() {
        let _ = host.ensure_created();
    }
}
```

`create_backend`：

```rust
pub fn create_backend(app: &AppHandle, kind: PopupUiBackendKind) -> Box<dyn PopupBackend> {
    match kind {
        PopupUiBackendKind::Webview => Box::new(WebviewPopupBackend::new(app.clone())),
        #[cfg(all(windows, feature = "popup-winui"))]
        PopupUiBackendKind::Winui => Box::new(winui::WinuiPopupBackend::new(app.clone())),
        #[cfg(not(all(windows, feature = "popup-winui")))]
        PopupUiBackendKind::Winui => Box::new(WebviewPopupBackend::new(app.clone())),
    }
}
```

M1 若 Winui 类型尚不存在：`create_backend` 对 Winui 也返回 Webview，并 `log::warn`。

- [ ] **步骤 2：替换调用点**

| 原调用 | 新调用 |
|--------|--------|
| `ensure_popup_window` | host `ensure_created`（保留函数为 facade 亦可） |
| `show_popup` / `show_popup_blocking` / `show_translation_popup_with` | `host.show(mode)` |
| `hide_popup` | `host.hide()` |

facade 建议放 `popup_backend/mod.rs`：

```rust
pub fn with_host<R>(app: &AppHandle, f: impl FnOnce(&mut PopupHost) -> R) -> Result<R, String> {
    let state = app.state::<Mutex<PopupHost>>();
    let mut guard = state.lock().map_err(|_| "PopupHost lock poisoned".to_string())?;
    Ok(f(&mut guard))
}
```

- [ ] **步骤 3：翻译事件双发**

在 `emit_translation_event` 成功后：

```rust
pub fn emit_translation_event(app: &AppHandle, event: TranslationEvent) -> Result<(), tauri::Error> {
    let result = app.emit(TRANSLATION_EVENT, &event);
    if let Ok(mut host) = app.state::<Mutex<PopupHost>>().lock() {
        host.publish_from_event(&event);
    }
    result
}
```

- [ ] **步骤 4：验证**

```bash
cd src-tauri && cargo test
```

手动（dev）：划词 / 托盘打开 / 关闭 hide / 设置仍 WebView — 行为与改造前一致。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src
git commit -m "refactor(popup): 全路径经 PopupHost 调度（WebView 行为不变）"
```

**M1 完成门禁：** 默认配置下产品行为相对现网零变化；`cargo test` 全绿。

---

## 任务 6：Cargo feature + WinUI 模块骨架

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 创建：`src-tauri/src/app/popup_backend/winui/mod.rs`、`backend.rs`、`bootstrap.rs`、`ui.rs`

- [ ] **步骤 1：features**

```toml
[features]
default = ["popup-winui"]
popup-winui = []
```

- [ ] **步骤 2：骨架（编译通过即可）**

```rust
// winui/mod.rs
#![cfg(all(windows, feature = "popup-winui"))]
mod backend;
mod bootstrap;
mod ui;
pub use backend::WinuiPopupBackend;

// backend.rs：ensure/show/hide/destroy 先返回 Err("not implemented") 或 no-op 窗体
```

- [ ] **步骤 3：**

```bash
cd src-tauri && cargo test
cd src-tauri && cargo test --no-default-features
```

两模式均能编译测试。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/app/popup_backend
git commit -m "chore(popup): 增加 popup-winui feature 与 winui 模块骨架"
```

---

## 任务 7：M2 Spike — 原生窗最小可显示（路径 A 或 B）

**文件：**
- 修改：`winui/bootstrap.rs`、`winui/ui.rs`、`winui/backend.rs`
- 可选记录：`docs/agent/architecture-notes.md` 增加「WinUI 表面路径：A 或 B」一句

- [ ] **步骤 1：Bootstrap / Runtime 探测**

`bootstrap.rs`：

```rust
pub struct BootstrapStatus {
    pub ok: bool,
    pub message: String,
}

/// 尝试初始化 Windows App Runtime；路径 B 可仅做存在性探测或直接 Ok。
pub fn try_bootstrap() -> BootstrapStatus { /* ... */ }
```

路径 A：调用 Windows App SDK Bootstrap API（按 spike 查到的官方签名）。  
路径 B：可返回 `ok: true` 并在注释标明「未依赖 XAML Runtime」。

- [ ] **步骤 2：创建隐藏窗口**

`ui.rs` 最低要求：

- 无任务栏按钮（或 `WS_EX_TOOLWINDOW`）
- 无系统厚边框（可自绘客户区）
- 初始 `SW_HIDE`
- 尺寸约 420×360 逻辑像素

- [ ] **步骤 3：`WinuiPopupBackend` 实现 ensure/show/hide**

- `ensure_created`：bootstrap + 建窗  
- `show(NearCursor)`：复用 `popup_window::compute_popup_position` + `cursor_logical_context`  
- `show(Restore)`：不改坐标  
- `hide`：幂等  
- `destroy`：销毁 HWND / XAML Window  

- [ ] **步骤 4：手动验证**

配置临时写死 kind=Winui（或单元外手动 `create_backend`），`ensure` + `show` 应看到空窗/占位窗，hide 消失不退出进程。

- [ ] **步骤 5：Commit（含路径结论）**

```bash
git add src-tauri/src/app/popup_backend/winui docs/agent/architecture-notes.md
git commit -m "feat(popup): WinUI/原生弹窗最小壳 ensure/show/hide"
```

若路径 A 失败锁定 B：commit message / architecture-notes 写明「采用路径 B：Win32 表面」。

---

## 任务 8：最小内容 — 源文 + 至少一卡流式

**文件：**
- 修改：`winui/ui.rs`、`backend.rs`（`publish`）

- [ ] **步骤 1：`publish` 绑定**

`publish(&PopupViewModel)`：

- 源文 `TextBlock` / 只读编辑框 ← `vm.source_text`
- 卡片列表：至少渲染 `cards[0..]` 的 `service_name` + `text` + 简单状态色
- 流式：每次 `publish` 全量刷新文本即可（v1 不做虚拟列表）

- [ ] **步骤 2：线程安全**

翻译回调可能在 tokio 线程：`publish` 必须 **投递到 UI 线程**（路径 A：DispatcherQueue；路径 B：`PostMessage` / 隐藏 message window）。单测可测「队列入队」辅助函数，不测真实 HWND。

- [ ] **步骤 3：手动**

mock 服务或真实服务：划词 → 源文出现 → delta 追加 → finished 完整。

- [ ] **步骤 4：Commit**

```bash
git commit -m "feat(popup): 原生弹窗绑定源文与流式结果卡"
```

---

## 任务 9：WinUI 初始化失败 → 降级 webview

**文件：**
- 修改：`host.rs`、`lib.rs` setup、`winui/backend.rs`

- [ ] **步骤 1：host 支持替换 backend**

```rust
impl PopupHost {
    pub fn replace_backend(&mut self, backend: Box<dyn PopupBackend>) {
        self.backend.destroy();
        self.backend = backend;
        self.degraded_from_winui = true;
    }
}
```

- [ ] **步骤 2：setup 逻辑**

```rust
let mut host = PopupHost::from_backend(create_backend(handle, kind));
if kind == PopupUiBackendKind::Winui {
    if let Err(err) = host.ensure_created() {
        log::error!("WinUI 弹窗初始化失败，降级 webview: {err}");
        host.replace_backend(Box::new(WebviewPopupBackend::new(handle.clone())));
        // 一次性 dialog：tauri-plugin-dialog
        // 文案中文常量；按钮打开 Runtime 下载 URL
        let _ = host.ensure_created();
    }
}
app.manage(Mutex::new(host));
```

- [ ] **步骤 3：单测降级标记 / resolve 逻辑（无需真窗）**

```rust
#[test]
fn degraded_flag_set_on_replace() {
    // Mock ensure fail → replace → degraded_from_winui == true
}
```

- [ ] **步骤 4：Commit**

```bash
git commit -m "fix(popup): WinUI 初始化失败时降级 WebView 并提示 Runtime"
```

**M2 完成门禁：** 配置 winui 时能 show 源文+流式卡；失败可 webview 继续翻译。

---

## 任务 10：主路径用户动作对齐

**文件：**
- 修改：`winui/ui.rs`、用户动作回调 → 现有 commands / `AppState` API

- [ ] **步骤 1：动作接线**

| UI 控件 | `PopupUserAction` / 调用 |
|---------|--------------------------|
| 关闭 | `hide`（不 destroy） |
| 取消 | `cancel_translation` 同等逻辑（复用 `AppState` cancel token） |
| 重试 | `retry_translation` |
| 复制 | 系统剪贴板写 card 文本 |
| 打开设置 | `show_settings_window` / `open_settings` |
| 源/目标语言 | `set_session_languages` + 可选触发重译 |

实现方式：backend 持有 `AppHandle`，在 UI 回调里 `app.state::<AppState>()` + 调 `ui::web_popup` 已有函数，**禁止复制翻译协议代码**。

- [ ] **步骤 2：手动清单**

划词、截图译、多服务卡、取消、换语言、复制、打开设置后关设置（设置 WebView 仍销毁）。

- [ ] **步骤 3：Commit**

```bash
git commit -m "feat(popup): 原生弹窗主路径用户动作对接 core"
```

---

## 任务 11：前端配置类型与投影（TDD）

**文件：**
- `frontend/src/types/config.ts`
- `frontend/src/lib/config.ts`
- `frontend/src/lib/config.test.ts`
- `frontend/src/settings/types.ts`
- `frontend/src/settings/stores/settings.ts`
- `frontend/src/settings/stores/settings.test.ts`（及所有 `makeAppConfig` 夹具）

- [ ] **步骤 1：失败测试**

```ts
// config.test.ts
it('投影 popupUiBackend，默认 webview', () => {
  const state = makeSettingsState()
  expect(projectToAppConfig(state).popupUiBackend).toBe('webview')
  state.general.popupUiBackend = 'winui'
  expect(projectToAppConfig(state).popupUiBackend).toBe('winui')
})
```

- [ ] **步骤 2：运行**

```bash
npm run test -- frontend/src/lib/config.test.ts
```

预期：FAIL。

- [ ] **步骤 3：实现**

```ts
// types/config.ts
export type PopupUiBackend = 'webview' | 'winui'
// AppConfig 增加：
popupUiBackend: PopupUiBackend
```

```ts
// settings/types.ts GeneralSettings
popupUiBackend: PopupUiBackend
```

`projectToAppConfig` / `syncFromBackend` / defaults / 所有测试夹具补字段。

- [ ] **步骤 4：**

```bash
npm run test
npm run typecheck
```

- [ ] **步骤 5：Commit**

```bash
git commit -m "feat(settings): 前端同步 popupUiBackend 配置"
```

---

## 任务 12：设置页 UI（仅 Windows 展示）+ 重启提示

**文件：**
- `frontend/src/settings/panels/GeneralPanel.vue`
- `frontend/src/i18n/locales/{zh-CN,zh-TW,en-US,ja-JP,ko-KR,fr-FR,de-DE,es-ES}.json`

- [ ] **步骤 1：i18n keys（8 语都补）**

```json
"settings.field.popupUiBackend": "翻译弹窗 UI",
"settings.description.popupUiBackend": "WinUI/原生更跟手、利于常驻内存；WebView 与现网一致。切换后需重启应用。",
"settings.option.popupWebview": "WebView",
"settings.option.popupWinui": "WinUI（预览）",
"settings.toast.popupBackendRestart": "弹窗 UI 后端已保存，重启应用后生效"
```

英文等语言给出对应翻译（en-US 必须自然英文）。

- [ ] **步骤 2：GeneralPanel**

```vue
<!-- 仅 Tauri Windows 显示：用 import.meta 或简单 userAgent/platform 探测 -->
<SettingRow
  v-if="isWindowsDesktop"
  :title="t('settings.field.popupUiBackend')"
  :description="t('settings.description.popupUiBackend')"
>
  <SettingSelect v-model="state.general.popupUiBackend" :options="popupBackendOptions" />
</SettingRow>
```

保存路径：若 `popupUiBackend` 相对 `syncFromBackend` 初始值变化，在既有 save 成功 toast 后追加 `settings.toast.popupBackendRestart`（或合并进成功提示）。

`isWindowsDesktop`：

```ts
const isWindowsDesktop = computed(() => {
  const tauri = (window as unknown as { __TAURI__?: unknown }).__TAURI__
  if (!tauri) return false
  return navigator.userAgent.includes('Windows')
})
```

- [ ] **步骤 3：**

```bash
npm run test
npm run typecheck
```

- [ ] **步骤 4：Commit**

```bash
git commit -m "feat(settings): Windows 设置页可切换弹窗 UI 后端"
```

---

## 任务 13：启动选用配置 backend（真切换）

**文件：**
- `lib.rs` / `create_backend`：去掉 M1「Winui 假 Webview」临时逻辑

- [x] **步骤 1：** 按 `config.popup_ui_backend` + feature + platform 创建对应 backend（`create_backend` 真切换 + `create_host_with_winui_fallback` 降级；无 M1 假 Webview）。
- [ ] **步骤 2：** 手动：设置 winui → 重启 → 原生弹窗；改回 webview → 重启 → Vue 弹窗。
- [x] **步骤 3：** Commit（含架构笔记同步 + `resolve_kind_winui_with_feature_is_winui` 单测）

```bash
git commit -m "feat(popup): 启动按 popupUiBackend 选择弹窗后端"
```

---

## 任务 14：多服务卡与 chrome 状态

**文件：**
- `winui/ui.rs`

- [ ] **步骤 1：** 多卡列表（ScrollViewer）；每卡显示 name / protocol 或 model（与 Web 规则一致：`microsoft_edge` 不强调模型）/ 状态 / 文本 / 失败信息。
- [ ] **步骤 2：** chrome：`is_translating` 时显示取消；否则隐藏或禁用。
- [ ] **步骤 3：** 手动多服务并发；单服务失败其它仍完成。
- [ ] **步骤 4：** Commit

```bash
git commit -m "feat(popup): 原生弹窗多服务卡片与翻译中状态"
```

**M3 完成门禁：** 设置可切换；双 backend 主路径可用；降级可用。

---

## 任务 15：视觉与定位打磨

**文件：**
- `winui/ui.rs`、定位常量

- [x] 圆角/阴影/字体接近系统 Fluent（路径 A 用 WinUI 主题；路径 B 用 DWM + 合理 padding）
- [x] 宽度 ~420；高度随内容有上限（可先固定 max height + 滚动）
- [x] `NearCursor` 与 WebView 共用 `compute_popup_position`
- [x] Commit：`style(popup): 原生弹窗视觉与定位打磨`

---

## 任务 16：CI / 开发依赖文档 / 内存记录

**文件：**
- `.github/workflows/ci.yml`
- 可选 `release.yml` / `nightly.yml`（仅当 build 缺 SDK 时）
- `README.md`、`docs/agent/architecture-notes.md`

- [ ] **步骤 1：CI**

确认 `backend` job：

```yaml
- run: cargo test
  working-directory: src-tauri
- run: cargo build
  working-directory: src-tauri
```

若 WinAppSDK 头/库缺失导致 winui 编译失败：在 Windows job 增加安装步骤（例如 `winget` / 下载 Windows App SDK redistributable 或 build tools）。**优先**让路径 B 在无完整 XAML 工具链时仍能 `cargo test` 绿。

- [ ] **步骤 2：文档**

`architecture-notes.md` 增补：

- `PopupBackend` / `popupUiBackend` / 重启切换 / 降级
- 开发依赖：Windows 10/11、WebView2、Windows App Runtime（winui 路径）
- 内存对照表（填实测）

`README.md`：用户可见说明「翻译弹窗可选原生 UI（Windows）」。

- [ ] **步骤 3：内存实测**（本机，两 backend 各一轮）写入表格。
- [ ] **步骤 4：Commit**

```bash
git commit -m "docs(ci): 弹窗双后端 CI 说明与架构/内存文档"
```

---

## 任务 17：文档同步硬门禁 + AGENTS/CLAUDE

**文件：**
- `docs/agent/architecture-notes.md`（若任务 16 未写全）
- `AGENTS.md` / `CLAUDE.md`（同步要点：弹窗 backend 配置、分层仍 core 在 Rust）
- `docs/superpowers/specs/2026-07-24-winui-popup-backend-design.md` 验收清单勾选
- 若有 `docs/roadmap/progressive-development-plan.md` 则更新对应条目

- [ ] **步骤 1：** 按 spec「验收清单」逐条勾选并注明证据（测试命令 / 手动）。
- [ ] **步骤 2：**

```bash
cd src-tauri && cargo test
npm run test
npm run typecheck
```

- [ ] **步骤 3：Commit**

```bash
git commit -m "docs: 同步弹窗双后端架构与验收状态"
```

**M4 / 全计划完成门禁：** spec 验收 1–7 可声明完成；无 .NET；Windows CI 绿。

---

## 自检（对照 spec）

| Spec 需求 | 任务 |
|-----------|------|
| 仅弹窗可选原生，设置/OCR/overlay 仍 WebView | 5, 10, 13 |
| `popupUiBackend` 默认 webview | 1, 11 |
| 重启切换 | 12, 13 |
| `PopupBackend` 边界 + ViewModel 共用 | 2, 3, 4, 5 |
| 同进程、无 .NET | 澄清 1、全程 |
| WinUI 失败降级 | 9 |
| 非 Windows 强制 webview | 3 `resolve_kind` |
| `windowPrecreate` 作用于当前 backend | 5 |
| CI Windows 可构建 | 6, 16 |
| 内存常驻对比 | 澄清 3、任务 16 |
| Runtime 引导 | 澄清 4、任务 9 |
| M1–M4 里程碑 | 任务映射表 |

**占位符扫描：** 无 TODO/TBD 步骤；spike 路径 A/B 有明确失败锁定规则。  
**类型一致性：** `PopupPositionMode` / `PopupViewModel` / `PopupUiBackendKind` / `popup_ui_backend` 字符串在各任务统一。

---

## 风险提醒（执行时）

1. **Tauri 与原生窗同进程消息循环**：WinUI/Win32 窗的 UI 线程与 WebView2 并存；show/hide 勿在全局快捷键同步栈里做重初始化（对齐现网 `show_popup` 线程策略）。
2. **首次创建死锁**：WebView 路径保持独立线程；原生路径避免在 tray 回调直接阻塞式 COM 初始化。
3. **双 UI 维护**：所有业务状态只经 `PopupViewModel`；禁止在 winui 复制 provider 逻辑。
4. **feature 关闭时的配置**：`winui` + 无 feature → 静默 webview + warn。
