# 独立设置页与独立翻译弹窗拆分 设计规格

> **关联架构文档：** [docs/architecture/ui-decoupling-proposal.md](../../architecture/ui-decoupling-proposal.md)
> **当前阶段：** 个人开发阶段，不考虑新老配置字段兼容（本地 config 可删后重建）。

## 目标

将当前「单 `main` 窗口同时承载翻译区与内嵌设置面板」的形态，拆分为两个职责独立的窗口：

1. **设置页（主窗口 `main`）**：承载 Provider 配置、目标语言、窗口策略开关等，未来扩展设置项的载体。应用启动入口。
2. **翻译弹窗（`translation-popup`）**：输入框 + 来源徽章 + 输出区 + 操作栏，按需唤起、跟随光标定位。

拆分后应用无常驻「翻译主窗口」；设置页即主窗口，翻译以独立弹窗形式唤起。本次只拆窗口与文件，**不抽出 `TranslationPopupPort` trait**（留给里程碑 3 Slint 替换时再做）。

## 非目标

- 不抽 `TranslationPopupPort` / `TranslationEventSink` trait。
- 不触动 `TranslationService`、`LlmProvider`、翻译事件类型、config 存储机制、划词/OCR 核心链路。
- 不做新老配置字段兼容（个人开发阶段，config 直接重建）。
- 不引入 Slint（里程碑 3）。
- 不改变 overlay 的框选/OCR 业务逻辑，仅改其窗口创建策略。

## 现状

- 单 `main` 窗口（480×360），`index.html` 同时含翻译区与 `#settingsPanel`（点「设置」按钮显隐）。
- `main.js`（380 行）混合翻译逻辑与设置逻辑；`style.css`（233 行）混合两套样式。
- `web_popup.rs`（251 行）混合翻译 command、窗口显示、事件 emit 编排。
- `Alt+T`/`Alt+O` 触发翻译时**不主动 show 窗口**，依赖窗口已可见——拆分后必须修正。
- overlay 已是运行时 `WebviewWindowBuilder` 按需创建的独立窗口。

## 整体形态与窗口模型

| 窗口 | label | 创建机制 | 默认生命周期 | 职责 |
|---|---|---|---|---|
| 设置页（主窗口） | `main` | `tauri.conf.json` 预创建 | 启动按配置显隐；关闭即隐藏 | 配置管理 |
| 翻译弹窗 | `translation-popup` | 运行时 `WebviewWindowBuilder` | 默认预创建隐藏；可配运行时创建 | 翻译输入/展示 |
| 截图 overlay | `screenshot-overlay` | 运行时 `WebviewWindowBuilder` | 默认预创建隐藏；可配运行时创建 | 区域框选 |

**创建机制说明：** `tauri.conf.json` 只静态声明 `main`（设置页，固定预创建）。翻译弹窗与 overlay 都走运行时 `WebviewWindowBuilder`，差异仅在「创建时机」与「关闭语义」，由配置驱动：

- **预创建模式（默认）：** `setup` 阶段创建并 `hide()`，常驻；唤起时 `show()`+定位；`CloseRequested` → `prevent_close` + `hide()`。
- **运行时模式：** `setup` 不创建；唤起时 `WebviewWindowBuilder` 创建+定位；`CloseRequested` → 放行 `close()` 销毁。

**启动显隐（设置页主窗口）：** `setup` 阶段读 `AppConfig`，若已配置 provider（`is_configured()` 为真）则隐藏 `main`，否则显示 `main` 引导配置。

**窗口策略配置项（新增到 `AppConfig`）：**

```rust
pub struct AppConfig {
    // ...既有字段...
    pub popup_precreate: bool,   // 默认 true
    pub overlay_precreate: bool, // 默认 true
}
```

切换策略需重启应用生效（启动时读配置决定是否预创建）。

## 翻译弹窗唤起与定位

**唤起入口：**

