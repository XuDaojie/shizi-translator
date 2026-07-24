# Windows WinUI3 原生翻译弹窗（与 WebView 并行）

> 日期：2026-07-24  
> 状态：**实现完成（feature 分支 `feature/winui-native-translation-popup`）**  
> 规模：L（新子系统 / 跨栈：Rust 宿主 + C# WinUI3）  
> 实现备注：宿主采用 **subprocess + localhost TCP**（非进程内 hostfxr）；打包 / CI / 文档见任务 12 与 `native/README.md`。

## 1. 背景与目标

### 1.1 背景

Shizi 以 Tauri 2 + Vue 实现 Windows 优先的翻译体验，翻译弹窗为 WebView（`frontend/src/popup/` + `popup_window.rs`）。Tauri 理论上可跨平台；产品路线为：

1. **各平台先用 WebView 弹窗**保证兼容；
2. **需要时再以原生 UI 优化**（Windows 先行 WinUI3；macOS / Linux 后置）。

本规格定义 Windows 上与现有 Vue 弹窗**并行**的 WinUI3 实现，以及与 Rust 核心的同进程对接方式。

### 1.2 目标

1. Windows 提供与现有 Vue 弹窗**功能对等**的 WinUI3 原生弹窗。
2. 与 WebView 弹窗**并行共存**，通过配置切换；**默认 `webview`**。
3. **业务核心仍在 Rust**（翻译批次、配置、划词、OCR、历史、快捷键、托盘）；WinUI3 **只负责弹窗 UI**。
4. 为日后其它原生窗口与 macOS / Linux 原生弹窗预留**目录与抽象扩展点**，但不预建空壳框架。

### 1.3 非目标（YAGNI）

- 不重写设置页 / OCR 窗 / overlay 为 WinUI。
- 不在 C# 内实现翻译协议、provider、OCR 核心。
- 第一版不做「保存后立刻热切换」两套窗状态无缝迁移（**下次唤起生效**）。
- 不把 WinUI 做成可独立分发的第二产品。
- 不一上来做跨窗口统一的 `INativeWindow` 大框架；第二个原生窗口出现后再抽共享 Bridge。

## 2. 产品决策摘要

| 项 | 结论 |
|----|------|
| 策略 | 并行双实现；WebView 保跨平台；Windows 可选原生优化 |
| 宿主 | 同进程：Tauri/Rust 托盘、快捷键、翻译核心不动 |
| 范围 | 第一版功能对等现有 Vue 弹窗 |
| 切换 | `config` + 正式设置页；默认 WebView |
| 视觉 | 布局与信息密度对齐现弹窗；控件用 WinUI/Fluent |
| 架构 | 进程内 .NET 宿主 + `PopupUi`；失败可降级子进程 IPC（同配置语义） |

## 3. 整体架构

```
┌─────────────────────────────────────────────────────────┐
│  Tauri 主进程 (Rust)                                      │
│  tray / shortcuts / config / translation / selection…   │
│                                                         │
│  PopupUi (trait)                                        │
│    ├─ WebviewPopupUi  → 现有 main WebviewWindow         │
│    └─ WinUiPopupUi    → hostfxr → C# WinUI3 窗体        │
│              │                                          │
│              └─ PopupBridge（命令下行 + 事件上行）         │
└─────────────────────────────────────────────────────────┘
```

- 「是否出弹窗、贴光标、开始翻译」仍由现有 Rust 路径决定。
- `ensure` / `show` 根据 `AppConfig.popupUi` 选择实现；**同一时刻只激活一种弹窗后端**。
- 主路径：**进程内 .NET 宿主**。若 WASDK 启动/打包证明不可行：同一 `PopupUi` + 几乎相同 Bridge 消息，改为**子进程本地 IPC**；设置项 `popupUi: winui` 语义不变。

## 4. 仓库落点

### 4.1 职责划分

| 位置 | 放什么 | 不放什么 |
|------|--------|----------|
| Rust（`popup_ui` / `popup_bridge`） | `PopupUi` 抽象、webview/winui 适配、生命周期、配置切换、FFI 契约 | 具体 XAML / 控件树 |
| `native/<platform>/...` | 该平台原生 UI **工程与实现** | 翻译协议、业务核心、仓库级抽象基类 |

