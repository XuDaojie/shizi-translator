# 架构关键点

> 模块行为与目录索引的参考说明。改相关子系统时按需阅读。协作流程（含 brainstorm / spec / plan 取舍）以 `AGENTS.md` / `CLAUDE.md` 为准。

## 分层与窗口

- **核心层（Rust）**：翻译业务、配置、provider 抽象（LLM/MT 平级）、划词、OCR、历史、日志。
- **UI 层**：① 翻译弹窗（见下「弹窗双后端」）② 设置页 `settings` → `settings.html` ③ 截图 overlay（纯静态，永久不迁 Vue）。历史面板右侧复用 `SourceCardView` / `ResultCardView` / `LanguageToolbar`。
- **约束**：核心逻辑不进前端 / 不进原生弹窗 UI 层；UI 模块互不耦合。设置 / OCR / overlay **始终** WebView。

## 弹窗双后端（WebView | 原生 winui）

规格：`docs/superpowers/specs/2026-07-24-winui-popup-backend-design.md`；计划：`docs/superpowers/plans/2026-07-24-winui-popup-backend.md`。

### 契约与目录

| 概念 | 说明 |
|------|------|
| `PopupBackend` | trait：`ensure_created` / `show` / `hide` / `destroy` / `publish`（`app/popup_backend/trait_api.rs`） |
| `PopupHost` | 进程级调度：持有 `Box<dyn PopupBackend>` + `PopupViewModel`；可选 `degraded_from_winui` |
| `WebviewPopupBackend` | 包装现网 `popup_window`；`publish` no-op（前端仍收 `translation:event`） |
| `WinuiPopupBackend` | **路径 B：Win32 表面**（`WS_POPUP` + `WS_EX_TOOLWINDOW` + DWM 圆角）；配置枚举值仍为 `winui`；**未依赖 WinAppSDK / XAML Runtime**；GDI 自绘对齐 Open Design WinUI3 原型（标题栏 / 源文卡 / 语言栏含列表 / 结果卡 / 状态栏，宽 468，Fluent 浅色 token；见 `docs/superpowers/specs/2026-07-24-winui-popup-fluent-align-design.md`） |
| feature | `popup-winui`（`Cargo.toml` default 含此项）；`--no-default-features` 仅 WebView |

业务主路径（划词 / 截图译 / 托盘打开）一律经 `popup_backend::with_host` → `PopupHost`，禁止绕过。

### 配置与切换

- 字段：`AppConfig.popup_ui_backend`（JSON camelCase：`popupUiBackend`），取值 `"webview" | "winui"`，**默认 `webview`**；未知值 `normalized` 回退 webview。
- 设置页（**仅 Windows**）「翻译弹窗 UI」：改配置后需**重启**生效（v1 无热切换）。
- 非 Windows：`resolve_popup_backend_kind` 恒为 `Webview`；配置可读写不崩溃。

### 启动选用与降级

```
lib setup
  → resolve_popup_backend_kind(config, POPUP_WINUI_FEATURE, cfg!(windows))
  → create_host_with_winui_fallback(create_backend)
  → manage(Mutex<PopupHost>)
```

- 仅当配置 `winui` **且** feature + Windows 时创建 `WinuiPopupBackend`。
- `ensure_created` 失败：同进程 `replace_backend(Webview)` + 一次性系统 dialog（路径 B 成功时不弹；dialog 文案仍可引导 Runtime 页，兼容未来路径 A）。
- `windowPrecreate.*.popup` 经 `host.ensure_created` 作用于**当前** backend。

### 生命周期（与 backend 无关）

- 弹窗关 = **hide** 常驻；设置 / OCR 关 = **销毁** WebView；托盘退出才结束进程。
- 原生路径关闭语义同样 hide，不销毁 HWND（destroy 仅切换/退出路径）。

### 开发依赖（本机 / CI）

| 依赖 | 用途 |
|------|------|
| Windows 10 / 11（x64） | 产品与原生弹窗目标平台 |
| Node.js + Rust stable + WebView2 | Tauri 主栈；设置 / OCR / WebView 弹窗 |
| `popup-winui` feature | 默认开启；CI `cargo test` / `cargo build` 带 default features |
| **不强制** Windows App Runtime / WinAppSDK / .NET | 路径 B 为纯 Win32 + 现有 `windows` crate；无需 XAML SDK 安装步骤 |

本机加速纯逻辑测：`cd src-tauri && cargo test --no-default-features`。

### 弹窗 backend 内存对照