- `Alt+T` 划词翻译：读选区 → `show_popup`（show+定位）→ 自动翻译。
- `Alt+O` 截图 OCR：overlay 框选 → OCR → `show_popup` → 自动翻译。
- 托盘「翻译」菜单 → `show_popup` 弹出空弹窗供手动输入。
- 弹窗内输入框 + 「翻译」按钮 → 手动翻译。

**修正现缺陷：** 快捷键触发翻译时必须主动 `show_popup`+定位，不再依赖窗口已可见。

**光标定位（PopupAnchor）：**

- 唤起时获取全局光标位置（Windows 用 `windows` crate 的 `GetCursorPos`，不引入新 crate）。
- 物理像素 ↔ 逻辑像素换算（复用 `css_rect_to_physical` 的逆向逻辑）。
- 多屏边界处理：弹窗宽高 + 光标位置，若超出所在显示器工作区右/下边界，向左/上偏移，避免跨屏或溢出。
- 定位逻辑抽成纯函数 `compute_popup_position(cursor: PhysPos, popup_size: LogicalSize, monitor_work_area: LogicalRect) -> LogicalPos`，便于单测。

**`web_popup.rs` 重组：**

- 保留：`start_translation` / `cancel_translation` / `retry_translation` / `take_pending_source_text` commands、`emit_translation_event`、`show_translation_error`。
- 新增：`show_translation_popup(app, config)` —— 封装 show+定位（预创建模式 show，运行时模式创建）。
- `shortcuts.rs` 在触发翻译前调用 `show_translation_popup`。

## 设置页（主窗口）拆分

**从主窗口剥离：** `index.html` 的 `#settingsPanel` 整块搬到 `settings.html`；翻译区搬到 `translate.html`；`main.js`/`style.css` 按窗口拆成 `translate.*` / `settings.*`。

**设置页内容：**

- 目标语言。
- Provider 选择（openai-compatible / claude / mock）+ 对应表单（API Key / Base URL / Model / Timeout / Claude 的 Enable Thinking）。
- **新增「窗口策略」分组：** `popup_precreate` / `overlay_precreate` 两个复选框。
- 保存配置按钮 + 状态提示；API Key 明文警告保留。
- 策略切换保存后提示「重启应用生效」。

**后端 command 桥：**

- `get_app_config` / `save_app_config` 复用，`AppConfig` 新增两字段。
- 新增 command `open_settings`：`show_window("main")` + `set_focus`，供弹窗「设置」按钮调用。
- `config.rs` 扩展为设置页 command 出口，仍薄包装 `ConfigStore`。

**设置入口（都 show 主窗口，不新建）：**

- 托盘「设置」菜单 → `show_window("main")`。
- 翻译弹窗「设置」按钮 → `open_settings` command。
- 双击托盘 → `show_window("main")`（改自当前 toggle 主窗口）。

**耦合边界：** 设置页只改 config，不直接操作弹窗状态。策略切换需重启生效。

## 统一窗口管理模块

新增 `src-tauri/src/app/popup_window.rs`，封装弹窗与 overlay 的双模式管理（设置页 `main` 仍由 `window.rs` 管理，职责分离）：

- `ensure_popup_window(app, config)` —— 预创建模式下启动时调用，创建并隐藏弹窗。
- `show_popup(app, anchor, config)` —— 预创建模式 show+定位 / 运行时模式创建+定位。
- `hide_or_close_popup(app, config)` —— 预创建 hide / 运行时 close。
- overlay 同理：`ensure_overlay` / `open_overlay` 改造为按 `overlay_precreate` 分支。

**关闭事件挂载：** 弹窗 `on_window_event(CloseRequested)` → 预创建模式 `prevent_close + hide`，运行时模式放行 `close`。设置页 `main` 始终 `prevent_close + hide`。

**capabilities 同步：** `default.json` 的 `windows` 数组加 `translation-popup`（overlay 已有）。

## 前端文件拆分

| 文件 | 职责 |
|---|---|
| `frontend/translate.html` / `translate.js` / `translate.css` | 翻译弹窗 DOM 与交互 |
| `frontend/settings.html` / `settings.js` / `settings.css` | 设置页 DOM 与交互 |
| `frontend/overlay.html` 等 | 不变 |