`native/` **不是**抽象类目录；抽象在 Rust。

### 4.2 目录约定

```text
native/
  README.md                    # 平台目录约定、构建与 Bridge 说明
  windows/
    Shizi.Native.sln           # 可选
    popup/                     # 翻译弹窗（WinUI3）——本规格唯一实现
      Shizi.Popup.csproj
      ...
    # 以后：settings/、ocr/ 等按功能追加
  # macos/、linux/：有实现再落盘，不强制空目录
```

| 路径 | 职责 |
|------|------|
| `src-tauri/src/app/popup_ui/` | `PopupUi` trait、webview/winui 适配与切换 |
| `src-tauri/src/app/popup_bridge/` | 与 WinUI 弹窗的 FFI/回调（第一版仅弹窗） |
| `native/windows/popup/` | WinUI3 翻译弹窗工程 |
| `native/README.md` | 目录约定与构建 |
| `AppConfig` + 设置页 | `popupUi` |

### 4.3 后续扩展

- 第一版：仅 `PopupUi` = Webview | WinUi(`native/windows/popup`)。
- 以后某窗口也要原生：Rust 新增对应 UI 端口或扩展 bridge 路由；`native/windows/<feature>/`；macOS 同理 `native/macos/<feature>/`。
- 多窗口共用协议时再抽 `ui_bridge` 或 `native/windows/bridge/`——**等第二个原生窗口再抽**。

## 5. 生命周期与 `show_popup` 改造

### 5.1 原则（与现网一致）

| 行为 | WebView | WinUI |
|------|---------|-------|
| 关窗 | prevent_close + **hide** | 同样 hide；不卸载整个 .NET |
| 退出 | 托盘退出结束进程 | 随主进程；host shutdown best-effort |
| 任务栏 | `skip_taskbar` | 同等（不抢任务栏） |
| 预创建 | `windowPrecreate.*.popup` | **同一配置项** 对两种后端生效 |

### 5.2 `PopupUi` 语义（窗体壳）

- `ensure()` — 后端就绪（WebView 建窗 / 加载 .NET 并建隐藏窗）
- `show(position_mode)` — `NearCursor` | `Restore`；显示并聚焦
- `hide()`
- `set_always_on_top(bool)` — 图钉
- `set_size(w, h)` — 逻辑像素；WinUI 侧按 DPI 换算
- `is_available() -> bool` — 宿主失败时 false，供回退

翻译命令与事件走 **Bridge**，不全部塞进 trait。

### 5.3 调用时机

1. **启动**：若 `windowPrecreate` 要求预建 popup → `ensure()`（按当前 `popupUi`）。WinUI `ensure` 失败 → 日志 + **会话内回退 webview** 再 `ensure`。
2. **划词成功 / 托盘打开 / 其它唤起**：`ensure` → 推送原文/pending → `show`。
3. **截图译**：现逻辑 hide 弹窗再 overlay；WinUI 同样 `hide()`，不销毁宿主。
4. **`popupUi` 变更**：旧后端 `hide()`；**不在保存瞬间热切**；**下一次** `ensure`/`show` 按新值。进行中翻译不强制 cancel；迟到事件靠 batch/revision 丢弃（对齐现前端）。
5. **`windowPrecreate.popup == false`**：启动不 `ensure`；首次唤起再建。

### 5.4 定位与尺寸

- **定位**：复用 `compute_popup_position`；Rust 算逻辑坐标后设 WinUI 窗位置。
- **尺寸**：UI 侧测量后上报（WebView 今日 `setSize`；WinUI 布局后 `report_content_size`）；上限约工作区高度 80%。
- **宽度**：内容区约 420；阴影/圆角在 C# 侧处理，外轮廓接近，不要求与 WebView 像素级同一装饰实现。

### 5.5 双后端互斥

- 全局激活 backend = 配置值（或会话回退值）。
- 只 show 激活后端；**禁止**同时可见两个翻译弹窗。

