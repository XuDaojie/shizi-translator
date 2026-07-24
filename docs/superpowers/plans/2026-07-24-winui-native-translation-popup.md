# Windows WinUI3 原生翻译弹窗（与 WebView 并行）实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 在 Windows 上以 WinUI3 实现与现有 Vue 弹窗功能对等的原生翻译窗，与 WebView 并行；经 `popupUi` 配置切换（默认 `webview`）；业务核心仍在 Rust，同进程宿主（hostfxr），失败会话回退 webview。

**架构：** Rust 引入 `PopupUi` trait + 会话级 backend 选择；`WebviewPopupUi` 包装现有 `popup_window`；`WinUiPopupUi` 经 hostfxr 加载 `native/windows/popup` 托管库。翻译命令/事件走 `PopupBridge`（JSON UTF-8、`bridgeVersion: 1`），与现有 `invoke`/`translation:event` 语义同构。设置页正式暴露切换项；切换**下次唤起**生效。

**技术栈：** Tauri 2 + Rust、.NET 8 / WinUI 3 / Windows App SDK（Release 自包含）、Vue 3 设置页、vitest / cargo test / `dotnet test`

**规格来源：** `docs/superpowers/specs/2026-07-24-winui-native-translation-popup-design.md`

---

## 与 spec 的实现澄清（写死未决项）

1. **Windows App SDK 分发**  
   - **Release / Tauri NSIS 打包：自包含（SelfContained=true + WindowsAppSDKSelfContained=true）**，用户无需预装 WASDK。  
   - **本地 Debug：允许 Framework-dependent** 以加快迭代（csproj 用 `$(Configuration)` 条件）。  
   - 产物目录约定：`native/windows/popup/bin/$(Configuration)/net8.0-windows10.0.19041.0/win-x64/`（及自包含子目录），Rust 侧按 `CARGO_MANIFEST_DIR` 相对路径 + 安装后 `resource_dir` 查找。

2. **设置页位置**  
   - 放在 **`GeneralPanel` →「外观」组**（`settings.group.appearance`），紧接「界面语言」之后。  
   - 控件：`SettingSelect`，选项 `webview` / `winui`；非 Windows 禁用 `winui` 并显示说明。  
   - 描述文案强调：**保存后下次打开翻译弹窗生效**。

3. **子进程 IPC 降级**  
   - **本计划主路径 = 进程内 hostfxr**。  
   - Bridge **消息体与 `PopupUi` 语义**与是否同进程无关；协议层不绑定 transport。  
   - 若任务 8 的「宿主/打包 spike」证明进程内不可行：在**同一任务内**改为启动 `Shizi.Popup.exe` + 本地命名管道/TCP localhost，**不改** `popupUi: winui` 配置语义；不另开规格。

4. **`emit_translation_event` 双投**  
   - 保留对 WebView 的 `app.emit("translation:event", …)`（webview 后端激活时前端仍依赖）。  
   - 当会话激活 backend 为 winui 时，**额外**经 Bridge `push` 同构 JSON；webview 隐藏时前端可收事件但用户不可见（可接受）；**禁止**同时 show 两个弹窗。

5. **配置字段名**  
   - JSON / 前端：`popupUi`  
   - Rust：`popup_ui: String`（`#[serde(rename_all = "camelCase")]`）  
   - 合法值：`"webview" | "winui"`；默认 `"webview"`；非法 → `normalized` 为 `"webview"`。  
   - 非 Windows：配置可读可写；`resolve_popup_ui_kind` 运行时强制 `Webview`。

6. **脱敏 `get_app_config` 经 Bridge**  
   - Bridge 响应 `get_app_config` 时对 `services[].apiKey` / `ocrServices[].apiKey` 置 `null`（与前端「不在 UI 层持有密钥用于展示」一致；弹窗卡片只需 id/name/protocol/model/enabled）。

7. **本轮明确不做**  
   - 设置页 / OCR / overlay 原生化；C# 内翻译协议；热切换无缝迁移状态；跨窗口 `INativeWindow` 大框架；独立分发第二产品；非 Windows 原生弹窗。

---

## 文件结构