**场景：** 仅托盘 + 弹窗预建并 hide，静置 ≥30s 后稳态；**不含**打开设置/OCR 的峰值。  
**指标：** Working Set（工作集）、Private Bytes（专用工作集）。  
**CI：** 无数值 gate；人工期望原生常驻不差于 WebView。

| 日期 | 版本 / commit | backend | WS (MB) | Private (MB) | 备注 |
|------|---------------|---------|---------|--------------|------|
| — | — | webview | 待本机实测 | 待本机实测 | 预建 hide 稳态 |
| — | — | winui（路径 B） | 待本机实测 | 待本机实测 | 预建 hide 稳态 |

采集命令（两 backend 各启动一轮，静置后再采）：

```powershell
# 进程名以实际 exe 为准（dev 可能为 shizi / shizi-translator 等）
Get-Process shizi* -ErrorAction SilentlyContinue |
  Select-Object Name, Id,
    @{N='WS_MB';E={[math]::Round($_.WorkingSet64/1MB,1)}},
    @{N='PM_MB';E={[math]::Round($_.PrivateMemorySize64/1MB,1)}}
```

## 托盘与窗口生命周期

- **三分法**：`main` 关窗 → `prevent_close` + `hide()`（用过后常驻）；`settings` / `ocr` 关窗 → **真正销毁**；托盘「退出」才结束进程。无窗时 `ExitRequested`（无 exit code）`prevent_exit` 以托盘驻留。
- **`windowPrecreate`**（`AppConfig`，设置 UI 不暴露）：`manual` / `autostart` 各含 `popup` / `overlay`。默认：手动 `popup=true, overlay=false`；自启双 `false`。启动按 `is_autostart_process()` 取对应对；用到时 `ensure` 再建。规格：`docs/superpowers/specs/2026-07-22-window-precreate-by-launch-mode-design.md`。
- `main`：运行时 `WebviewWindowBuilder`（不在 `tauri.conf` 静态声明）；`popup_window::ensure_popup_exists` / `show_popup`。手动且 `popup=true` 时 setup 预建；前端 `TranslationPopup` ready 后 show（约 2s 超时）；`--autostart` 且未预建时无窗，首次划词/托盘再创建。热唤起：`NearCursor` / `Restore`。
- 弹窗：`decorations(false)` + `transparent(true)` + `resizable(false)`；`.toolbar` 用 `data-tauri-drag-region`；`.popup` 宽 420px；高度 `usePopupHeight` + `ResizeObserver` 动态 `setSize`（宽 452，高 h+32，上限屏高 80%）。权限见 `capabilities/default.json`。
- Overlay：按 `windowPrecreate.*.overlay` 是否启动预建；`open_overlay` 已存在则 `reload`，否则 build；用完 hide 复用。
- 设置 / OCR：启动不预创建；关闭即销毁。截图识别前 `hide_ocr_window` 仍只 hide。
- **冷启动 splash**（settings / ocr / translate）：入口 HTML 内联 splash；`dismissBootSplash`。`main` hide 再开不重放；settings / ocr 重建再走 splash。

## 服务、配置与协议

- 事实来源：`config.json` 的 `services[]`（`protocol` / `endpoint` / `model` / `apiKey` / `enabled`）。旧单 provider 路径已废弃。
- 协议 id：`openai_chat` / `claude_messages` / `mock` / `microsoft_edge`。映射在 `core/translation/protocol.rs` 的 `provider_for_service`；未知协议报错。`microsoft_edge` 经 `BatchTranslateProvider` + `StreamingAdapter` 适配流式。
- `AppConfig` 另含 `updateChannel`（`stable`/`beta`）、`autoCheckUpdate`（默认 `true`）、`launchAtLogin`（默认 `false`）；前后端经 `projectToAppConfig` / `syncFromBackend` 同步。
- 开机自启：`launchAtLogin` → `app/autostart.rs` 写 HKCU `...\Run\Shizi`（命令带 `--autostart`）；`save_app_config` 与启动 setup 均同步；托盘/关窗 hide 为硬编码产品行为，设置页不再提供「最小化启动 / 托盘显隐 / 关闭行为」开关。
- 设置页挂载 `settings.syncFromBackend()`：后端 `services` 空 → 前端 `projectToAppConfig` 覆盖写回；非空 → `mergeBackendIntoServices` 按 id 合并（后端覆盖 enabled/apiKey/endpoint/model/protocol；前端保留 prompts/keyStatus/chainOfThought/pulledModels/note）。
- `save_app_config` 后广播 `app-config:changed`；弹窗同步卡片（翻译中不新增未参与批次的服务卡）。
- Dev-only：`ServiceMeta.protocols.length === 0` 与 `<DevOnly>`（`import.meta.env.DEV`）在 release 隐藏、dev 可见。自动检查更新、思维链已落地，**不再**列为 wip / DevOnly（主题 / 反思等仍可能 DevOnly）。