### 5.6 .NET 宿主

- **懒加载**：首次需要 winui 的 `ensure` 时再 hostfxr 加载。
- **进程级单例**：加载一次，多次 show/hide。
- **退出**：先 hide，再尝试 shutdown 托管运行时（失败不挡退出）。

## 6. Bridge 契约

### 6.1 定位

- 弹窗 UI 专用通道，不是第二套业务核心。
- WebView 继续 Tauri `invoke` / `listen`；WinUI 走 Bridge。
- 语义对齐现有弹窗已用能力。
- 进程内：C ABI + 回调；消息体优先 **JSON UTF-8**，字段与现事件同构。
- API Key 等不得进 Bridge 明文；配置推送字段裁剪/脱敏与前端一致。
- `bridgeVersion: 1`；未知类型记日志并忽略。

### 6.2 UI → Rust（对齐现 invoke）

| 请求 | 说明 |
|------|------|
| `start_translation` | `{ text }` |
| `cancel_translation` | |
| `retry_translation` | |
| `set_session_languages` | `{ sourceLang, targetLang }` |
| `open_settings` | |
| `trigger_ocr_translation` | 内部仍 hide 弹窗 |
| `save_edge_translate_env` | Edge 相关 |
| `take_pending_source_text` | 冷启动 pending |
| `get_app_config` | 初始化卡片/语言等 |
| 界面 locale | 随 config 或等价请求 |
| `report_content_size` | `{ width, height }` 逻辑像素 |
| `ready` | 首帧可交互；冷启动约 2s 超时仍 show（对齐现 readyGate） |

图钉优先走 `PopupUi.set_always_on_top`，不强制 Bridge。  
收藏/书签：UI 内「功能开发中」，不新增 Rust API。

### 6.3 Rust → UI

| 推送 | 说明 |
|------|------|
| `translation_event` | 与 `translation:event` **字段同构**（Started/Delta/Finished/Failed + serviceInstanceId 等） |
| `app_config_changed` | 刷新启用服务卡；翻译中不新增未参与批次的卡 |
| `interface_language_changed` | 重载界面文案 |
| `show_context`（可并入 show） | sourceText、position、可选 sourceBadge |
| 会话语言 | 与 Started / 默认目标语一致时同步 |

### 6.4 事件状态机（C# 必须对齐）

对照 `useTranslationEvents` / `cardConfigSync`：

- 按启用服务保序 `initCards`。
- `getCard(serviceInstanceId)` 复用；单服务 Failed 不影响其它。
- 结果卡默认 collapsed；首非空 / failed / finished 展开；用户折叠本 batch 优先。
- batchId / revision 防迟到 Started 覆盖新原文。
- 冷启动：pending 原文 + ready 后补齐；**不无限 buffer 所有 Delta**。

### 6.5 不走 Bridge

划词、OCR 识别、历史写入、托盘、设置页保存：仍在既有 Rust / 设置 WebView。弹窗只收 `app_config_changed`。

## 7. 配置、设置页、回退

### 7.1 配置

```json
{
  "popupUi": "webview"
}
```

- 类型：`"webview" | "winui"`；默认 `"webview"`；未知值 normalized 为 `"webview"`。
- 非 Windows：配置可读，**运行时强制 webview**；设置页禁用 WinUI 并说明仅 Windows。
- 不新增预创建开关；沿用 `windowPrecreate`。

### 7.2 设置页

- **正式版用户可见**（非 DevOnly）。
- 单选/下拉：WebView（默认）/ WinUI3 原生（Windows）。
- 说明：切换后**下次打开翻译弹窗生效**。
- i18n 补齐（至少与项目惯例一致的 zh-CN / en-US 等）。
- 具体面板位置实现时贴现有信息架构（启动与窗口 / 外观等）。

### 7.3 切换流程

1. 保存 → 写盘 → `app-config:changed`。
2. 更新期望 backend；当前后端 `hide()`；不必立刻建新后端。
3. 下次 `ensure`/`show` 按新值。
4. webview → winui：首次 ensure 再加载 hostfxr。

