# 独立设置页与翻译弹窗拆分 实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将单 `main` 窗口拆为「设置页（`main`，承载配置）+ 翻译弹窗（`translation-popup`，运行时按需唤起并跟随光标定位）」两个独立窗口，前端按窗口拆 `translate.*` / `settings.*` 两套，删除原 `index/main/style`。

**架构：** `tauri.conf.json` 只静态声明 `main`（设置页）。翻译弹窗与截图 overlay 都走运行时 `WebviewWindowBuilder`，由新增配置项 `popup_precreate` / `overlay_precreate` 决定「启动预创建隐藏」或「唤起时创建」，切换需重启。新增 `app/popup_window.rs` 封装弹窗双模式管理与光标定位纯函数；`web_popup.rs` 新增 `show_translation_popup`，快捷键/OCR/托盘触发翻译前主动唤起弹窗（修正现缺陷）。设置页与弹窗前端不互相通信，只通过 config 间接耦合。

**技术栈：** Tauri 2 + 原生静态前端（无构建）+ Rust edition 2021 + `windows` crate（`GetCursorPos`/`MonitorFromPoint`/`GetMonitorInfoW`，复用已启用 feature，不引入新 crate）。

**关联文档：**
- spec：[docs/superpowers/specs/2026-07-01-settings-translation-window-split-design.md](../specs/2026-07-01-settings-translation-window-split-design.md)
- 架构：[docs/architecture/ui-decoupling-proposal.md](../../architecture/ui-decoupling-proposal.md)

---

## 文件结构

**新增文件：**

| 文件 | 职责 |
|---|---|
| `src-tauri/src/app/popup_window.rs` | 翻译弹窗双模式窗口管理 + 光标定位纯函数 `compute_popup_position` + 逻辑坐标类型 |
| `src-tauri/src/platform/windows/cursor.rs` | Windows 光标位置 + 工作区获取（物理→逻辑换算） |
| `frontend/translate.html` / `translate.js` / `translate.css` | 翻译弹窗 DOM 与交互 |
| `frontend/settings.html` / `settings.js` / `settings.css` | 设置页 DOM 与交互 |

**修改文件：**

| 文件 | 动作 |
|---|---|
| `src-tauri/src/core/config/types.rs` | `AppConfig` 加 `popup_precreate`/`overlay_precreate`、`is_configured()`、`from_env`/`normalized` 补默认 |
| `src-tauri/src/app/mod.rs` | 声明 `pub mod popup_window;` |
| `src-tauri/src/app/window.rs` | 移除 `toggle_window`（双击托盘改 show） |
| `src-tauri/src/app/tray.rs` | 菜单加「翻译」「设置」项；双击改 `show_window` |
| `src-tauri/src/app/shortcuts.rs` | 触发翻译前调用 `show_translation_popup` |
| `src-tauri/src/ui/web_popup.rs` | 新增 `show_translation_popup`；`start_translation_from_input` 不再 show 主窗口；`show_translation_error` 改为唤起弹窗 |
| `src-tauri/src/ui/overlay.rs` | `open_overlay` 按 `overlay_precreate` 分支；抽出 `build_overlay`/`ensure_overlay` |
| `src-tauri/src/ui/ocr_popup.rs` | `start_translation_from_ocr` 传 config 给 `open_overlay` |
| `src-tauri/src/ui/config.rs` | 新增 `open_settings` command |
| `src-tauri/src/lib.rs` | 注册 `open_settings`；setup 阶段按配置预创建/显隐窗口 |
| `src-tauri/tauri.conf.json` | `main` 窗口 url 指向 `settings.html`、显式 `label` |
| `src-tauri/capabilities/default.json` | `windows` 加 `translation-popup` |
| `src-tauri/src/platform/windows/mod.rs` | 声明 `pub mod cursor;` 并 re-export `cursor_logical_context` |
| `src-tauri/src/platform/mod.rs` | re-export `cursor_logical_context` |
| `src-tauri/src/platform/unsupported.rs` | 新增 `cursor_logical_context` 返回 `None` |

**删除文件：**

| 文件 | 动作 |
|---|---|
| `frontend/index.html` / `main.js` / `style.css` | 拆分完成后删除 |

**任务分组：** 17 个任务，分 6 组：① 配置层（T1）② 弹窗定位与平台光标（T2-T4）③ 弹窗/overlay 窗口管理与编排（T5-T8）④ 托盘/设置 command/装配（T9-T12）⑤ Tauri 配置与权限（T13-T14）⑥ 前端拆分与清理（T15-T17），收尾验收（T18）。

**跨模块高风险点：**
- `start_translation_from_input` 移除 `show_window` 后，三个触发入口（Alt+T / Alt+O 的 submit / 托盘翻译）都必须显式调用 `show_translation_popup`，否则翻译在不可见弹窗中运行——三处同改，遗漏即缺陷。
- overlay 预创建模式用 `location.reload()` 复用持久窗口触发前端重新加载帧，需手动验证多轮 Alt+O 不残留旧帧。

---