**Tauri 路由：** `frontendDist` 仍是 `../frontend`，窗口 `url` 指向各自 HTML：`main`→`settings.html`，`translation-popup`→`translate.html`，overlay→`overlay.html`。

**职责边界（遵循 proposal）：**

- 弹窗前端不拼 provider 请求，只发 `start_translation` command。
- 设置前端只读写 config，不触碰翻译状态。
- 两者不互相直接通信；设置改 config，弹窗下次翻译时读 config。

**清理：** 拆分完成后删除原 `index.html` / `main.js` / `style.css`。

## 测试与验收

**Rust 单元测试（TDD，先写失败测试）：**

- `AppConfig` 新增 `popup_precreate` / `overlay_precreate` 字段的序列化/反序列化测试（正向，不测新老兼容）。
- `compute_popup_position` 纯函数测试：光标在屏中部（正常）、光标近右边界（左移）、光标近下边界（上移）三例。
- `AppConfig::is_configured()` 测试：有 active provider 且 api_key 非空 → true；缺 key → false。

**前端语法检查：** `node --check frontend/translate.js`、`node --check frontend/settings.js`。

**手动验证（mock provider，桌面环境）：**

- 首次未配置启动 → 设置页主窗口显示；配置保存后重启 → 主窗口隐藏。
- `Alt+T` 划词 → 弹窗在光标附近出现并自动翻译；多屏边界不溢出。
- `Alt+O` 截图 OCR → 弹窗出现并翻译，徽章「来自 OCR」。
- 托盘「翻译」→ 空弹窗手动输入翻译。
- 托盘「设置」/ 弹窗「设置」按钮 → 设置页主窗口显示。
- 切换 `popup_precreate=false` 重启 → 弹窗运行时创建、关闭即销毁；`=true` → 预创建、关闭即隐藏。
- overlay 同理验证两种策略。
- 翻译 finished/取消/失败/清空 → 徽章隐藏（回归不破坏现有行为）。

**验收标准：**

- 设置页与翻译弹窗完全独立，无共享 DOM/JS。
- 快捷键触发时弹窗主动 show+定位（修正现缺陷）。
- 窗口策略可配置，两种模式均可工作。
- `cargo test` 全绿、`cargo build --release` 无警告、前端语法检查通过。

**不触动：** `TranslationService`、`LlmProvider`、config 存储机制、翻译事件类型、划词/OCR 核心链路。

## 文件结构

| 文件 | 动作 |
|---|---|
| `src-tauri/src/core/config/types.rs` | 修改：`AppConfig` 加 `popup_precreate`/`overlay_precreate`、`is_configured()`、`from_env`/`normalized` 补默认 |
| `src-tauri/src/app/popup_window.rs` | 新增：弹窗/overlay 双模式窗口管理 |
| `src-tauri/src/app/window.rs` | 修改：设置页 `main` 的 show/close-to-hide 保留；双击托盘改 show 设置页 |
| `src-tauri/src/app/tray.rs` | 修改：菜单加「翻译」「设置」项 |
| `src-tauri/src/app/shortcuts.rs` | 修改：触发翻译前调用 `show_translation_popup` |
| `src-tauri/src/ui/web_popup.rs` | 修改：新增 `show_translation_popup`，重组 |
| `src-tauri/src/ui/config.rs` | 修改：新增 `open_settings` command |
| `src-tauri/src/ui/overlay.rs` | 修改：按 `overlay_precreate` 分支化 |
| `src-tauri/src/lib.rs` | 修改：注册 `open_settings`、`setup` 阶段按配置预创建/显隐窗口 |
| `src-tauri/tauri.conf.json` | 修改：`main` 窗口 url 指向 `settings.html` |
| `src-tauri/capabilities/default.json` | 修改：`windows` 加 `translation-popup` |
| `frontend/translate.html` / `translate.js` / `translate.css` | 新增：翻译弹窗 |
| `frontend/settings.html` / `settings.js` / `settings.css` | 新增：设置页 |
| `frontend/index.html` / `main.js` / `style.css` | 删除 |