### 7.4 失败回退（会话级）

| 场景 | 行为 |
|------|------|
| winui ensure/宿主失败 | 日志；**本进程会话回退 webview** 并 ensure；**不写回 config** |
| 用户提示 | 至少日志；可选一次系统/托盘提示；不强制 |
| 再次保存 winui | 允许重试 ensure |
| 降级子进程 IPC | 仅进程内路径整体放弃时；用户仍选 `winui` |

## 8. WinUI UI 对等清单

### 8.1 窗口壳

无系统标题栏、顶栏拖动、圆角阴影、不进任务栏、图钉、内容宽约 420、高度自适应（上限约工作区 80%）、贴光标/恢复位置由 Rust 下发。

### 8.2 界面区块

| 区块 | 必须能力 |
|------|----------|
| 顶栏 | 图钉、截图译、设置；收藏/书签占位提示 |
| 原文卡 | 输入、自动增高、朗读、复制、来源徽章、检测语种徽章 |
| 语言栏 | 源/目标（含 auto）、交换 → `set_session_languages` |
| 结果区 | 多服务保序流式、折叠/展开、复制/朗读、失败重试、usage（若有） |
| 状态栏 | 就绪/翻译中/失败与取消等操作提示 |

### 8.3 界面语言与翻译语言

- 跟 `interfaceLanguage` / `interface-language:changed`。
- 文案 key 语义对齐前端 `popup.*`（实现可独立资源文件）。
- 翻译语言：19 种 + 源 `auto`；第一版允许 C# 镜像表，优先与 Rust/共享源一致并由测试锁定。

## 9. 错误处理（汇总）

| 场景 | 处理 |
|------|------|
| 单服务失败 | 仅该卡 Failed |
| Bridge 失败 | 状态栏/toast，不崩进程 |
| 宿主 ensure 失败 | 会话回退 webview |
| 配置切换中途 | 旧窗 hide；不强制 cancel 批次 |

## 10. 测试策略

| 层 | 内容 |
|----|------|
| Rust 单测 | `popupUi` 解析/默认/非法值；backend 选择与互斥；位置计算复用；Bridge 序列化字段 |
| C# 单测 | 事件状态机、语言交换、尺寸上报相关纯逻辑 |
| 手工验收 | 切换 webview↔winui；划词多卡流式；取消/重试；截图译；图钉；关窗 hide；非 Windows 构建 |
| CI | Windows：`dotnet build` + `cargo test`；其它 OS 跳过 native，Rust `#[cfg(windows)]` |

## 11. 打包与 CI（要点）

- Windows App SDK：dev 可跑、release 可安装（自包含 vs 框架依赖由实现计划选型）。
- Tauri bundle 打入 C# 输出与必要运行库。
- 非 Windows CI 不编译 WinUI 工程。

## 12. 文档门禁（实现收尾）

- `docs/agent/architecture-notes.md`：双弹窗后端、`popupUi`、`native/windows/popup`。
- `native/README.md`。
- README / roadmap 若提及弹窗技术栈，补 Windows 可选 WinUI。

## 13. 建议实现切片（供 plan 拆分）

1. 配置 + 设置 UI + Rust `PopupUi` 骨架（webview 包装现逻辑）
2. Bridge 契约 + WinUI 壳 show/hide/定位
3. 事件状态机 + 原文/结果卡流式
4. 工具栏完整操作 + i18n + 高度
5. 打包/CI + 失败回退 + 文档

## 14. 验收标准

1. 默认 `popupUi: webview`，行为与现网一致。
2. Windows 设置可选 WinUI3；保存后**下次唤起**走原生弹窗。
3. WinUI 路径：划词/手动输入/多服务流式/取消重试/语言切换/图钉/设置/截图译/关窗 hide 均可用。
4. 两后端不得同时可见；切换与预创建配置语义正确。
5. WinUI 宿主失败时会话回退 webview，翻译主路径不永久不可用。
6. 非 Windows 构建与运行不依赖 WinUI 工程。
7. 架构文档与 `native/README.md` 已同步。