| 文件 | 职责 |
|------|------|
| 修改 `src-tauri/src/core/config/types.rs` | `AppConfig.popup_ui` + default + `normalized` |
| 创建 `src-tauri/src/app/popup_ui/mod.rs` | 模块导出、`PopupUi` trait、`PopupPositionMode` 再导出 |
| 创建 `src-tauri/src/app/popup_ui/kind.rs` | `PopupUiKind::{Webview,WinUi}` 解析与平台强制 |
| 创建 `src-tauri/src/app/popup_ui/session.rs` | 进程内激活 backend、会话回退 webview、互斥 hide |
| 创建 `src-tauri/src/app/popup_ui/webview.rs` | `WebviewPopupUi`：包装 `popup_window` |
| 创建 `src-tauri/src/app/popup_ui/winui.rs` | `#[cfg(windows)]` `WinUiPopupUi`：hostfxr + 壳操作 |
| 创建 `src-tauri/src/app/popup_ui/facade.rs` | `ensure_popup` / `show_popup` / `hide_popup` 统一入口 |
| 修改 `src-tauri/src/app/popup_window.rs` | 保留 WebView 实现细节；对外入口逐步改调 facade（或 facade 调本文件） |
| 修改 `src-tauri/src/app/mod.rs` | `pub mod popup_ui;` |
| 创建 `src-tauri/src/app/popup_bridge/mod.rs` | Bridge 模块 |
| 创建 `src-tauri/src/app/popup_bridge/protocol.rs` | 请求/推送信封、`bridgeVersion`、serde 类型 + 单测 |
| 创建 `src-tauri/src/app/popup_bridge/dispatch.rs` | UI→Rust：映射到现有 `web_popup` / `config` / OCR 入口 |
| 创建 `src-tauri/src/app/popup_bridge/push.rs` | Rust→UI 推送（含双投策略） |
| 创建 `src-tauri/src/app/popup_bridge/host.rs` | `#[cfg(windows)]` hostfxr 加载、回调注册、shutdown |
| 修改 `src-tauri/src/ui/web_popup.rs` | `show_translation_popup*` / `hide` 走 facade；`emit_translation_event` 双投 |
| 修改 `src-tauri/src/ui/config.rs` | `save_app_config` 后通知 session 期望 backend 变更（hide 旧后端） |
| 修改 `src-tauri/src/lib.rs` | setup 预建走 facade；退出 best-effort shutdown |
| 修改 `src-tauri/src/app/state.rs` | 可选：挂 `PopupSession`/`BridgeHub` 于 `AppState`（或 `OnceLock` 全局） |
| 修改 `src-tauri/Cargo.toml` | Windows：`libloading`（或等价）加载 hostfxr；**不加**无用依赖 |
| 修改 `src-tauri/build.rs` | Windows：可选 `dotnet build` 产出复制到 `target/` 旁（失败仅 warn，dev 可手编） |
| 创建 `native/README.md` | 目录约定、构建、Bridge 说明 |
| 创建 `native/windows/popup/Shizi.Popup.csproj` | WinUI3 库/宿主工程 |
| 创建 `native/windows/popup/Shizi.Popup.sln` | 可选解决方案 |
| 创建 `native/windows/popup/**` | Host / Bridge / UI / State / 资源 / 单测项目 |
| 修改 `frontend/src/types/config.ts` | `popupUi: 'webview' \| 'winui'` |
| 修改 `frontend/src/lib/config.ts` | `projectToAppConfig` 投影 |
| 修改 `frontend/src/lib/config.test.ts` | 投影断言 |
| 修改 `frontend/src/settings/types.ts` | `GeneralSettings.popupUi` |
| 修改 `frontend/src/settings/stores/settings.ts` | defaults + `syncFromBackend` |
| 修改 `frontend/src/settings/stores/settings.test.ts` | `makeAppConfig` / merge 断言 |
| 修改 `frontend/src/settings/panels/GeneralPanel.vue` | 外观组选择器 |
| 修改 `frontend/src/i18n/locales/*.json`（8 语） | 字段/描述/选项文案 |
| 修改 `.github/workflows/ci.yml`（及 release 若需要） | Windows job：`dotnet build` + `cargo test`；非 Windows 跳过 native |
| 修改 `docs/agent/architecture-notes.md` | 双后端、`popupUi`、`native/windows/popup` |
| 修改 `README.md` / `docs/roadmap/progressive-development-plan.md` / `AGENTS.md` / `CLAUDE.md` | 收尾文档（若文案触及弹窗技术栈） |

**刻意不改：**  
- `pot-desktop/`  
- 设置页其它面板的业务逻辑  
- 翻译 provider / OCR 核心协议  

---

## 任务 1：后端 `AppConfig.popup_ui`（TDD）

**文件：**
- 修改：`src-tauri/src/core/config/types.rs`
- 测试：同文件 `#[cfg(test)]`

- [ ] **步骤 1：编写失败的测试**

在 `types.rs` 的 tests 模块追加：

```rust
#[test]
fn app_config_defaults_popup_ui_webview() {
    let config = AppConfig::default();
    assert_eq!(config.popup_ui, "webview");
}

#[test]
fn app_config_missing_popup_ui_deserializes_to_webview() {
    let json = r#"{
        "targetLang": "zh-CN",
        "services": [],
        "ocrServices": []
    }"#;
    let config: AppConfig = serde_json::from_str(json).expect("deserialize");
    let config = config.normalized();
    assert_eq!(config.popup_ui, "webview");
}

#[test]
fn app_config_normalized_rejects_invalid_popup_ui() {
    let mut config = AppConfig::default();
    config.popup_ui = "native".into();
    let config = config.normalized();
    assert_eq!(config.popup_ui, "webview");
}

#[test]
fn app_config_accepts_winui_popup_ui() {
    let mut config = AppConfig::default();
    config.popup_ui = "winui".into();
    let config = config.normalized();
    assert_eq!(config.popup_ui, "winui");
}
```

- [ ] **步骤 2：运行测试确认失败**

```bash
cd src-tauri && cargo test app_config_defaults_popup_ui_webview app_config_missing_popup_ui_deserializes_to_webview app_config_normalized_rejects_invalid_popup_ui app_config_accepts_winui_popup_ui -- --nocapture
```

预期：编译失败（字段不存在）或 FAIL。

- [ ] **步骤 3：最少实现**

在 `AppConfig`（`#[serde(rename_all = "camelCase")]`）增加：

```rust
#[serde(default = "default_popup_ui")]
pub popup_ui: String,
```

```rust
fn default_popup_ui() -> String {
    "webview".to_string()
}

fn normalize_popup_ui(value: String) -> String {
    match value.as_str() {
        "webview" | "winui" => value,
        _ => "webview".to_string(),
    }
}
```

- `default()` 初始化 `popup_ui: default_popup_ui()`  
- `normalized()` 末尾：`self.popup_ui = normalize_popup_ui(self.popup_ui);`

- [ ] **步骤 4：运行测试确认通过**

```bash
cd src-tauri && cargo test app_config_defaults_popup_ui -- --nocapture
```

预期：相关 4 测 PASS。

- [ ] **步骤 5：Commit**

```bash
git add src-tauri/src/core/config/types.rs
git commit -m "feat(config): 增加 popupUi 配置项（webview/winui）"
```

---

## 任务 2：前端配置类型、投影、设置页与 i18n

