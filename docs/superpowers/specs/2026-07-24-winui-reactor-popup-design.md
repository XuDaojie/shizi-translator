# 翻译弹窗真 WinUI 3（Rust + windows-reactor）设计

## 背景

Shizi 翻译弹窗双后端中，配置值 `popupUiBackend: "winui"` 的实现长期锁定为 **路径 B：Win32 + GDI 自绘**。该路径无法提供系统级 Fluent 控件、Mica、标准焦点/无障碍行为，与 Open Design 高仿真原型 `#popup-winui3` 存在结构性差距。

微软 **windows-rs** 生态已提供官方推荐的 Rust 侧 WinUI 3 方案：

| 组件 | 作用 |
|------|------|
| [`windows-reactor`](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-reactor.md) | 声明式（React 风格）UI，**底层渲染真实 WinUI 3 控件** |
| [`windows-reactor-setup`](https://github.com/microsoft/windows-rs/blob/master/docs/crates/windows-reactor-setup.md) | `build.rs` 中暂存 Windows App SDK Runtime / 清单 |
| 既有 `windows` crate | Win32/WinRT API（OCR、截图等继续使用） |

UI 形态为 **纯 Rust 描述控件树**（`fn(&mut RenderCx) -> Element` + hooks），**无 XAML 配置文件**；亦 **非** GDI 手绘。与「C#/C++ + XAML」是同一 WinUI 3 运行时，不同前端语言/DSL。

本设计将 `winui` 后端的目标实现从路径 B 升级为 **路径 R（Reactor）**。

## 目标

1. 在 **仅 Windows** 上，当配置为 `winui` 时，翻译弹窗使用 **真实 WinUI 3**（经 `windows-reactor`），业务仍在 Rust 核心。
2. 保持现有契约：`PopupBackend` / `PopupHost` / `PopupViewModel` / `PopupUserAction`；设置 / OCR / overlay **始终 WebView**。
3. 视觉与信息架构对齐 Open Design `popup-winui3`（五区：标题栏 / 源文 / 语言栏 / 结果列表 / 状态栏；宽约 468；柿子橙 accent；尽可能 Mica）。
4. 失败可降级：Runtime 缺失或初始化失败 → 同进程 **WebView**（既有 `create_host_with_winui_fallback` 语义），并引导安装 Windows App Runtime。
5. **不引入 .NET / C# 工程**。
6. 主程序语言与 UI 描述语言均为 **Rust**（官方 windows-rs 推荐栈）。

## 非目标（v1）

- 不把设置页 / OCR / overlay 迁到 Reactor。
- 不实现 macOS / Linux 原生弹窗。
- 不做 backend 热切换（仍重启生效）。
- 不追求与 WebView（Bob 风）弹窗像素一致。
- 不把翻译协议、配置持久化、历史写入放进 UI 层。
- v1 不强制把 NSIS 改为捆绑完整 Runtime（优先 framework-dependent + 引导安装；self-contained 可作为后续发布选项）。
- 不在 v1 用 Reactor 重写托盘菜单（托盘保持现状）。

## 规模

**L 档**：新 UI 运行时栈 + 与 Tauri 消息循环共存 + 依赖/CI/发布变更；须独立 spec → plan → 编码阶段。

## 架构

### 总览

```
┌──────────────────────────────────────────────────────────┐
│  Tauri 主进程（Rust）                                      │
│  core: config / translation / history / …                │
│  app: tray / shortcuts / PopupHost                       │
│                                                          │
│  PopupBackend                                            │
│    ├─ WebviewPopupBackend  → main WebView（现状）         │
│    └─ WinuiPopupBackend    → 路径 R：windows-reactor     │
│         · bootstrap WinAppSDK                            │
│         · ReactorWindow / App 宿主翻译弹窗                 │
│         · publish → hooks state 驱动重绘                   │
│         · 控件事件 → PopupUserAction                       │
│                                                          │
│  低频：settings / ocr / overlay WebView                  │
└──────────────────────────────────────────────────────────┘
```

### 分层原则（不变）

1. **业务在 core**；Reactor 层只展示与输入。
2. **`PopupBackend` 是唯一弹窗宿主边界**（`ensure_created` / `show` / `hide` / `destroy` / `publish`）。
3. 同一时刻只激活一个 backend；`popupUiBackend` 重启生效。
4. 关窗语义：弹窗 **hide 常驻**；托盘退出才进程结束。

### UI 编程模型（官方 Reactor 方式）

- **界面结构**：Rust 中 `vstack` / `hstack` / `scroll_viewer` / `list_view` / `button` / `text_box` / `text_block` 等 builder 组成 `Element`。
- **状态**：`cx.use_state` / `use_memo` 等 hooks；`publish(PopupViewModel)` 将快照写入共享状态并触发 re-render。
- **事件**：`.on_click` 等 → 调用既有 `actions::handle_user_action`（或等价通道）。
- **外观**：优先系统 Fluent + `Backdrop::Mica`（若 API 可用）；品牌 accent（柿子橙）通过样式/资源在 v1 尽量贴近原型。
- **不是**：GDI `paint_*`；也不是独立 `.xaml` 文件。

### 与 Tauri 共存（关键风险 · 必须先 spike）

WinUI / Reactor 要求 **STA UI 线程** 与自身消息泵；Tauri/WebView2 也有消息循环。v1 采用下列策略，**以 spike 结果写死**：

| 策略 | 说明 | 优先 |
|------|------|------|
| **S1：同进程 + 专用 STA 线程跑 Reactor 消息循环** | `ensure_created` 时起线程 `bootstrap` + `ReactorWindow`/`App`；`publish` 经线程安全队列投递到 UI 线程 | 首选尝试 |
| **S2：同进程主线程集成** | 仅当 spike 证明可与 Tauri 共存且无死锁 | 备选 |
| **S3：降级** | spike 失败保留 WebView；GDI 路径 B 仅作过渡期可选 fallback（见下） | 失败兜底 |

**硬规则：**

- 禁止在全局快捷键回调同步栈里做重初始化（对齐现网 `show_popup` 线程策略）。
- `publish` 必须非阻塞；UI 更新投递到 Reactor UI 线程。
- `hide` / `show` 必须幂等，不销毁 Runtime（进程级 bootstrap 一次）。

### 模块落点（预期）

```
src-tauri/src/app/popup_backend/
  winui/
    mod.rs          # 对外 WinuiPopupBackend
    backend.rs      # trait 实现
    bootstrap.rs    # WinAppSDK bootstrap（替换「路径 B 恒 Ok」）
    reactor/        # 新建：UI 树、状态桥、窗口生命周期
      mod.rs
      app.rs        # 窗口 create/show/hide
      view.rs       # render(fn) 五区布局
      state.rs      # ViewModel ↔ hooks 桥
    ui.rs           # 路径 B GDI：迁移期保留或 feature 门控，见「路径 B 处置」
    actions.rs      # 尽量复用
```

`Cargo.toml`（方向，plan 写死版本）：

- `windows-reactor` / `windows-reactor-setup`：以 **windows-rs monorepo git 依赖或 crates.io 正式版** 为准（文档曾注明 reactor **未上 crates.io** 时用 git + 锁定 rev）。
- 现有 `windows = "0.58"` 可能需与 reactor 要求的 `windows` / `windows-core` 版本对齐；**以 spike 可编译版本为准统一升级**，避免双版本 ABI 冲突。
- feature：`popup-winui` 语义升级为「启用路径 R」；可选 `popup-winui-gdi` 过渡编译 GDI。

### 路径 B（GDI）处置

| 阶段 | 策略 |
|------|------|
| Spike / 迁移期 | GDI 实现可保留在 `#[cfg(feature = "popup-winui-gdi")]` 或源码旁路，便于对比 |
| 路径 R 主路径绿灯后 | **默认 `winui` = Reactor**；配置不再指向 GDI |
| 清理 | plan 末段任务删除 GDI 绘制大文件或移入 `legacy/`，更新架构文档 |

产品配置枚举 **仍为** `"webview" | "winui"`（不新增 `"reactor"` 字符串，避免设置页与文档二次迁移）。文档中称路径 R。

## 功能范围（弹窗 UI）

### 与 Open Design 对齐的信息架构

1. **标题栏**：品牌、钉/收藏/截图/书签/设置/主题（未接线可 stub）、最小化/关闭；可拖动。
2. **源文区**：多行文本（可编辑策略 v1：展示 + 复制；是否就地编辑触发重译由 plan 定，默认对齐现网：改语言重译、源文以会话输入为准）。
3. **语言栏**：源 / 交换 / 目标；列表选择（ComboBox / Flyout / ListView 择一，优先系统控件）。
4. **结果列表**：多服务卡片；状态、正文、model/tokens（规则同 `resultCardMeta`）；复制；取消/重试走状态区或卡片。
5. **状态栏**：状态文案 + 字数。

### 必须接线的 `PopupUserAction`

`Close`、`OpenSettings`、`CancelTranslation`、`Retry`、`CopyResult`、`SetSessionLanguages`。

### 视觉

- SSOT：`OpenDesignProjects/shizi` → `#popup-winui3` 与 `src/popup/winui3/*`。
- 宽约 **468** 逻辑像素；高度内容驱动或上限滚动（对齐现网体验）。
- Accent：**柿子橙**（`#D55A1F` / 深色变体按原型）；优先 Mica 窗口背景。

## 配置与生命周期

| 项 | 决定 |
|----|------|
| 字段 | `popupUiBackend`: `webview` \| `winui`，默认 **`webview`**（降低首发 Runtime 风险） |
| 切换 | 设置页（仅 Windows）→ 保存 → **重启生效** |
| 选用 | `winui` + feature + Windows → 尝试路径 R |
| 失败 | `ensure_created` Err → `replace_backend(Webview)` + 一次性 dialog + Runtime 下载页（既有 URL 策略） |
| 预建 | `windowPrecreate.*.popup` 仍经 `host.ensure_created` |

## 依赖与发布

| 项 | 决定 |
|----|------|
| 运行时 | Windows App SDK / Windows App Runtime（WinUI 3） |
| 部署模型 v1 | **framework-dependent** 优先；`windows-reactor-setup::as_framework_dependent()`（或文档等价 API） |
| 本机开发 | 安装对应 Runtime；文档写明版本钉扎 |
| CI | Windows job：安装 Runtime 或 self-contained 测试包；`cargo test` / `cargo build` 带 `popup-winui` |
| 非 Windows | 不编译 reactor UI；行为恒 webview |

## 测试策略

1. **单元**：ViewModel 桥、动作映射、语言交换规则（不启真实窗口）。
2. **集成 / 手动**：`popupUiBackend=winui` 冷启动 → 划词 → 多卡流式 → 换语言 → 复制 → 设置 → 关闭 hide → 再开。
3. **Spike 验收门**（未通过则不进全量 UI 对齐）：
   - 同进程或约定模型下弹出 Mica 窗；
   - `publish` 更新文本可见；
   - hide/show 稳定；
   - 与托盘/WebView 设置共存无死锁；
   - Runtime 缺失时降级 WebView。

## 分阶段交付

| 阶段 | 内容 | 出口 |
|------|------|------|
| **M0 Spike** | 依赖接入、bootstrap、最小窗、与 Tauri 共存模型写死 | 可演示计数器级窗 + 降级路径 |
| **M1 契约** | `WinuiPopupBackend` 切到 Reactor 生命周期；`publish`/动作贯通 | 源文 + 单结果卡 + 关闭/复制 |
| **M2 五区** | 对齐原型布局与语言列表 | 主路径完整 |
| **M3 抛光** | Mica/accent/多卡/tokens/滚动 | 目视接近原型 |
| **M4 清理** | 文档、CI、移除或隔离 GDI | 架构文档更新 |

## 风险

| 风险 | 缓解 |
|------|------|
| Reactor 与 Tauri 消息循环冲突 | M0 spike 否决门；失败则不宣称 winui=真 WinUI |
| `windows` 版本与 reactor 冲突 | 统一升级；feature 门控 |
| crates.io 未发布 | git 依赖 + 锁定 commit；跟进正式版 |
| Runtime 体积 / 用户未安装 | 默认 webview；dialog 引导；后续可选 self-contained |
| STA 线程安全 | 所有 UI API 仅 UI 线程；跨线程只传 ViewModel 快照 |

## 成功标准

1. 配置 `winui` 且 Runtime 可用时，弹窗为 **真 WinUI 3 控件**（可用 Inspect / 视觉确认系统按钮与 Mica，而非 GDI 矩形）。
2. 主路径：划词译、多服务结果、换语言、复制、设置、关闭 hide 可用。
3. Runtime 失败 → WebView 降级，应用不崩溃。
4. `cargo test`（Windows default features）通过；架构文档写明路径 R。
5. 代码中 UI 以 Reactor `Element` 树为主，**不再**依赖 GDI 作为默认 `winui` 实现。

## 与既有文档关系

| 文档 | 关系 |
|------|------|
| `2026-07-24-winui-popup-backend-design.md` | 双后端总 spec；本 spec **修订其「WinUI 实现 = 路径 B」** 为路径 R |
| `2026-07-24-winui-popup-backend.md`（plan） | 历史 plan；路径 R 须 **新 plan**，不直接当任务清单 |
| `2026-07-24-winui-popup-fluent-align-design.md` | GDI 精修；路径 R 落地后作视觉对照，GDI 实现退场 |
| `docs/agent/architecture-notes.md` | 编码阶段更新：winui = Reactor |

## 已确定的设计取舍

1. **栈**：Rust + **windows-reactor**（官方推荐），不引入 .NET。  
2. **UI 模型**：声明式 Rust 控件树，无 XAML 文件，非 GDI 绘制。  
3. **配置名**：仍为 `winui`；实现换路径 R。  
4. **降级**：失败 → WebView。  
5. **视觉 SSOT**：Open Design `#popup-winui3`。

## 留给 plan / spike 的未决项

1. 与 Tauri 共存的最终模型（S1/S2）以 **M0 实测** 写死。  
2. `windows` / reactor 的 **精确版本与 git rev**。  
3. 源文是否可编辑并触发重译（建议 v1 只读展示 + 复制，与现网输入源一致）。  
4. framework-dependent vs self-contained 发布默认值（开发用 framework；安装包策略 plan 定）。