## 任务 1：AppConfig 新增窗口策略字段与 is_configured()

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    #[test]
    fn app_config_defaults_precreate_window_strategies() {
        let config = AppConfig::from_env();
        assert!(config.popup_precreate, "popup_precreate 默认应为 true");
        assert!(config.overlay_precreate, "overlay_precreate 默认应为 true");
    }

    #[test]
    fn app_config_serializes_window_strategy_fields_camel_case() {
        let config = AppConfig::from_env();
        let json = serde_json::to_string(&config).expect("序列化");
        assert!(json.contains("\"popupPrecreate\":true"), "应输出 camelCase 字段 popupPrecreate: {json}");
        assert!(json.contains("\"overlayPrecreate\":true"), "应输出 camelCase 字段 overlayPrecreate: {json}");
    }

    #[test]
    fn app_config_deserializes_window_strategy_defaults_when_missing() {
        // 个人开发阶段：旧配置无这两个字段，反序列化应回落默认 true（不测新老兼容，仅测 serde(default)）。
        let json = r#"{
            "provider": "openai-compatible",
            "targetLang": "中文",
            "openaiCompatible": {
                "apiKey": "sk-x",
                "baseUrl": "https://api.openai.com/v1",
                "model": "gpt-4o-mini",
                "timeoutSeconds": 60
            }
        }"#;
        let config: AppConfig = serde_json::from_str::<AppConfig>(json)
            .expect("缺少窗口策略字段应可反序列化")
            .normalized();
        assert!(config.popup_precreate);
        assert!(config.overlay_precreate);
    }

    #[test]
    fn is_configured_true_when_openai_has_api_key() {
        let mut config = AppConfig::from_env();
        config.provider = "openai-compatible".to_string();
        config.openai_compatible.api_key = Some("sk-x".to_string());
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_false_when_openai_missing_api_key() {
        let mut config = AppConfig::from_env();
        config.provider = "openai-compatible".to_string();
        config.openai_compatible.api_key = None;
        assert!(!config.is_configured());
    }

    #[test]
    fn is_configured_true_when_claude_has_api_key() {
        let mut config = AppConfig::from_env();
        config.provider = "claude".to_string();
        config.claude.api_key = Some("sk-ant".to_string());
        assert!(config.is_configured());
    }

    #[test]
    fn is_configured_true_when_mock_provider() {
        let mut config = AppConfig::from_env();
        config.provider = "mock".to_string();
        assert!(config.is_configured(), "mock provider 无需 key 视为已配置");
    }
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib core::config::types::tests`
预期：编译失败，报 `no field popup_precreate on type AppConfig` / `method is_configured not found`。

- [ ] **步骤 3：编写最少实现代码**

修改 `AppConfig` 结构体与 impl（在 `types.rs` 中）：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub provider: String,
    pub target_lang: String,
    pub openai_compatible: OpenAiCompatibleAppConfig,
    #[serde(default)]
    pub claude: ClaudeAppConfig,
    #[serde(default = "default_true")]
    pub popup_precreate: bool,
    #[serde(default = "default_true")]
    pub overlay_precreate: bool,
}

fn default_true() -> bool {
    true
}
```

修改 `from_env`：

```rust
    pub fn from_env() -> Self {
        Self {
            provider: env::var("SHIZI_LLM_PROVIDER")
                .unwrap_or_else(|_| DEFAULT_PROVIDER.to_string()),
            target_lang: env::var("SHIZI_TARGET_LANG")
                .unwrap_or_else(|_| DEFAULT_TARGET_LANG.to_string()),
            openai_compatible: OpenAiCompatibleAppConfig::from_env(),
            claude: ClaudeAppConfig::from_env(),
            popup_precreate: true,
            overlay_precreate: true,
        }
        .normalized()
    }
```

修改 `normalized`：

```rust
    pub fn normalized(mut self) -> Self {
        self.provider = normalize_string(self.provider, DEFAULT_PROVIDER);
        self.target_lang = normalize_string(self.target_lang, DEFAULT_TARGET_LANG);
        self.openai_compatible = self.openai_compatible.normalized();
        self.claude = self.claude.normalized();
        self
    }

    /// 当前 active provider 是否具备所需凭证（用于启动时决定是否显示设置页引导）。
    /// mock 无需 key；claude/openai-compatible 需 api_key。
    pub fn is_configured(&self) -> bool {
        match self.provider.as_str() {
            "mock" => true,
            "claude" => self.claude.api_key.is_some(),
            _ => self.openai_compatible.api_key.is_some(),
        }
    }
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib core::config::types::tests`
预期：PASS（全部新增测试通过，原有测试不破坏）。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): 新增窗口策略字段与 is_configured"
```

---

## 任务 2：compute_popup_position 纯函数与逻辑坐标类型

**文件：**
- 创建：`src-tauri/src/app/popup_window.rs`（本任务只写类型 + 纯函数 + 测试，窗口管理函数在任务 4 补齐）
- 修改：`src-tauri/src/app/mod.rs`

- [ ] **步骤 1：编写失败的测试**

创建 `src-tauri/src/app/popup_window.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalPos {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work_area_1920x1080() -> LogicalRect {
        LogicalRect { x: 0.0, y: 0.0, width: 1920.0, height: 1080.0 }
    }

    fn popup_400x300() -> LogicalSize {
        LogicalSize { width: 400.0, height: 300.0 }
    }

    #[test]
    fn cursor_in_middle_keeps_position() {
        // 光标在屏中部：弹窗不溢出，定位 = 光标位置。
        let pos = compute_popup_position(
            LogicalPos { x: 800.0, y: 500.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 800.0, y: 500.0 });
    }

    #[test]
    fn cursor_near_right_shifts_left() {
        // 1800 + 400 = 2200 > 1920：右溢出，左移使右边界贴工作区右边。
        let pos = compute_popup_position(
            LogicalPos { x: 1800.0, y: 500.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 1520.0, y: 500.0 });
    }

    #[test]
    fn cursor_near_bottom_shifts_up() {
        // 950 + 300 = 1250 > 1080：下溢出，上移使底边贴工作区底边。
        let pos = compute_popup_position(
            LogicalPos { x: 800.0, y: 950.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 800.0, y: 780.0 });
    }

    #[test]
    fn cursor_at_corner_clamps_to_work_area_origin() {
        // 光标超出工作区左上：钳回工作区原点。
        let pos = compute_popup_position(
            LogicalPos { x: -100.0, y: -100.0 },
            popup_400x300(),
            work_area_1920x1080(),
        );
        assert_eq!(pos, LogicalPos { x: 0.0, y: 0.0 });
    }
}
```

在 `src-tauri/src/app/mod.rs` 末尾追加：

```rust
pub mod popup_window;
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cd src-tauri && cargo test --lib app::popup_window::tests`
预期：编译失败，报 `function compute_popup_position not found`。

- [ ] **步骤 3：编写最少实现代码**

在 `popup_window.rs` 的 `LogicalRect` 定义之后、`#[cfg(test)]` 之前插入纯函数：

```rust
/// 计算弹窗左上角逻辑坐标：默认放在光标处，若弹窗右/下溢出工作区则左/上移，
/// 最后钳制不低于工作区左上角。纯函数，便于单测。
pub fn compute_popup_position(
    cursor: LogicalPos,
    popup_size: LogicalSize,
    work_area: LogicalRect,
) -> LogicalPos {
    let mut x = cursor.x;
    let mut y = cursor.y;

    // 右溢出 → 左移，使右边界贴工作区右边。
    if x + popup_size.width > work_area.x + work_area.width {
        x = work_area.x + work_area.width - popup_size.width;
    }
    // 下溢出 → 上移，使底边贴工作区底边。
    if y + popup_size.height > work_area.y + work_area.height {
        y = work_area.y + work_area.height - popup_size.height;
    }
    // 不低于工作区左上。
    if x < work_area.x {
        x = work_area.x;
    }
    if y < work_area.y {
        y = work_area.y;
    }

    LogicalPos { x, y }
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cd src-tauri && cargo test --lib app::popup_window::tests`
预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/app/popup_window.rs src-tauri/src/app/mod.rs
git commit -m "feat(popup): 弹窗光标定位纯函数 compute_popup_position"
```

---

## 任务 3：平台光标获取（Windows + unsupported 兜底）

**文件：**
- 创建：`src-tauri/src/platform/windows/cursor.rs`
- 修改：`src-tauri/src/platform/windows/mod.rs`
- 修改：`src-tauri/src/platform/mod.rs`
- 修改：`src-tauri/src/platform/unsupported.rs`

- [ ] **步骤 1：编写 Windows 实现**

创建 `src-tauri/src/platform/windows/cursor.rs`：

```rust
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// 返回光标所在显示器工作区（逻辑像素）：
/// `(cursor_x, cursor_y, work_x, work_y, work_w, work_h)`，全为逻辑像素。
/// `scale` 用于物理→逻辑换算（MVP 取主窗口 scale，多屏精确缩放留后续）。
/// 任一 Win32 调用失败返回 `None`，由调用方退化为不定位。
pub fn cursor_logical_context(scale: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return None;
        }
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        let work = info.rcWork;
        let s = scale.max(0.0001);
        Some((
            cursor.x as f64 / s,
            cursor.y as f64 / s,
            work.left as f64 / s,
            work.top as f64 / s,
            (work.right - work.left) as f64 / s,
            (work.bottom - work.top) as f64 / s,
        ))
    }
}
```

- [ ] **步骤 2：编写 unsupported 兜底**

在 `src-tauri/src/platform/unsupported.rs` 末尾（`recognize_region` 之后、`#[cfg(test)]` 之前）追加：

```rust
/// 非 Windows 平台无法获取光标上下文，返回 `None`，调用方退化为不定位。
pub fn cursor_logical_context(_scale: f64) -> Option<(f64, f64, f64, f64, f64, f64)> {
    None
}
```

- [ ] **步骤 3：接线 re-export**

在 `src-tauri/src/platform/windows/mod.rs` 顶部 `pub mod capture;` / `pub mod ocr;` 旁追加，并在 `pub use` 区追加 re-export：

```rust
pub mod cursor;
```

并在该文件现有 `use`/`pub use` 之后追加：

```rust
pub use cursor::cursor_logical_context;
```

在 `src-tauri/src/platform/mod.rs` 把两处 `pub use` 各补一项：

```rust
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod unsupported;

#[cfg(target_os = "windows")]
pub use windows::{capture_screen, cursor_logical_context, recognize_region};

#[cfg(not(target_os = "windows"))]
pub use unsupported::{capture_screen, cursor_logical_context, recognize_region};
```

> 注意：`unsupported` 当前是单文件 `platform/unsupported.rs`，不是目录；`pub use unsupported::{...}` 已是现有写法，仅扩展列表。

- [ ] **步骤 4：验证编译**