**文件：**
- 修改：`frontend/src/types/config.ts`
- 修改：`frontend/src/lib/config.ts`
- 修改：`frontend/src/lib/config.test.ts`
- 修改：`frontend/src/settings/types.ts`
- 修改：`frontend/src/settings/stores/settings.ts`
- 修改：`frontend/src/settings/stores/settings.test.ts`
- 修改：`frontend/src/settings/panels/GeneralPanel.vue`
- 修改：`frontend/src/i18n/locales/zh-CN.json`、`en-US.json`、`zh-TW.json`、`ja-JP.json`、`ko-KR.json`、`fr-FR.json`、`de-DE.json`、`es-ES.json`

- [ ] **步骤 1：编写失败的测试**

在 `frontend/src/lib/config.test.ts` 追加：

```ts
it('投影 popupUi（默认 webview）', () => {
  const state = makeState([])
  expect(projectToAppConfig(state).popupUi).toBe('webview')
  state.general.popupUi = 'winui'
  expect(projectToAppConfig(state).popupUi).toBe('winui')
})
```

在 `settings.test.ts` 的 `makeAppConfig` 增加默认 `popupUi: 'webview'`，并在 `syncFromBackend` 相关用例中断言后端 `popupUi: 'winui'` 会写入 `state.general.popupUi`（仿 `updateChannel` 写法）。

- [ ] **步骤 2：运行测试确认失败**

```bash
cd frontend && npx vitest run src/lib/config.test.ts src/settings/stores/settings.test.ts
```

预期：FAIL（类型/字段缺失）。

- [ ] **步骤 3：最少实现**

1. `AppConfig` 增加 `popupUi: 'webview' | 'winui'`  
2. `GeneralSettings` 增加 `popupUi: 'webview' | 'winui'`  
3. `buildDefaults()`：`popupUi: 'webview'`  
4. `projectToAppConfig`：`popupUi: state.general.popupUi === 'winui' ? 'winui' : 'webview'`  
5. `syncFromBackend`：`state.general.popupUi = backend.popupUi === 'winui' ? 'winui' : 'webview'`  
6. `GeneralPanel.vue` 外观组增加：

```vue
<SettingRow
  :title="t('settings.field.popupUi')"
  :description="t('settings.description.popupUi')"
>
  <SettingSelect
    v-model="state.general.popupUi"
    :options="popupUiOptions"
    :disabled="!popupUiWinuiAvailable"
  />
</SettingRow>
```

```ts
const popupUiWinuiAvailable = computed(() => {
  // 仅 Windows 桌面壳可选；纯 vite / 非 win 禁用
  const p = (navigator.userAgentData as { platform?: string } | undefined)?.platform
    ?? navigator.platform
  return /Win/i.test(String(p))
})
const popupUiOptions = computed(() => [
  { label: t('settings.option.popupUiWebview'), value: 'webview' },
  {
    label: t('settings.option.popupUiWinui'),
    value: 'winui',
    // SettingSelect 若不支持 per-option disabled，则整体在非 Windows disable + 描述说明
  },
])
```

非 Windows：若当前值为 `winui` 仍显示，但选择控件 disabled，描述用 `settings.description.popupUiWindowsOnly`。

7. i18n（至少 zh-CN / en-US 写准，其余语种可先英/中等价填入，禁止缺 key）：

```json
"settings.field.popupUi": "翻译弹窗 UI",
"settings.description.popupUi": "切换后下次打开翻译弹窗生效。WinUI3 仅 Windows 可用。",
"settings.option.popupUiWebview": "WebView（默认）",
"settings.option.popupUiWinui": "WinUI3 原生",
"settings.description.popupUiWindowsOnly": "WinUI3 原生弹窗仅在 Windows 上可用。"
```

- [ ] **步骤 4：运行测试确认通过**

```bash
cd frontend && npx vitest run src/lib/config.test.ts src/settings/stores/settings.test.ts
```

预期：PASS。

- [ ] **步骤 5：Commit**

```bash
git add frontend/src/types/config.ts frontend/src/lib/config.ts frontend/src/lib/config.test.ts frontend/src/settings/types.ts frontend/src/settings/stores/settings.ts frontend/src/settings/stores/settings.test.ts frontend/src/settings/panels/GeneralPanel.vue frontend/src/i18n/locales
git commit -m "feat(settings): 正式设置项 popupUi 切换翻译弹窗后端"
```

---

## 任务 3：`PopupUiKind` 与会话 backend 选择（纯 Rust TDD）

**文件：**
- 创建：`src-tauri/src/app/popup_ui/kind.rs`
- 创建：`src-tauri/src/app/popup_ui/session.rs`
- 创建：`src-tauri/src/app/popup_ui/mod.rs`
- 修改：`src-tauri/src/app/mod.rs`

- [ ] **步骤 1：编写失败的测试**

`kind.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_webview_and_winui() {
        assert_eq!(PopupUiKind::parse("webview"), PopupUiKind::Webview);
        assert_eq!(PopupUiKind::parse("winui"), PopupUiKind::WinUi);
        assert_eq!(PopupUiKind::parse("nope"), PopupUiKind::Webview);
    }

    #[test]
    fn resolve_forces_webview_on_non_windows() {
        // 使用显式参数，避免测试依赖宿主 OS 宏分支不可控：
        assert_eq!(
            PopupUiKind::resolve_for_platform("winui", /* is_windows */ false),
            PopupUiKind::Webview
        );
        assert_eq!(
            PopupUiKind::resolve_for_platform("winui", true),
            PopupUiKind::WinUi
        );
    }
}
```