## 国际化

- `interfaceLanguage`：`auto` 跟 OS，未知回退 `zh-CN`；事件 `interface-language:changed { locale, revision }` 即时同步、不 reload。
- 前端静态 `zh-CN`/`en-US`，其余内置动态 chunk；用户包 `<app_config_dir>/lang/*.json`（≤1 MiB），回退链 user → 同 locale 内置 → zh-CN。
- 翻译语言 19 种 + 源语言 `auto`；LLM 用稳定英文语言名；Edge 严格映射，未知 code 报错。
- 默认目标语言：`AppConfig::default` 读 OS；`normalized` 兜底 `FALLBACK_TARGET_LANG = "zh-CN"`。

## 批次翻译与弹窗

- 启用服务保序并发；session = `{batch_id}:{service_id}`；事件带 `serviceInstanceId/serviceName/serviceType/protocol`。
- `initCards` 预建占位；冷启动可用 pending 原文，热窗以 `Started.sourceText` 为准 + revision 防迟到覆盖。
- 结果卡默认 `collapsed`；首非空正文 / failed / finished 展开；用户折叠本 batch 优先（`collapseUserOverride`）；`getCard` 按 `serviceInstanceId` 复用。
- 语言下拉：inline combobox（`.lang-picker`），非浮层。
- 语种检测：`Finished.detectedSourceLang`（LLM 首行解析或 MT `detectedLanguage`）→ 前端 `.lang-badge`。

## 快捷键

- `Alt+D` 划词翻译：主键释放后等修饰键全松再 Ctrl+C；成功取词才 show 弹窗。
- `Alt+S` 截图 OCR 翻译；独立文字识别默认无快捷键（托盘入口；用户可在设置绑定，不翻译、不写历史）。
- `CapturePurpose`：`Translate` | `RecognizeOnly`，在 `submit_capture_region` 分叉。
- 启动注册 best-effort，冲突记入 `shortcut_conflicts`；保存路径 all-or-nothing。新快捷键须同步 `capabilities/default.json`。

## 前后端通信（摘要）

- 翻译/配置：`start_translation`、`take_pending_source_text`、`get_app_config`、`save_app_config`、`get_shortcut_conflicts`
- Overlay：`get_capture_frame_meta` / `get_capture_frame_bytes` / `submit_capture_region` / `cancel_capture`
- 日志：`write_frontend_log` / `export_logs`；Edge：`save_edge_translate_env`
- 更新：`check_for_update`（可选 `channel`；缺省读 `AppConfig.updateChannel`）；启动 `spawn_startup_update_check`（`autoCheckUpdate` 时系统 dialog + `open_url`）。通道仅 `stable`/`beta`；CI 滚动 `nightly`（`.github/workflows/nightly.yml`，tag `nightly` 非 semver；包版本 `*-nightly.*`）。`evaluate_check` 对当前版本 pre 首段为 `nightly` 直接 `up_to_date`，避免 semver 误报「可升级到同号正式版」。
- 事件：`translation:event` → `Started` / `Delta` / `Finished` / `Failed`

## 历史与日志

- 历史：`HistoryStore` → `history.sqlite3`；统一翻译入口写 session/result；设置页 `list_translation_history` / `clear_translation_history`。
- 日志：`logs/Shizi.log`（tauri-plugin-log）与 `frontend.log`（append command）分文件；`logLevel` 可运行时切换；API Key 与正文脱敏；失败 best-effort。

## 目录索引（实现时优先打开）

| 领域 | 路径 |
|------|------|
| 装配 / 托盘 / 快捷键 | `src-tauri/src/lib.rs`、`app/` |
| 弹窗双后端 | `src-tauri/src/app/popup_backend/`（`host` / `webview` / `winui/`） |
| 配置 | `src-tauri/src/core/config/` |
| 翻译 / 协议 | `src-tauri/src/core/translation/` |
| LLM / MT | `src-tauri/src/core/llm/`、`core/mt/` |
| 检查更新 | `src-tauri/src/core/update/`、`ui/update.rs` |
| 截图 / OCR | `src-tauri/src/core/capture/`、`core/ocr/`、`ocr_translation.rs` |
| UI commands | `src-tauri/src/ui/` |
| 翻译弹窗前端（WebView） | `frontend/src/popup/` |
| 设置页 | `frontend/src/` + `settings.html` |