运行：`cd src-tauri && cargo build`
预期：编译通过，无错误。`Win32_Graphics_Gdi` 与 `Win32_UI_WindowsAndMessaging` feature 已在 `Cargo.toml` 启用，无需修改依赖。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/platform/windows/cursor.rs src-tauri/src/platform/windows/mod.rs src-tauri/src/platform/mod.rs src-tauri/src/platform/unsupported.rs
git commit -m "feat(platform): 光标与工作区获取（Windows + 非 Windows 兜底）"
```

---

## 任务 4：popup_window.rs 窗口管理函数

**文件：**
- 修改：`src-tauri/src/app/popup_window.rs`

- [ ] **步骤 1：编写实现代码**

在 `popup_window.rs` 顶部（类型定义之前）追加 import，并在纯函数之后追加窗口管理函数。完整文件 import 区改为：

```rust
use tauri::{LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::core::config::AppConfig;
use crate::platform::cursor_logical_context;

pub const POPUP_LABEL: &str = "translation-popup";
```

（保留任务 2 已有的 `LogicalPos`/`LogicalSize`/`LogicalRect`/`compute_popup_position`/tests。）

在 `compute_popup_position` 之后追加：

```rust
/// 预创建模式下启动时调用：创建并隐藏翻译弹窗。运行时模式无操作。
/// 已存在则跳过（幂等）。
pub fn ensure_popup_window(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    if !config.popup_precreate {
        return Ok(());
    }
    if app.get_webview_window(POPUP_LABEL).is_some() {
        return Ok(());
    }
    build_popup(app)?;
    Ok(())
}

/// 唤起弹窗：预创建模式 show + 定位；运行时模式创建 + 定位。
/// 光标上下文不可用时退化为不重新定位（保留上一次位置或默认）。
pub fn show_popup(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    let window = if config.popup_precreate {
        app.get_webview_window(POPUP_LABEL)
            .ok_or_else(|| "翻译弹窗未预创建".to_string())?
    } else {
        if let Some(existing) = app.get_webview_window(POPUP_LABEL) {
            let _ = existing.close();
        }
        build_popup(app)?
    };

    let scale = app
        .get_webview_window("main")
        .and_then(|w| w.scale_factor().ok())
        .unwrap_or(1.0);

    if let Some((cx, cy, wx, wy, ww, wh)) = cursor_logical_context(scale) {
        let outer = window
            .outer_size()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(scale);
        let pos = compute_popup_position(
            LogicalPos { x: cx, y: cy },
            LogicalSize { width: outer.width, height: outer.height },
            LogicalRect { x: wx, y: wy, width: ww, height: wh },
        );
        window
            .set_position(LogicalPosition::new(pos.x, pos.y))
            .map_err(|e| e.to_string())?;
    }

    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// 预创建模式 hide / 运行时模式 close。供后续清理路径使用。
pub fn hide_or_close_popup(app: &tauri::AppHandle, config: &AppConfig) {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    if config.popup_precreate {
        let _ = window.hide();
    } else {
        let _ = window.close();
    }
}

/// 预创建模式下挂载 CloseRequested：prevent_close + hide。
/// 运行时模式不挂载（放行 close 销毁），故本函数仅在 precreate 时生效。
pub fn setup_popup_close_event(app: &tauri::AppHandle, config: &AppConfig) {
    if !config.popup_precreate {
        return;
    }
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    let window_to_hide = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_to_hide.hide();
        }
    });
}

fn build_popup(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    WebviewWindowBuilder::new(app, POPUP_LABEL, WebviewUrl::App("translate.html".into()))
        .title("Shizi 翻译")
        .inner_size(480.0, 360.0)
        .resizable(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())
}
```

- [ ] **步骤 2：验证编译与测试**

运行：`cd src-tauri && cargo build && cargo test --lib app::popup_window::tests`
预期：编译通过；纯函数测试仍 PASS。窗口管理函数无单测（依赖 Tauri 运行时，靠手动验证）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/app/popup_window.rs
git commit -m "feat(popup): 弹窗双模式窗口管理 ensure/show/hide_or_close/close_event"
```

---

## 任务 5：web_popup.rs 新增 show_translation_popup 并修正 show 行为

**文件：**
- 修改：`src-tauri/src/ui/web_popup.rs`

- [ ] **步骤 1：修改 import 区**

将 `web_popup.rs` 顶部 use 块中的：

```rust
use crate::{
    app::{state::AppState, window::show_window},
    core::{
```

改为：

```rust
use tauri::Manager;

use crate::{
    app::{popup_window, state::AppState},
    core::{
        config::AppConfig,
```

（移除 `window::show_window`，新增 `Manager`、`popup_window`、`AppConfig`。）

- [ ] **步骤 2：新增 show_translation_popup**

在 `emit_translation_event` 函数之后插入：

```rust
/// 唤起翻译弹窗（show + 光标定位）。触发翻译前调用，修正旧版依赖窗口已可见的缺陷。
pub fn show_translation_popup(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    popup_window::show_popup(app, config)
}
```

- [ ] **步骤 3：移除 start_translation_from_input 中的 show_window**

在 `start_translation_from_input` 中删除下面这一行（保留其后的 `thread::sleep` 与 `emit_translation_event`）：

```rust
    show_window(&app);
```

> 说明：sleep(120ms) 保留，用于运行时模式下待 webview 就绪再 emit `Started`；预创建模式下 webview 已加载，sleep 无害。弹窗的 show 由各触发入口显式调用 `show_translation_popup` 完成（任务 6/8/10）。

- [ ] **步骤 4：show_translation_error 改为唤起弹窗**

将 `show_translation_error` 整体替换为：

```rust
pub fn show_translation_error(app: &tauri::AppHandle, message: impl Into<String>) {
    let session_id = create_session_id().unwrap_or_else(|_| "selection-error".to_string());
    let config = app
        .state::<AppState>()
        .config_store
        .get()
        .ok();
    if let Some(config) = config {
        let _ = show_translation_popup(app, &config);
    }
    let _ = emit_translation_event(
        app,
        TranslationEvent::Failed {
            session_id: TranslationSessionId(session_id),
            message: message.into(),
            retryable: false,
        },
    );
}
```

- [ ] **步骤 5：验证编译与测试**

运行：`cd src-tauri && cargo build && cargo test --lib ui::web_popup::tests`
预期：编译通过；现有 `cache_automatic_source_text` 测试仍 PASS。

- [ ] **步骤 6：Commit**

```bash
git add src-tauri/src/ui/web_popup.rs
git commit -m "refactor(web_popup): 新增 show_translation_popup 并移除主窗口 show"
```

---

## 任务 6：快捷键触发翻译前唤起弹窗

**文件：**
- 修改：`src-tauri/src/app/shortcuts.rs`

- [ ] **步骤 1：修改 import 区**

将 `shortcuts.rs` 顶部 use 块中的：

```rust
use crate::{
    app::state::AppState,
    core::{selection::copy_selected_text, translation::TranslationInput},
    ui::{
        ocr_popup::start_translation_from_ocr,
        web_popup::{show_translation_error, start_translation_from_input},
    },
};
```

改为：

```rust
use crate::{
    app::state::AppState,
    core::{selection::copy_selected_text, translation::TranslationInput},
    ui::{
        ocr_popup::start_translation_from_ocr,
        web_popup::{show_translation_error, show_translation_popup, start_translation_from_input},
    },
};
```

- [ ] **步骤 2：handle_selection_translate 中先唤起弹窗**

将 `handle_selection_translate` 中成功读到选区、`set_pending_source_text` 之后、`start_translation_from_input` 之前，插入唤起弹窗。把：

```rust
        let state: State<'_, AppState> = app_handle.state();
        if let Err(error) = state.set_pending_source_text(selected_text.clone()) {
            show_translation_error(&app_handle, error);
            return;
        }

        if let Err(error) = start_translation_from_input(
            TranslationInput::SelectedText(selected_text),
            app_handle.clone(),
            state.inner(),
        ) {
            show_translation_error(&app_handle, error);
        }
```

改为：

```rust
        let state: State<'_, AppState> = app_handle.state();
        if let Err(error) = state.set_pending_source_text(selected_text.clone()) {
            show_translation_error(&app_handle, error);
            return;
        }

        let config = state.config_store.get();
        if let Ok(config) = &config {
            if let Err(error) = show_translation_popup(&app_handle, config) {
                show_translation_error(&app_handle, error);
                return;
            }
        }

        if let Err(error) = start_translation_from_input(
            TranslationInput::SelectedText(selected_text),
            app_handle.clone(),
            state.inner(),
        ) {
            show_translation_error(&app_handle, error);
        }
```