`session.rs`：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desired_follows_config_until_session_fallback() {
        let mut s = PopupUiSession::new();
        s.set_desired(PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::WinUi);
        s.fallback_to_webview_for_session();
        assert_eq!(s.active(), PopupUiKind::Webview);
        // 用户再次选择 winui（set_desired）允许重试
        s.set_desired(PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::WinUi);
    }

    #[test]
    fn fallback_does_not_change_desired_config_value() {
        let mut s = PopupUiSession::new();
        s.set_desired(PopupUiKind::WinUi);
        s.fallback_to_webview_for_session();
        assert_eq!(s.desired(), PopupUiKind::WinUi);
        assert_eq!(s.active(), PopupUiKind::Webview);
    }
}
```

- [ ] **步骤 2：运行测试确认失败**

```bash
cd src-tauri && cargo test popup_ui -- --nocapture
```

预期：模块不存在 / FAIL。

- [ ] **步骤 3：最少实现**

```rust
// kind.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupUiKind {
    Webview,
    WinUi,
}

impl PopupUiKind {
    pub fn parse(raw: &str) -> Self {
        match raw {
            "winui" => Self::WinUi,
            _ => Self::Webview,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webview => "webview",
            Self::WinUi => "winui",
        }
    }

    pub fn resolve_for_platform(raw: &str, is_windows: bool) -> Self {
        let parsed = Self::parse(raw);
        if !is_windows && parsed == Self::WinUi {
            return Self::Webview;
        }
        parsed
    }

    pub fn resolve_from_config(raw: &str) -> Self {
        Self::resolve_for_platform(raw, cfg!(windows))
    }
}
```

```rust
// session.rs
#[derive(Debug, Default)]
pub struct PopupUiSession {
    desired: PopupUiKind,
    session_force_webview: bool,
}

impl PopupUiSession {
    pub fn new() -> Self { Self::default() }
    pub fn set_desired(&mut self, kind: PopupUiKind) {
        self.desired = kind;
        // 用户重新选择 winui 时清除会话回退，允许重试
        if kind == PopupUiKind::WinUi {
            self.session_force_webview = false;
        }
    }
    pub fn desired(&self) -> PopupUiKind { self.desired }
    pub fn active(&self) -> PopupUiKind {
        if self.session_force_webview {
            PopupUiKind::Webview
        } else {
            self.desired
        }
    }
    pub fn fallback_to_webview_for_session(&mut self) {
        self.session_force_webview = true;
    }
}
```

`mod.rs` 导出子模块；`app/mod.rs` 增加 `pub mod popup_ui;`。

- [ ] **步骤 4：测试通过 + Commit**

```bash
cd src-tauri && cargo test popup_ui -- --nocapture
git add src-tauri/src/app/popup_ui src-tauri/src/app/mod.rs
git commit -m "feat(popup): PopupUiKind 与会话级 backend 回退"
```

---

## 任务 4：`PopupUi` trait + `WebviewPopupUi` + facade 接入现网路径

**文件：**
- 创建：`src-tauri/src/app/popup_ui/webview.rs`
- 创建：`src-tauri/src/app/popup_ui/facade.rs`
- 修改：`src-tauri/src/app/popup_ui/mod.rs`
- 修改：`src-tauri/src/ui/web_popup.rs`（`show_translation_popup*`）
- 修改：`src-tauri/src/app/popup_window.rs` 或调用方（`lib.rs` `ensure_popup_window`、`hide_popup`、`shortcuts`/`tray`/`window`）
- 修改：`src-tauri/src/app/state.rs`（持有 `Mutex<PopupUiSession>` 或等价）

**目标：** 默认 `webview` 行为与现网**完全一致**；调用点统一走 facade，为 winui 留钩子。

- [ ] **步骤 1：定义 trait 与 WebView 适配（无宿主依赖）**

```rust
// mod.rs 或 trait 文件
pub trait PopupUi: Send + Sync {
    fn ensure(&self, app: &tauri::AppHandle) -> Result<(), String>;
    fn show(&self, app: &tauri::AppHandle, mode: PopupPositionMode) -> Result<(), String>;
    fn hide(&self, app: &tauri::AppHandle) -> Result<(), String>;
    fn set_always_on_top(&self, app: &tauri::AppHandle, on: bool) -> Result<(), String>;
    fn set_size(&self, app: &tauri::AppHandle, width: f64, height: f64) -> Result<(), String>;
    fn is_available(&self) -> bool;
}
```

`WebviewPopupUi`：

- `ensure` → `popup_window::ensure_popup_exists`
- `show` → `popup_window::show_popup` / `show_popup_blocking`（保持 Windows 线程规则）
- `hide` → `popup_window::hide_popup`
- `set_always_on_top` → `get_webview_window("main")?.set_always_on_top(on)`
- `set_size` → 现 WebView 由前端 `setSize`；trait 方法可 no-op 或直接 `set_size`（与现一致即可）
- `is_available` → `true`

`facade`：

```rust
pub fn ensure_for_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    // 读 session + config.popup_ui → active kind
    // 若 windowPrecreate 不要求 popup，直接 Ok
    // Webview: ensure；WinUi: ensure，失败则 session.fallback + webview.ensure
}

pub fn show_for_config(app: &AppHandle, config: &AppConfig, mode: PopupPositionMode) -> Result<(), String> { ... }

pub fn hide_active(app: &AppHandle) -> Result<(), String> { ... }