> 说明：OCR 路径（`Alt+O`）的弹窗唤起放在 `submit_capture_region`（任务 8），因为弹窗应在 OCR 完成后、翻译开始前出现，而非截图开始时。

- [ ] **步骤 3：验证编译与测试**

运行：`cd src-tauri && cargo build && cargo test --lib app::shortcuts::tests`
预期：编译通过；`classify_alt_o_as_ocr` / `classify_alt_t_as_selection` 仍 PASS。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/shortcuts.rs
git commit -m "fix(shortcuts): 划词翻译前主动唤起弹窗"
```

---

## 任务 7：overlay 双模式窗口创建策略

**文件：**
- 修改：`src-tauri/src/ui/overlay.rs`

- [ ] **步骤 1：抽出 build_overlay 并改造 open_overlay**

将 `overlay.rs` 顶部 import 区的：

```rust
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::{
    app::state::AppState,
    core::ocr::OcrHints,
    platform::recognize_region,
    ui::web_popup::{show_translation_error, start_translation_from_input},
};
```

改为（新增 `AppConfig`）：

```rust
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::{
    app::state::AppState,
    core::{config::AppConfig, ocr::OcrHints},
    platform::recognize_region,
    ui::web_popup::{show_translation_error, show_translation_popup, start_translation_from_input},
};
```

将现有 `open_overlay` 整体替换为下面的 `build_overlay` + `ensure_overlay` + `open_overlay` 三函数：

```rust
/// 构建隐藏的 overlay 窗口并挂载 Destroyed 兜底（释放 pending_capture 帧与 capture 锁）。
fn build_overlay(app: &tauri::AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(app, OVERLAY_LABEL, WebviewUrl::App("overlay.html".into()))
        .title("Shizi 截图")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .fullscreen(true)
        // 创建时不可见：WebView2 加载 HTML + canvas putImageData 期间会显示默认白底，
        // 由前端在内容就绪后 invoke('show_overlay') 让后端显示，消除占位闪烁。
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    // 兜底：overlay 被外部关闭或异常销毁时（非 submit/cancel 正常路径），
    // 释放 pending_capture 帧与 capture 锁，避免锁永久占用导致后续 Alt+O 被拒。
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Destroyed = event {
            let state: tauri::State<'_, AppState> = app_handle.state();
            let _ = state.take_pending_capture();
            let _ = state.finish_capture();
        }
    });
    Ok(window)
}

/// 预创建模式下启动时调用：创建并隐藏 overlay。已存在则跳过（幂等）。
pub fn ensure_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window(OVERLAY_LABEL).is_some() {
        return Ok(());
    }
    build_overlay(app)?;
    Ok(())
}

/// 在光标所在显示器上铺满 overlay 窗口。整屏帧须已存入 AppState。
/// 预创建模式：窗口已存在，reload 触发前端重新加载帧（窗口保持隐藏，前端就绪后 invoke show_overlay）。
/// 运行时模式：关旧建新（当前帧由新窗口前端加载）。
pub fn open_overlay(app: &tauri::AppHandle, config: &AppConfig) -> Result<(), String> {
    if config.overlay_precreate {
        if let Some(window) = app.get_webview_window(OVERLAY_LABEL) {
            window.eval("location.reload()").map_err(|e| e.to_string())?;
        } else {
            build_overlay(app)?;
        }
    } else {
        if let Some(existing) = app.get_webview_window(OVERLAY_LABEL) {
            let _ = existing.close();
        }
        build_overlay(app)?;
    }
    Ok(())
}
```

> 说明：overlay 关闭语义（submit/cancel 用 `hide_overlay` 保留 hwnd 以避 PostMessage 失效）在两种模式下都不变——spec 明确「不改变 overlay 的框选/OCR 业务逻辑，仅改其窗口创建策略」。预创建模式用 `location.reload()` 复用持久窗口重新触发前端 load→渲染→`show_overlay` 流程，与现有前端完全兼容。

- [ ] **步骤 2：submit_capture_region 中先唤起弹窗**

将 `submit_capture_region` 的 `match result` 块中 `Ok(Some(input))` 分支：

```rust
        Ok(Some(input)) => {
            if let Err(error) = start_translation_from_input(input, app.clone(), app_state) {
                show_translation_error(&app, error);
            }
        }
```

改为（翻译前唤起弹窗）：

```rust
        Ok(Some(input)) => {
            let config = app_state.config_store.get();
            if let Ok(config) = &config {
                if let Err(error) = show_translation_popup(&app, config) {
                    show_translation_error(&app, error);
                }
            }
            if let Err(error) = start_translation_from_input(input, app.clone(), app_state) {
                show_translation_error(&app, error);
            }
        }
```

- [ ] **步骤 3：验证编译**

运行：`cd src-tauri && cargo build`
预期：编译通过（`hide_overlay` / `show_overlay` / `cancel_capture` 等保持不变）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/ui/overlay.rs
git commit -m "refactor(overlay): 按 overlay_precreate 分支化窗口创建策略"
```

---

## 任务 8：ocr_popup 传 config 给 open_overlay

**文件：**
- 修改：`src-tauri/src/ui/ocr_popup.rs`

- [ ] **步骤 1：传 config 给 open_overlay**

将 `start_translation_from_ocr` 中：

```rust
    // overlay 自身承载交互，不需要主窗口可见。成功打开后保留 capture 锁，等 submit/cancel 释放。
    if let Err(error) = open_overlay(&app) {
        let _ = state.take_pending_capture();
        let _ = state.finish_capture();
        show_translation_error(&app, format!("无法打开截图窗口：{error}"));
    }
```

改为：

```rust
    // overlay 自身承载交互，不需要主窗口可见。成功打开后保留 capture 锁，等 submit/cancel 释放。
    let config = state.config_store.get();
    let overlay_result = match &config {
        Ok(config) => open_overlay(&app, config),
        Err(error) => Err(error.to_string()),
    };
    if let Err(error) = overlay_result {
        let _ = state.take_pending_capture();
        let _ = state.finish_capture();
        show_translation_error(&app, format!("无法打开截图窗口：{error}"));
    }
```

- [ ] **步骤 2：验证编译**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ui/ocr_popup.rs
git commit -m "refactor(ocr_popup): open_overlay 传入 config 决定创建策略"
```

---

## 任务 9：window.rs 移除 toggle_window

**文件：**
- 修改：`src-tauri/src/app/window.rs`

- [ ] **步骤 1：移除 toggle_window**

将 `window.rs` 整体替换为：

```rust
use tauri::Manager;

pub fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn setup_close_to_hide(app: &tauri::App) {
    if let Some(window) = app.get_webview_window("main") {
        let window_to_hide = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_to_hide.hide();
            }
        });
    }
}
```

> 说明：双击托盘改用 `show_window`（任务 10），`toggle_window` 不再有调用方，移除以避免 dead code。

- [ ] **步骤 2：验证编译**

运行：`cd src-tauri && cargo build`
预期：编译通过（此时 `tray.rs` 仍引用 `toggle_window`，会在任务 10 修复；若此处报错可先继续到任务 10 一并修复——但为保持每步可编译，先在步骤 1 后临时把 tray 的引用改掉或先做任务 10。推荐顺序：先做任务 10 再 commit 任务 9，或合并为一个 commit）。

> 为保持原子可编译，本任务与任务 10 合并提交：完成步骤 1 后立即执行任务 10，再一起编译与 commit。

- [ ] **步骤 3：Commit**（与任务 10 合并）

见任务 10 步骤 3。

---

## 任务 10：tray.rs 新增「翻译」「设置」菜单项

**文件：**
- 修改：`src-tauri/src/app/tray.rs`

- [ ] **步骤 1：重写 tray.rs**

将 `tray.rs` 整体替换为：

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager,
};

use crate::{
    app::state::AppState,
    app::window::show_window,
    ui::web_popup::show_translation_popup,
};

pub fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let translate_item = MenuItem::with_id(app, "translate", "翻译", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&translate_item, &settings_item, &quit_item])?;

    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("Shizi - 翻译助手")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "translate" => {
                let config = app
                    .state::<AppState>()
                    .config_store
                    .get()
                    .map_err(|e| e.to_string());
                if let Ok(config) = config {
                    let _ = show_translation_popup(app, &config);
                }
            }
            "settings" => show_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}
```