pub fn on_popup_ui_config_changed(app: &AppHandle, new_config: &AppConfig) {
    // 更新 session.desired；hide 当前后端；不立即 ensure 新后端
}
```

- [ ] **步骤 2：接入调用点**

将下列入口改为 facade（保持函数名可兼容，避免大爆炸重命名）：

| 现调用 | 新行为 |
|--------|--------|
| `popup_window::ensure_popup_window` | 内部转 `facade::ensure_for_config` 或 lib 直接调 facade |
| `popup_window::show_popup` / `show_popup_blocking` | facade（webview 分支仍用原实现） |
| `popup_window::hide_popup` | facade hide active（**同时**可 hide webview 以保证互斥） |
| `web_popup::show_translation_popup*` | facade show |
| `save_app_config` 成功后 | `on_popup_ui_config_changed` |

**互斥：** 无论 active 为何，show 前对非 active 后端 `hide()`。

- [ ] **步骤 3：回归测试**

```bash
cd src-tauri && cargo test compute_popup_position app_config_ -- --nocapture
cd src-tauri && cargo build
```

预期：编译通过；既有定位/配置测 PASS。手工：`npm run tauri dev` 默认 webview 划词仍可用（执行阶段验证）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_ui src-tauri/src/app/popup_window.rs src-tauri/src/ui/web_popup.rs src-tauri/src/ui/config.rs src-tauri/src/app/state.rs src-tauri/src/lib.rs
git commit -m "refactor(popup): PopupUi facade 统一弹窗 ensure/show/hide"
```

---

## 任务 5：Bridge 协议信封与序列化（TDD）

**文件：**
- 创建：`src-tauri/src/app/popup_bridge/protocol.rs`
- 创建：`src-tauri/src/app/popup_bridge/mod.rs`
- 修改：`src-tauri/src/app/mod.rs`（`pub mod popup_bridge;`）

- [ ] **步骤 1：编写失败的测试**

```rust
#[test]
fn bridge_request_roundtrip_start_translation() {
    let raw = r#"{"bridgeVersion":1,"type":"start_translation","requestId":"r1","payload":{"text":"hello"}}"#;
    let env: BridgeEnvelope = serde_json::from_str(raw).unwrap();
    assert_eq!(env.bridge_version, 1);
    assert_eq!(env.type_name, "start_translation");
    let p: StartTranslationPayload = serde_json::from_value(env.payload.unwrap()).unwrap();
    assert_eq!(p.text, "hello");
}

#[test]
fn translation_event_push_matches_camel_case_fields() {
    // 使用与 core::translation::TranslationEvent 相同的 serde 输出再包一层
    let push = BridgePush {
        bridge_version: 1,
        type_name: "translation_event".into(),
        payload: serde_json::json!({
            "type": "delta",
            "sessionId": "b1:svc",
            "serviceInstanceId": "svc",
            "text": "你好"
        }),
    };
    let s = serde_json::to_string(&push).unwrap();
    assert!(s.contains("bridgeVersion"));
    assert!(s.contains("translation_event"));
    assert!(s.contains("serviceInstanceId"));
}

#[test]
fn unknown_type_is_parseable_for_ignore() {
    let raw = r#"{"bridgeVersion":1,"type":"future_thing"}"#;
    let env: BridgeEnvelope = serde_json::from_str(raw).unwrap();
    assert_eq!(env.type_name, "future_thing");
}
```