- [ ] **步骤 2：验证编译（任务 9 + 10 合并）**

运行：`cd src-tauri && cargo build`
预期：编译通过，无 `toggle_window` 未找到错误，无 dead_code 警告。

- [ ] **步骤 3：Commit（任务 9 + 10 合并）**

```bash
git add src-tauri/src/app/window.rs src-tauri/src/app/tray.rs
git commit -m "feat(tray): 新增翻译/设置菜单项并移除 toggle_window"
```

---

## 任务 11：config.rs 新增 open_settings command

**文件：**
- 修改：`src-tauri/src/ui/config.rs`

- [ ] **步骤 1：新增 open_settings**

将 `config.rs` 整体替换为：

```rust
use crate::{
    app::state::AppState,
    app::window::show_window,
    core::config::AppConfig,
};

#[tauri::command]
pub async fn get_app_config(state: tauri::State<'_, AppState>) -> Result<AppConfig, String> {
    state.config_store.get().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn save_app_config(
    config: AppConfig,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, String> {
    state
        .config_store
        .save(config)
        .map_err(|error| error.to_string())
}

/// 翻译弹窗「设置」按钮调用：显示并聚焦设置页主窗口（不新建窗口）。
#[tauri::command]
pub async fn open_settings(app: tauri::AppHandle) -> Result<(), String> {
    show_window(&app);
    Ok(())
}
```

- [ ] **步骤 2：验证编译**

运行：`cd src-tauri && cargo build`
预期：编译通过（`open_settings` 尚未注册，运行时不可用，任务 12 注册）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/ui/config.rs
git commit -m "feat(config): 新增 open_settings command"
```

---

## 任务 12：lib.rs 装配——注册命令、setup 预创建与主窗口显隐

**文件：**
- 修改：`src-tauri/src/lib.rs`

- [ ] **步骤 1：重写 lib.rs**

将 `lib.rs` 整体替换为：

```rust
mod app;
mod core;
mod platform;
mod ui;