- [ ] **步骤 2：实现协议类型**

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEnvelope {
    pub bridge_version: u32,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgePush {
    pub bridge_version: u32,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTranslationPayload { pub text: String }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSessionLanguagesPayload {
    pub source_lang: String,
    pub target_lang: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportContentSizePayload {
    pub width: f64,
    pub height: f64,
}
```

**请求 type 全集（与 spec §6.2）：**  
`start_translation` | `cancel_translation` | `retry_translation` | `set_session_languages` | `open_settings` | `trigger_ocr_translation` | `save_edge_translate_env` | `take_pending_source_text` | `get_app_config` | `report_content_size` | `ready`

**推送 type：**  
`translation_event` | `app_config_changed` | `interface_language_changed` | `show_context` | `response`（带 `requestId` + `ok`/`error`/`body`）

- [ ] **步骤 3：测试通过 + Commit**

```bash
cd src-tauri && cargo test popup_bridge::protocol -- --nocapture
git add src-tauri/src/app/popup_bridge src-tauri/src/app/mod.rs
git commit -m "feat(popup-bridge): Bridge JSON 协议信封与单测"
```

---

## 任务 6：Bridge dispatch（UI→Rust）接到现有业务

**文件：**
- 创建：`src-tauri/src/app/popup_bridge/dispatch.rs`
- 修改：`src-tauri/src/app/popup_bridge/mod.rs`

- [ ] **步骤 1：编写可单测的纯分发映射（无 Tauri 时用假回调）**

对「仅解析 + 选择 handler 名」可单测；真正 invoke 业务在集成函数 `handle_bridge_request(app, state, envelope) -> BridgeResponse`。

```rust
#[test]
fn maps_known_types() {
    assert!(matches!(classify("start_translation"), BridgeOp::StartTranslation));
    assert!(matches!(classify("ready"), BridgeOp::Ready));
    assert!(matches!(classify("nope"), BridgeOp::Unknown));
}
```

- [ ] **步骤 2：实现 `handle_bridge_request`**

| type | 调用 |
|------|------|
| `start_translation` | `web_popup::start_translation_from_text` |
| `cancel_translation` | 现 cancel 逻辑（与 command 相同路径） |
| `retry_translation` | 现 retry |
| `set_session_languages` | `state.set_session_languages` |
| `open_settings` | `show_settings_window` / `open_settings` |
| `trigger_ocr_translation` | `shortcuts::trigger_ocr_translate`（内部 hide 弹窗） |
| `save_edge_translate_env` | 现 command 体 |
| `take_pending_source_text` | `state.take_pending_source_text` |
| `get_app_config` | `config_store.get` + **脱敏 apiKey** |
| `report_content_size` | `PopupUi::set_size`（active backend） |
| `ready` | 标记 ready；解除 show 门闩（对齐 2s 超时） |
| 未知 | log::warn，response error 或 ignore |

`BridgeResponse`：

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeResponse {
    bridge_version: u32,
    #[serde(rename = "type")]
    type_name: &'static str, // "response"
    request_id: Option<String>,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}
```

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/app/popup_bridge
git commit -m "feat(popup-bridge): UI→Rust 请求分发到现有翻译/配置入口"
```

---

## 任务 7：Bridge push + `emit_translation_event` 双投

**文件：**
- 创建：`src-tauri/src/app/popup_bridge/push.rs`
- 修改：`src-tauri/src/ui/web_popup.rs`（`emit_translation_event`）
- 修改：配置变更 / 界面语言变更广播点（`ui/config.rs`、`ui/i18n.rs`）

- [ ] **步骤 1：实现 push hub**

```rust
pub struct BridgePushHub { /* optional callback: Box<dyn Fn(String) + Send + Sync> */ }

impl BridgePushHub {
    pub fn set_sink(&self, f: Option<Arc<dyn Fn(String) + Send + Sync>>);
    pub fn push_json(&self, type_name: &str, payload: serde_json::Value);
}
```

- [ ] **步骤 2：改造 `emit_translation_event`**

```rust
pub fn emit_translation_event(app: &AppHandle, event: TranslationEvent) -> Result<(), tauri::Error> {
    // 1) 始终 emit 给 WebView（保持兼容）
    app.emit(TRANSLATION_EVENT, &event)?;
    // 2) 若 session.active == WinUi 且 hub 有 sink，再 push
    let value = serde_json::to_value(&event).unwrap_or_default();
    crate::app::popup_bridge::push::global().push_json("translation_event", value);
    Ok(())
}
```

同理：`app-config:changed`、`interface-language:changed` 在 winui 激活时 push `app_config_changed` / `interface_language_changed`（payload 与现事件同构；config 脱敏）。

- [ ] **步骤 3：`show_context` 推送**

在 facade `show` 成功路径（或划词启动翻译前）：若 active 为 winui，push：

```json
{
  "bridgeVersion": 1,
  "type": "show_context",
  "payload": {
    "sourceText": "...",
    "sourceBadge": "selectedText",
    "positionMode": "nearCursor"
  }
}
```

原文仍可走 pending + `take_pending_source_text`（冷启动对齐 Vue）。

- [ ] **步骤 4：Commit**

```bash
git add src-tauri/src/app/popup_bridge src-tauri/src/ui/web_popup.rs src-tauri/src/ui/config.rs src-tauri/src/ui/i18n.rs
git commit -m "feat(popup-bridge): 翻译事件与配置变更双投到 WinUI Bridge"
```

---

## 任务 8：WinUI 工程骨架 + 窗口壳 + hostfxr 宿主（含打包 spike）

**文件：**
- 创建：`native/windows/popup/Shizi.Popup.csproj`
- 创建：`native/windows/popup/Host/PopupExports.cs`（`UnmanagedCallersOnly` 入口）
- 创建：`native/windows/popup/Host/PopupApplication.cs` / `MainWindow.xaml(+.cs)`
- 创建：`native/windows/popup/Bridge/NativeBridge.cs`
- 创建：`src-tauri/src/app/popup_bridge/host.rs`
- 创建：`src-tauri/src/app/popup_ui/winui.rs`
- 修改：`src-tauri/Cargo.toml`、`build.rs`
- 创建：`native/README.md`（可先最小版，任务 12 补全）

### 8.A C# 工程最小集

- TFM：`net8.0-windows10.0.19041.0`  
- `UseWinUI=true`，Windows App SDK 1.5+（与机器 SDK 对齐，pin 具体 Version 在 csproj）  
- `EnableMsixTooling` 按 unpackaged 桌面宿主需要配置  
- Release：

```xml
<SelfContained>true</SelfContained>
<WindowsAppSDKSelfContained>true</WindowsAppSDKSelfContained>
<RuntimeIdentifier>win-x64</RuntimeIdentifier>
```

导出 C ABI（名称固定，Rust 侧写死）：

| 导出 | 语义 |
|------|------|
| `shizi_popup_initialize(request_cb, push 保留)` | 启 STA 线程 + DispatcherQueue；注册 UI→Rust 回调 |
| `shizi_popup_ensure()` | 建隐藏窗 |
| `shizi_popup_show(x, y, mode)` | mode: 0 NearCursor 用坐标 / 1 Restore 忽略坐标 |
| `shizi_popup_hide()` | hide 不销毁 |
| `shizi_popup_set_always_on_top(bool)` | 图钉 |
| `shizi_popup_set_size(w, h)` | 逻辑像素 |
| `shizi_popup_push_json(ptr, len)` | Rust→UI |
| `shizi_popup_shutdown()` | best-effort |
| `shizi_popup_is_available()` | 1/0 |

窗口壳要求（spec §8.1）：无系统标题栏、圆角阴影、不进任务栏、内容宽约 420、顶栏可拖。

### 8.B Rust hostfxr

- 首次 `WinUiPopupUi::ensure`：定位 `hostfxr` → 加载托管库 → `initialize`  
- 进程级单例（`OnceLock`）  
- 回调：C# → `extern "C" fn(bytes, len)` → 解析 JSON → `dispatch::handle_bridge_request` → 回写 response（同步返回或再 `push_json` `type=response`）  
- **线程：** UI 在 STA；回调进 Rust 时尽量快速投递到 async/业务线程，避免卡 UI  

### 8.C Spike 验收（本任务完成门禁）

在 Windows 开发机：

```bash
dotnet build native/windows/popup/Shizi.Popup.csproj -c Release
cd src-tauri && cargo test popup_ui -- --nocapture
# 临时把 config popupUi=winui 或单测/dev 钩子
npm run tauri dev
```

必须观察到：ensure 后存在隐藏 WinUI 窗；show 出现；hide 消失且进程仍在；任务栏无常驻按钮。

若 **hostfxr + WASDK 同进程** 无法稳定启动：改为 `Command` 拉起 `Shizi.Popup.exe`，Bridge 改命名管道 `\\.\pipe\shizi-popup-{pid}`，JSON 行协议复用同一 envelope；`WinUiPopupUi` 内部换 transport，**配置与 trait 不变**。

- [ ] **步骤：实现 + Commit（可拆 2 个 commit：csproj 壳 / Rust host）**

```bash
git add native src-tauri/src/app/popup_bridge/host.rs src-tauri/src/app/popup_ui/winui.rs src-tauri/Cargo.toml src-tauri/build.rs
git commit -m "feat(winui): 原生弹窗壳与 hostfxr 宿主接入"
```

---

## 任务 9：C# 事件状态机与配置同步（TDD，纯逻辑）

**文件：**
- 创建：`native/windows/popup/State/CardState.cs`
- 创建：`native/windows/popup/State/TranslationEventReducer.cs`
- 创建：`native/windows/popup/State/CardConfigSync.cs`
- 创建：`native/windows/popup/Shizi.Popup.Tests/`（xUnit 或 MSTest）

- [ ] **步骤 1：编写失败的测试（对齐 `useTranslationEvents` / `cardConfigSync`）**

用例至少覆盖：

1. 新 batch `started`：重置卡、设 translating  
2. `delta` 迟到 batchId 忽略  
3. 首非空 delta 自动展开（无 user override）  
4. `failed` 仅该卡；其它卡不受影响  
5. `finished` 写 fullText + usage  
6. `syncCards` 空闲时按启用服务保序建卡  
7. 翻译中 `syncCards` 不新增未参与服务卡  

示例（xUnit）：

```csharp
[Fact]
public void Delta_from_stale_batch_is_ignored()
{
    var s = new PopupTranslationState();
    s.Dispatch(Started("b1:s1", "s1"));
    s.Dispatch(Delta("b2:s1", "s1", "x")); // 不同 batch
    Assert.Equal("", s.Cards["s1"].Text);
}
```

- [ ] **步骤 2：实现 reducer，跑通测试**

```bash
dotnet test native/windows/popup/Shizi.Popup.Tests/Shizi.Popup.Tests.csproj
```

- [ ] **步骤 3：Commit**

```bash
git add native/windows/popup
git commit -m "feat(winui): 翻译事件状态机与卡片同步（单测锁定）"
```

---

## 任务 10：C# 弹窗 UI 对等（区块与交互）

**文件：**
- `MainWindow.xaml` 及控件：`PopupToolbar`、`SourceCard`、`LanguageToolbar`、`ResultCard`、`StatusBar`
- `Services/BridgeService.cs`（发请求 / 收推送）
- `Services/Localization.cs`（界面语言；key 语义对齐 `popup.*`）
- `Data/TranslationLanguages.cs`（19 + auto，与 `frontend/src/shared/translation-languages.ts` 代码表一致；单测锁定 code 列表）

- [ ] **步骤 1：语言表单测**

```csharp
[Fact]
public void Target_language_codes_match_expected_count()
{
    Assert.Equal(19, TranslationLanguages.Targets.Count);
    Assert.Contains(TranslationLanguages.All, l => l.Code == "auto");
}
```

- [ ] **步骤 2：实现 UI 绑定到 `PopupTranslationState`**

| 区块 | 行为 |
|------|------|
| 顶栏 | 图钉 → `set_always_on_top`；截图译 → `trigger_ocr_translation`；设置 → `open_settings`；收藏/书签 → 本地 tip「功能开发中」 |
| 原文 | 输入、自动增高、朗读（系统 TTS）、复制、来源/检测语种徽章 |
| 语言栏 | 源/目标/交换 → `set_session_languages`；auto 交换错误提示 |
| 结果区 | 多卡保序、折叠、复制/朗读、失败重试 → `retry_translation` |
| 状态栏 | 就绪/翻译中/失败；取消 → `cancel_translation` |

流式：`delta` 更新 `Text` 绑定；不必像素级复刻 Web 光标动画，保持可读追加即可。

- [ ] **步骤 3：ready / pending / 高度**

- 首帧可交互发 `ready`  
- 冷启动：`take_pending_source_text` + `get_app_config` 初始化卡片  
- `SizeChanged` / 布局后 `report_content_size`（宽约 420，高内容 + 边距，Rust 侧钳制工作区 80%）  
- 对齐 Vue：`setSize` 逻辑宽可用 420（装饰阴影在 HWND 外侧自理）

- [ ] **步骤 4：Commit**

```bash
git add native/windows/popup
git commit -m "feat(winui): 翻译弹窗 UI 区块与 Bridge 操作对等"
```

---

## 任务 11：ensure 失败会话回退 + 切换语义收尾

**文件：**
- 修改：`src-tauri/src/app/popup_ui/facade.rs`、`winui.rs`、`session.rs`
- 修改：`src-tauri/src/ui/config.rs`（已接 `on_popup_ui_config_changed` 则补测）

- [ ] **步骤 1：单测 session + 伪 backend**

用 mock `PopupUi`（测试专用）：

```rust
#[test]
fn ensure_winui_failure_falls_back_without_writing_desired() {
    // desired=WinUi → winui.ensure err → active=Webview → webview.ensure ok
    // desired 仍为 WinUi
}
```

若 mock 难挂 Tauri，将「选择 + 回退」提纯为 `ensure_with_backends(desired, winui_ok: bool) -> PopupUiKind` 纯函数测。

- [ ] **步骤 2：实现**

```rust
pub fn ensure_for_config(...) -> Result<(), String> {
    let desired = PopupUiKind::resolve_from_config(&config.popup_ui);
    session.set_desired(desired);
    let active = session.active();
    match active {
        PopupUiKind::WinUi => {
            if let Err(e) = winui.ensure(app) {
                log::warn!("WinUI popup ensure failed, fallback to webview: {e}");
                session.fallback_to_webview_for_session();
                // 可选：一次 tray/系统提示
                webview.ensure(app)?;
            }
        }
        PopupUiKind::Webview => webview.ensure(app)?,
    }
    Ok(())
}
```

`on_popup_ui_config_changed`：

1. `session.set_desired(resolve(config.popup_ui))`  
2. hide webview **与** winui（互斥清理）  
3. 不 ensure  

- [ ] **步骤 3：Commit**

```bash
git add src-tauri/src/app/popup_ui src-tauri/src/ui/config.rs
git commit -m "fix(popup): WinUI ensure 失败会话回退 webview 且不写回配置"
```

---

## 任务 12：打包、CI、文档门禁

**文件：**
- 修改：`src-tauri/tauri.conf.json` 或 bundle 资源钩子（把 `native/windows/popup` Release 输出拷入 resources）  
- 修改：`.github/workflows/ci.yml`（Windows job）  
- 修改：`native/README.md`  
- 修改：`docs/agent/architecture-notes.md`  
- 修改：`README.md`、`docs/roadmap/progressive-development-plan.md`（若提及弹窗技术）  
- 修改：`AGENTS.md` + `CLAUDE.md`（架构摘要增加 `popupUi` / `native/windows/popup` 一句；两处同步）  
- 规格状态：`docs/superpowers/specs/2026-07-24-winui-native-translation-popup-design.md` 可标「实现中/已实现」由执行阶段收尾时更新  

### 12.A 打包

- `beforeBuildCommand` 或 `build.rs` / 脚本：`dotnet publish … -c Release`  
- Tauri `bundle.resources` 包含托管输出目录（DLL、runtimeconfig、WASDK 自包含文件）  
- 运行时查找顺序：1）可执行文件旁 `popup-native/` 2）dev 时 `native/windows/popup/bin/...`

### 12.B CI

```yaml
# Windows job 增加
- run: dotnet build native/windows/popup/Shizi.Popup.csproj -c Release
- run: dotnet test native/windows/popup/Shizi.Popup.Tests/Shizi.Popup.Tests.csproj -c Release --no-build
- run: cargo test --manifest-path src-tauri/Cargo.toml
```

非 Windows job：不编译 C#；Rust 中 `#[cfg(windows)]` 模块不参与，`popup_ui` 非 Windows 恒 webview。

### 12.C 架构文档要点（写入 architecture-notes）

- UI 层增加可选 WinUI 弹窗；默认 WebView  
- `AppConfig.popupUi`；`app/popup_ui` + `app/popup_bridge`；`native/windows/popup`  
- 切换下次唤起；ensure 失败会话回退  

### 12.D 手工验收清单（执行阶段勾选）

1. 默认 webview 与现网一致  
2. 设置改 winui → 保存 → **下次**划词出原生窗  
3. 多服务流式 / 取消 / 重试 / 语言切换 / 图钉 / 设置 / 截图译 / 关窗 hide  
4. 两后端不同时可见  
5. 模拟 ensure 失败（错误路径 DLL）→ 回退 webview，config 仍为 winui  
6. 非 Windows CI 绿  

- [ ] **步骤：实现文档与 CI + Commit**

```bash
git add native/README.md docs/agent/architecture-notes.md README.md docs/roadmap AGENTS.md CLAUDE.md .github/workflows src-tauri
git commit -m "docs(ci): WinUI 弹窗打包说明、架构文档与 CI"
```

---

## 任务分组与建议提交节奏

| 组 | 任务 | 主题 |
|----|------|------|
| A 配置与设置 | 1–2 | `popupUi` 端到端配置面 |
| B Rust 弹窗抽象 | 3–4 | trait / session / facade，webview 零回归 |
| C Bridge | 5–7 | 协议、分发、双投 |
| D 原生宿主与 UI | 8–10 | hostfxr、状态机、完整 UI |
| E 稳健与收尾 | 11–12 | 回退、打包、CI、文档 |

每任务结束必须：相关测试绿 + 一次原子 commit（任务 8 允许拆壳/宿主两个 commit）。

---

## 规格覆盖自检

| Spec 章节 | 对应任务 |
|-----------|----------|
| §1–2 目标/并行/默认 webview | 1–2, 4 |
| §3 架构 PopupUi + Bridge | 3–7 |
| §4 native 目录 | 8, 12 |
| §5 生命周期 ensure/show/hide/预创建/切换 | 4, 8, 11 |
| §6 Bridge 契约与状态机 | 5–7, 9–10 |
| §7 配置/设置/回退 | 1–2, 11 |
| §8 UI 对等清单 | 10 |
| §9 错误处理 | 6, 9, 11 |
| §10 测试策略 | 各任务 TDD + 12 手工清单 |
| §11 打包 CI | 8 澄清 + 12 |
| §12 文档门禁 | 12 |
| §14 验收标准 | 12.D |

**占位符扫描：** 无「TODO/待定/类似任务 N」；未决项已在文首写死。  
**类型一致性：** `popup_ui` / `popupUi` / `PopupUiKind::{Webview,WinUi}` / Bridge `bridgeVersion: 1` / 请求 type 字符串全文统一。

---

## 风险与执行注意

1. **Windows 回调栈建 WebView 死锁**规则继续有效；WinUI ensure 亦勿在托盘同步回调硬等 STA 初始化——必要时 `thread::spawn` 与现 `show_popup` 一致。  
2. **hostfxr 路径**在 dev/release/安装后三套布局，查找失败要有明确日志。  
3. **不要**在 C# 复制翻译协议；只 UI + Bridge。  
4. 执行阶段开始前用 `AskUserQuestion` 选子代理驱动 vs 内联（项目 L 档强制），不得默认内联。