use app::{
    popup_window::{ensure_popup_window, setup_popup_close_event},
    shortcuts::{handle_global_shortcut, register_global_shortcuts},
    state::AppState,
    tray::setup_tray,
    window::setup_close_to_hide,
};
use core::config::ConfigStore;
use tauri::Manager;
use ui::{
    config::{get_app_config, open_settings, save_app_config},
    overlay::{ensure_overlay, open_overlay_cancel_capture_unused_placeholder, show_overlay,
              cancel_capture, get_capture_frame_bytes, get_capture_frame_meta,
              submit_capture_region},
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};
```

> **修正（占位行需删除）：** 上面 `overlay::` 的 import 列表里不要保留 `open_overlay_cancel_capture_unused_placeholder` 这个占位名——实际只需导入真正用到的命令。正确写法见下方完整 import 块，以「实际用到的符号」为准：

```rust
use ui::{
    config::{get_app_config, open_settings, save_app_config},
    overlay::{
        cancel_capture, ensure_overlay, get_capture_frame_bytes, get_capture_frame_meta,
        show_overlay, submit_capture_region,
    },
    web_popup::{
        cancel_translation, retry_translation, start_translation, take_pending_source_text,
    },
};
```

继续函数体：

```rust
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    handle_global_shortcut(app, shortcut, event);
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            start_translation,
            cancel_translation,
            retry_translation,
            take_pending_source_text,
            get_app_config,
            save_app_config,
            open_settings,
            get_capture_frame_meta,
            get_capture_frame_bytes,
            submit_capture_region,
            cancel_capture,
            show_overlay,
        ])
        .setup(|app| {
            let config_store = ConfigStore::load(app.handle())
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            let config = config_store
                .get()
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            app.manage(AppState::new(config_store));

            setup_tray(app)?;
            setup_close_to_hide(app);

            // 设置页主窗口显隐：已配置 provider 则隐藏驻留托盘，否则显示引导配置。
            if let Some(main_window) = app.get_webview_window("main") {
                if config.is_configured() {
                    let _ = main_window.hide();
                } else {
                    let _ = main_window.show();
                    let _ = main_window.set_focus();
                }
            }

            // 翻译弹窗预创建（按配置）+ 关闭事件挂载。
            ensure_popup_window(app.handle(), &config)
                .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            setup_popup_close_event(app.handle(), &config);

            // 截图 overlay 预创建（按配置）。
            if config.overlay_precreate {
                ensure_overlay(app.handle())
                    .map_err(|error| tauri::Error::Anyhow(error.into()))?;
            }

            register_global_shortcuts(app).map_err(|error| tauri::Error::Anyhow(error.into()))?;

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
```

> 注意：`overlay::open_overlay` 不在 `lib.rs` 使用（它由 `ocr_popup` 调用），故不在此 import；`ocr_popup` 自己 `use crate::ui::overlay::open_overlay`。`show_overlay` 是 command，需注册并 import。

- [ ] **步骤 2：验证编译与测试**

运行：`cd src-tauri && cargo build && cargo test`
预期：编译通过；全部既有测试 PASS。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(app): 装配弹窗/overlay 预创建与主窗口显隐"
```

---

## 任务 13：tauri.conf.json 主窗口指向 settings.html

**文件：**
- 修改：`src-tauri/tauri.conf.json`

- [ ] **步骤 1：修改主窗口配置**

将 `tauri.conf.json` 的 `app.windows` 数组改为：

```json
    "windows": [
      {
        "label": "main",
        "title": "Shizi - 设置",
        "url": "settings.html",
        "width": 480,
        "height": 520,
        "resizable": true,
        "center": true
      }
    ]
```

> 说明：显式 `label: "main"`（原本默认 label 即 main，显式更清晰）；`url` 指向 `settings.html`；高度调为 520 以容纳新增「窗口策略」分组。翻译弹窗与 overlay 不在此声明，均运行时创建。

- [ ] **步骤 2：验证构建**

运行：`cd src-tauri && cargo build`
预期：编译通过（`tauri::generate_context!` 校验配置）。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "chore(tauri): 主窗口指向 settings.html"
```

---

## 任务 14：capabilities 授权 translation-popup

**文件：**
- 修改：`src-tauri/capabilities/default.json`

- [ ] **步骤 1：加入 translation-popup**

将 `default.json` 的 `windows` 数组改为：

```json
  "windows": ["main", "translation-popup", "screenshot-overlay"],
```

- [ ] **步骤 2：验证构建**

运行：`cd src-tauri && cargo build`
预期：编译通过。

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/capabilities/default.json
git commit -m "chore(capabilities): 授权 translation-popup 窗口"
```

---

## 任务 15：前端设置页 settings.html / settings.js / settings.css

**文件：**
- 创建：`frontend/settings.html`
- 创建：`frontend/settings.js`
- 创建：`frontend/settings.css`

- [ ] **步骤 1：创建 settings.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Shizi - 设置</title>
  <link rel="stylesheet" href="settings.css">
  <script type="module" src="settings.js" defer></script>
</head>
<body>
  <div class="container">
    <div class="header">
      <h1>Shizi 设置</h1>
      <p class="subtitle">Provider 与窗口策略配置</p>
    </div>
    <div class="content">
      <label>
        目标语言
        <input id="targetLangInput" type="text" placeholder="中文">
      </label>
      <label>
        Provider
        <select id="providerSelect">
          <option value="openai-compatible">OpenAI Compatible</option>
          <option value="claude">Claude</option>
          <option value="mock">Mock（调试用）</option>
        </select>
      </label>

      <div id="openaiSettings">
        <label>
          API Key
          <input id="apiKeyInput" type="password" placeholder="sk-...">
        </label>
        <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
        <label>
          Base URL
          <input id="baseUrlInput" type="text" placeholder="https://api.openai.com/v1">
        </label>
        <label>
          Model
          <input id="modelInput" type="text" placeholder="gpt-4o-mini">
        </label>
        <label>
          Timeout 秒
          <input id="timeoutInput" type="number" min="1" step="1" placeholder="60">
        </label>
      </div>

      <div id="claudeSettings" class="hidden">
        <label>
          API Key
          <input id="claudeApiKeyInput" type="password" placeholder="sk-ant-...">
        </label>
        <p class="config-warning">API Key 本阶段会明文保存到本机配置文件，请只在可信设备上使用。</p>
        <label>
          Base URL
          <input id="claudeBaseUrlInput" type="text" placeholder="https://api.anthropic.com">
        </label>
        <label>
          Model
          <input id="claudeModelInput" type="text" placeholder="claude-haiku-4-5">
        </label>
        <label>
          Timeout 秒
          <input id="claudeTimeoutInput" type="number" min="1" step="1" placeholder="60">
        </label>
        <label>
          <input id="claudeEnableThinkingInput" type="checkbox">
          Enable Thinking（仅对支持的模型生效，Haiku 需关闭）
        </label>
      </div>

      <fieldset class="window-strategy">
        <legend>窗口策略</legend>
        <label>
          <input id="popupPrecreateInput" type="checkbox">
          翻译弹窗预创建（关闭后需重启生效）
        </label>
        <label>
          <input id="overlayPrecreateInput" type="checkbox">
          截图 overlay 预创建（关闭后需重启生效）
        </label>
      </fieldset>

      <button id="saveConfigBtn">保存配置</button>
      <div id="configStatus" class="config-status"></div>
    </div>
    <div class="footer">
      <span>快捷键：Alt+T 划词翻译 · Alt+O 截图 OCR</span>
    </div>
  </div>
</body>
</html>
```

- [ ] **步骤 2：创建 settings.js**

```js
const targetLangInput = document.getElementById('targetLangInput');
const apiKeyInput = document.getElementById('apiKeyInput');
const baseUrlInput = document.getElementById('baseUrlInput');
const modelInput = document.getElementById('modelInput');
const timeoutInput = document.getElementById('timeoutInput');
const saveConfigBtn = document.getElementById('saveConfigBtn');
const configStatus = document.getElementById('configStatus');
const providerSelect = document.getElementById('providerSelect');
const openaiSettings = document.getElementById('openaiSettings');
const claudeSettings = document.getElementById('claudeSettings');
const claudeApiKeyInput = document.getElementById('claudeApiKeyInput');
const claudeBaseUrlInput = document.getElementById('claudeBaseUrlInput');
const claudeModelInput = document.getElementById('claudeModelInput');
const claudeTimeoutInput = document.getElementById('claudeTimeoutInput');
const claudeEnableThinkingInput = document.getElementById('claudeEnableThinkingInput');
const popupPrecreateInput = document.getElementById('popupPrecreateInput');
const overlayPrecreateInput = document.getElementById('overlayPrecreateInput');

const invoke = window.__TAURI__?.core?.invoke;

// 记录上次保存的策略值，用于检测切换并提示重启。
let lastPopupPrecreate = true;
let lastOverlayPrecreate = true;

function setConfigStatus(message, isError = false) {
  configStatus.textContent = message;
  configStatus.style.color = isError ? '#b42318' : '#666';
}

function toggleProviderSettings() {
  const provider = providerSelect.value;
  openaiSettings.classList.toggle('hidden', provider !== 'openai-compatible');
  claudeSettings.classList.toggle('hidden', provider !== 'claude');
}

function fillConfigForm(config) {
  targetLangInput.value = config.targetLang ?? '中文';
  providerSelect.value = config.provider ?? 'openai-compatible';
  apiKeyInput.value = config.openaiCompatible?.apiKey ?? '';
  baseUrlInput.value = config.openaiCompatible?.baseUrl ?? 'https://api.openai.com/v1';
  modelInput.value = config.openaiCompatible?.model ?? 'gpt-4o-mini';
  timeoutInput.value = String(config.openaiCompatible?.timeoutSeconds ?? 60);
  claudeApiKeyInput.value = config.claude?.apiKey ?? '';
  claudeBaseUrlInput.value = config.claude?.baseUrl ?? 'https://api.anthropic.com';
  claudeModelInput.value = config.claude?.model ?? 'claude-haiku-4-5';
  claudeTimeoutInput.value = String(config.claude?.timeoutSeconds ?? 60);
  claudeEnableThinkingInput.checked = config.claude?.enableThinking ?? false;
  popupPrecreateInput.checked = config.popupPrecreate ?? true;
  overlayPrecreateInput.checked = config.overlayPrecreate ?? true;
  lastPopupPrecreate = popupPrecreateInput.checked;
  lastOverlayPrecreate = overlayPrecreateInput.checked;
  toggleProviderSettings();
}

function readConfigForm() {
  return {
    provider: providerSelect.value,
    targetLang: targetLangInput.value.trim() || '中文',
    openaiCompatible: {
      apiKey: apiKeyInput.value.trim() || null,
      baseUrl: baseUrlInput.value.trim(),
      model: modelInput.value.trim(),
      timeoutSeconds: Number(timeoutInput.value),
    },
    claude: {
      apiKey: claudeApiKeyInput.value.trim() || null,
      baseUrl: claudeBaseUrlInput.value.trim(),
      model: claudeModelInput.value.trim(),
      timeoutSeconds: Number(claudeTimeoutInput.value),
      enableThinking: claudeEnableThinkingInput.checked,
    },
    popupPrecreate: popupPrecreateInput.checked,
    overlayPrecreate: overlayPrecreateInput.checked,
  };
}

function validateConfig(config) {
  if (config.provider === 'mock') return null;
  const sections = config.provider === 'claude' ? [config.claude] : [config.openaiCompatible];
  for (const section of sections) {
    let url;
    try {
      url = new URL(section.baseUrl);
    } catch {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return 'Base URL 请输入有效的 http(s) 地址';
    }
    if (!section.model) {
      return 'Model 不能为空';
    }
    if (!Number.isInteger(section.timeoutSeconds)
        || section.timeoutSeconds < 1
        || section.timeoutSeconds > 600) {
      return 'Timeout 秒请输入 1-600 的整数';
    }
  }
  return null;
}

async function loadAppConfig() {
  if (!invoke) {
    setConfigStatus('Tauri API 未就绪，无法读取配置', true);
    return;
  }
  try {
    const config = await invoke('get_app_config');
    fillConfigForm(config);
    setConfigStatus('');
  } catch (error) {
    setConfigStatus(String(error), true);
  }
}

async function saveAppConfig() {
  if (!invoke) {
    setConfigStatus('Tauri API 未就绪，无法保存配置', true);
    return;
  }
  const configToSave = readConfigForm();
  const validationError = validateConfig(configToSave);
  if (validationError) {
    setConfigStatus(validationError, true);
    return;
  }

  const strategyChanged =
    configToSave.popupPrecreate !== lastPopupPrecreate
    || configToSave.overlayPrecreate !== lastOverlayPrecreate;

  saveConfigBtn.disabled = true;
  saveConfigBtn.textContent = '保存中...';
  setConfigStatus('保存中...');

  try {
    const config = await invoke('save_app_config', { config: configToSave });
    fillConfigForm(config);
    setConfigStatus(
      strategyChanged
        ? '配置已保存，窗口策略切换需重启应用生效'
        : '配置已保存，下一次翻译生效'
    );
  } catch (error) {
    setConfigStatus(String(error), true);
  } finally {
    saveConfigBtn.disabled = false;
    saveConfigBtn.textContent = '保存配置';
  }
}

providerSelect.addEventListener('change', toggleProviderSettings);
saveConfigBtn.addEventListener('click', saveAppConfig);

loadAppConfig();
```

- [ ] **步骤 3：创建 settings.css**

```css
* { margin: 0; padding: 0; box-sizing: border-box; }
[hidden] { display: none !important; }

body {
  font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
  background: #f5f5f5;
  color: #333;
  height: 100vh;
  overflow: hidden;
  user-select: none;
}

.container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 16px;
}

.header { text-align: center; margin-bottom: 12px; }
.header h1 { font-size: 22px; font-weight: 700; color: #1a1a1a; }
.subtitle { font-size: 12px; color: #888; margin-top: 2px; }

.content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
}

.content > label {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 12px;
  color: #555;
}

.content input {
  padding: 7px 8px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font: inherit;
  outline: none;
}
.content input:focus { border-color: #4a90d9; }

.content select {
  padding: 7px 8px;
  border: 1px solid #ddd;
  border-radius: 6px;
  font: inherit;
  outline: none;
  background: #fff;
  cursor: pointer;
}
.content select:focus { border-color: #4a90d9; }

.content label:has(input[type="checkbox"]) {
  flex-direction: row;
  align-items: center;
}

.config-warning { font-size: 11px; color: #b54708; }
.config-status { min-height: 16px; font-size: 12px; color: #666; }

.window-strategy {
  border: 1px solid #ddd;
  border-radius: 8px;
  background: #fff;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.window-strategy legend {
  font-size: 12px;
  color: #555;
  padding: 0 4px;
}
.window-strategy label {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  color: #555;
}

#saveConfigBtn {
  padding: 8px;
  border: none;
  border-radius: 6px;
  background: #4a90d9;
  color: #fff;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
}
#saveConfigBtn:hover:not(:disabled) { opacity: 0.85; }
#saveConfigBtn:disabled { cursor: not-allowed; opacity: 0.55; }

.footer { text-align: center; font-size: 11px; color: #aaa; margin-top: 8px; }
```

- [ ] **步骤 4：语法检查**

运行：`node --check frontend/settings.js`
预期：无输出（语法正确）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/settings.html frontend/settings.js frontend/settings.css
git commit -m "feat(frontend): 独立设置页 settings"
```

---

## 任务 16：前端翻译弹窗 translate.html / translate.js / translate.css

**文件：**
- 创建：`frontend/translate.html`
- 创建：`frontend/translate.js`
- 创建：`frontend/translate.css`

- [ ] **步骤 1：创建 translate.html**

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Shizi 翻译</title>
  <link rel="stylesheet" href="translate.css">
  <script type="module" src="translate.js" defer></script>
</head>
<body>
  <div class="container">
    <div class="input-area">
      <textarea id="inputText" placeholder="输入要翻译的文本..." rows="3"></textarea>
    </div>
    <div class="action-bar">
      <button id="translateBtn">翻译</button>
      <button id="cancelBtn" hidden>取消</button>
      <button id="retryBtn" hidden>重试</button>
      <button id="settingsBtn">设置</button>
      <button id="clearBtn">清空</button>
    </div>
    <div class="output-area">
      <div id="sourceBadge" class="source-badge hidden"></div>
      <div id="outputText" class="output-box">翻译结果将显示在这里</div>
    </div>
  </div>
</body>
</html>
```

- [ ] **步骤 2：创建 translate.js**

```js
const inputText = document.getElementById('inputText');
const outputText = document.getElementById('outputText');
const sourceBadge = document.getElementById('sourceBadge');
const translateBtn = document.getElementById('translateBtn');
const settingsBtn = document.getElementById('settingsBtn');
const clearBtn = document.getElementById('clearBtn');
const cancelBtn = document.getElementById('cancelBtn');
const retryBtn = document.getElementById('retryBtn');

const tauriApi = window.__TAURI__;
const invoke = tauriApi?.core?.invoke;
const listen = tauriApi?.event?.listen;

let isTranslating = false;
let currentSessionId = null;

function resetOutput() {
  outputText.textContent = '翻译结果将显示在这里';
  outputText.style.color = '#999';
}

function setSourceBadge(sourceType) {
  switch (sourceType) {
    case 'selectedText':
      sourceBadge.textContent = '来自划词';
      sourceBadge.classList.remove('hidden');
      break;
    case 'ocrText':
      sourceBadge.textContent = '来自 OCR';
      sourceBadge.classList.remove('hidden');
      break;
    default:
      sourceBadge.classList.add('hidden');
      sourceBadge.textContent = '';
      break;
  }
}

function hideSourceBadge() {
  sourceBadge.classList.add('hidden');
  sourceBadge.textContent = '';
}

function setActionButtons({ translating, canRetry }) {
  isTranslating = translating;
  translateBtn.disabled = translating;
  clearBtn.disabled = translating;
  translateBtn.textContent = translating ? '翻译中...' : '翻译';
  cancelBtn.hidden = !translating;
  retryBtn.hidden = !canRetry;
  retryBtn.disabled = translating;
}

function scrollOutputToBottom() {
  outputText.scrollTop = outputText.scrollHeight;
}

function getSessionId(payload) {
  const sessionId = payload?.sessionId;
  if (typeof sessionId === 'string') {
    return sessionId;
  }
  if (sessionId && typeof sessionId === 'object') {
    return sessionId[0] ?? sessionId['0'] ?? null;
  }
  return null;
}

async function applyPendingSourceText() {
  if (!invoke) {
    return;
  }
  try {
    const sourceText = await invoke('take_pending_source_text');
    if (sourceText) {
      inputText.value = sourceText;
    }
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
}

function shouldHandleSessionEvent(payload) {
  const sessionId = getSessionId(payload);
  return !currentSessionId || !sessionId || sessionId === currentSessionId;
}

function renderTranslationEvent(payload) {
  switch (payload.type) {
    case 'started':
      currentSessionId = getSessionId(payload);
      inputText.value = payload.sourceText ?? inputText.value;
      outputText.textContent = '';
      outputText.style.color = '#333';
      setSourceBadge(payload.sourceType);
      setActionButtons({ translating: true, canRetry: false });
      break;
    case 'delta':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += payload.text ?? '';
      outputText.style.color = '#333';
      scrollOutputToBottom();
      break;
    case 'finished':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.fullText ?? outputText.textContent;
      outputText.style.color = '#333';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'failed':
      if (currentSessionId && !shouldHandleSessionEvent(payload)) return;
      outputText.textContent = payload.message ?? '翻译失败';
      outputText.style.color = '#b42318';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      scrollOutputToBottom();
      break;
    case 'cancelled':
      if (!shouldHandleSessionEvent(payload)) return;
      outputText.textContent += '\n[已取消]';
      outputText.style.color = '#999';
      currentSessionId = null;
      hideSourceBadge();
      setActionButtons({ translating: false, canRetry: true });
      break;
    default:
      break;
  }
}

if (listen) {
  listen('translation:event', (event) => {
    renderTranslationEvent(event.payload);
  });
}

window.addEventListener('focus', applyPendingSourceText);

settingsBtn.addEventListener('click', async () => {
  if (!invoke) return;
  try {
    await invoke('open_settings');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
});

translateBtn.addEventListener('click', async () => {
  if (isTranslating) {
    return;
  }
  const text = inputText.value.trim();
  if (!text) {
    outputText.textContent = '请输入要翻译的文本';
    outputText.style.color = '#999';
    return;
  }
  if (!invoke) {
    outputText.textContent = 'Tauri API 未就绪，请在桌面应用中运行';
    outputText.style.color = '#b42318';
    return;
  }
  outputText.textContent = '翻译中...';
  outputText.style.color = '#999';
  setActionButtons({ translating: true, canRetry: false });
  try {
    await invoke('start_translation', { text });
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    currentSessionId = null;
    hideSourceBadge();
    setActionButtons({ translating: false, canRetry: true });
  }
});

clearBtn.addEventListener('click', () => {
  if (isTranslating) {
    return;
  }
  inputText.value = '';
  currentSessionId = null;
  resetOutput();
  hideSourceBadge();
  setActionButtons({ translating: false, canRetry: false });
});

cancelBtn.addEventListener('click', async () => {
  if (!invoke) {
    return;
  }
  try {
    await invoke('cancel_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
  }
});

retryBtn.addEventListener('click', async () => {
  if (isTranslating) {
    return;
  }
  if (!invoke) {
    outputText.textContent = 'Tauri API 未就绪，请在桌面应用中运行';
    outputText.style.color = '#b42318';
    return;
  }
  outputText.textContent = '翻译中...';
  outputText.style.color = '#999';
  setActionButtons({ translating: true, canRetry: false });
  try {
    await invoke('retry_translation');
  } catch (error) {
    outputText.textContent = String(error);
    outputText.style.color = '#b42318';
    currentSessionId = null;
    hideSourceBadge();
    setActionButtons({ translating: false, canRetry: true });
  }
});

inputText.addEventListener('keydown', (e) => {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault();
    translateBtn.click();
  }
});

applyPendingSourceText();
```

- [ ] **步骤 3：创建 translate.css**

```css
* { margin: 0; padding: 0; box-sizing: border-box; }
[hidden] { display: none !important; }
.hidden { display: none !important; }

body {
  font-family: system-ui, -apple-system, 'Segoe UI', sans-serif;
  background: #f5f5f5;
  color: #333;
  height: 100vh;
  overflow: hidden;
  user-select: none;
}

.container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  padding: 16px;
  gap: 8px;
}

.input-area textarea {
  width: 100%;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  font-family: inherit;
  resize: none;
  outline: none;
  background: #fff;
  transition: border-color 0.2s;
}
.input-area textarea:focus { border-color: #4a90d9; }

.action-bar { display: flex; gap: 8px; }
.action-bar button {
  flex: 1;
  padding: 8px;
  border: none;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.2s;
}
.action-bar button:hover:not(:disabled) { opacity: 0.85; }
button:disabled { cursor: not-allowed; opacity: 0.55; }

#translateBtn { background: #4a90d9; color: #fff; }
#settingsBtn { background: #f0f0f0; color: #555; }
#clearBtn { background: #e0e0e0; color: #555; }
#cancelBtn { background: #f0f0f0; color: #555; }
#retryBtn { background: #38bdf8; color: #fff; }

.output-area {
  flex: 1;
  min-height: 120px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.source-badge {
  align-self: flex-start;
  padding: 2px 8px;
  border-radius: 6px;
  font-size: 11px;
  color: #555;
  background: #eef3f8;
}

.output-box {
  flex: 1;
  padding: 10px;
  border: 1px solid #ddd;
  border-radius: 8px;
  font-size: 14px;
  background: #fff;
  color: #999;
  overflow-y: auto;
}
```

- [ ] **步骤 4：语法检查**

运行：`node --check frontend/translate.js`
预期：无输出（语法正确）。

- [ ] **步骤 5：Commit**

```bash
git add frontend/translate.html frontend/translate.js frontend/translate.css
git commit -m "feat(frontend): 独立翻译弹窗 translate"
```

---

## 任务 17：删除旧前端 index.html / main.js / style.css

**文件：**
- 删除：`frontend/index.html`
- 删除：`frontend/main.js`
- 删除：`frontend/style.css`

- [ ] **步骤 1：删除旧文件**

运行：

```bash
git rm frontend/index.html frontend/main.js frontend/style.css
```

- [ ] **步骤 2：确认无残留引用**

搜索是否还有引用旧前端文件的地方：

运行：`git grep -n "index.html\|main.js\|style.css" -- src-tauri frontend || echo "无残留引用"`
预期：输出「无残留引用」。若有命中，修正后再继续。

- [ ] **步骤 3：Commit**

```bash
git commit -m "chore(frontend): 删除拆分前的单窗口前端 index/main/style"
```

---

## 任务 18：验收——构建、测试、语法检查

**文件：** 无（验证步骤）

- [ ] **步骤 1：Rust 测试全绿**

运行：`cd src-tauri && cargo test`
预期：全部测试 PASS（含任务 1/2 新增测试与全部既有测试）。

- [ ] **步骤 2：Release 构建无警告**

运行：`cd src-tauri && cargo build --release`
预期：编译通过，无 warning（特别检查 dead_code、unused_import）。

- [ ] **步骤 3：前端语法检查**

运行：

```bash
node --check frontend/translate.js
node --check frontend/settings.js
```

预期：均无输出。

- [ ] **步骤 4：手动验证（mock provider，桌面环境）**

启动：`npm run tauri dev`，逐项验证并记录结果：

1. 首次未配置启动 → 设置页主窗口显示；保存 mock provider 配置后重启 → 主窗口隐藏驻留托盘。
2. `Alt+T` 划词 → 翻译弹窗在光标附近出现并自动翻译；光标近屏幕右/下边界时弹窗左/上移不溢出。
3. `Alt+O` 截图 OCR → 框选 → 弹窗出现并翻译，徽章「来自 OCR」。
4. 托盘「翻译」→ 空弹窗手动输入翻译。
5. 托盘「设置」/ 弹窗「设置」按钮 → 设置页主窗口显示。
6. 设置页切换 `popup_precreate=false` 保存 → 提示「重启生效」→ 重启 → 弹窗运行时创建、关闭即销毁；改回 `true` 重启 → 预创建、关闭即隐藏。
7. `overlay_precreate=false` 重启 → overlay 运行时创建；连续两次 `Alt+O` 不残留旧帧；`=true` 同理。
8. 翻译 finished/取消/失败/清空 → 来源徽章隐藏（回归现有行为）。

- [ ] **步骤 5：文档同步**

按协作规范第 2 条，同步：
- spec 文档复选框回填（若 spec 含验收清单）。
- [README](../../README.md) 当前能力与限制（窗口拆分形态、窗口策略配置项）。
- roadmap 完成状态、[架构文档](../../architecture/ui-decoupling-proposal.md) MVP 偏差项更新（设置页已独立、弹窗双模式）。

- [ ] **步骤 6：收尾**

执行 `finishing-a-development-branch`（或等价 finish 流程）前，确认文档已同步；若遗漏，finish 第一步先补齐。

---

## 自检结果

**1. 规格覆盖度：**
- 窗口模型（main/translation-popup/overlay 创建机制与生命周期）→ T4/T7/T12/T13。
- 窗口策略配置项 → T1（字段）+ T15（设置 UI）+ T12（setup 分支）。
- 启动显隐（is_configured）→ T1（is_configured）+ T12（setup 显隐）。
- 唤起入口与修正缺陷（Alt+T/Alt+O/托盘/手动）→ T5/T6/T8/T10。
- 光标定位 PopupAnchor + compute_popup_position 纯函数 → T2 + T3 + T4。
- web_popup 重组（show_translation_popup）→ T5。
- 设置页拆分（剥离 settingsPanel/翻译区、窗口策略分组、open_settings、设置入口）→ T11/T15/T10。
- 统一窗口管理模块 popup_window.rs → T4。
- 关闭事件挂载 → T4（popup）+ T7（overlay 复用现有 hide 语义）+ T9（main）。
- capabilities 同步 → T14。
- 前端文件拆分 + 路由 + 职责边界 + 清理 → T13/T15/T16/T17。
- 测试与验收（TDD 单测、前端语法、手动验证、验收标准）→ T1/T2/T18。

**2. 占位符扫描：** 任务 12 含一段「修正占位行需删除」的说明——这是对编写过程中一处临时占位符名的显式删除指令，最终 import 块已给出正确写法，不留占位符。其余任务均含完整代码与命令。已确认无「TODO/待定/类似任务 N」。

**3. 类型一致性：**
- `AppConfig.popup_precreate`/`overlay_precreate`/`is_configured()` 在 T1 定义，T4/T5/T6/T7/T8/T10/T12/T15 使用，名称一致。
- `compute_popup_position(LogicalPos, LogicalSize, LogicalRect) -> LogicalPos` 在 T2 定义，T4 使用，签名一致。
- `show_translation_popup(app, &AppConfig)` 在 T5 定义，T6/T8/T10 使用，签名一致。
- `open_overlay(app, &AppConfig)` / `ensure_overlay(app)` 在 T7 定义，T8/T12 使用，签名一致。
- `open_settings` command 在 T11 定义，T12 注册，T16 调用，名称一致。
- `cursor_logical_context(scale) -> Option<(f64,f64,f64,f64,f64,f64)>` 在 T3 定义，T4 使用，签名一致。
