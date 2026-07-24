# Windows 翻译弹窗双后端（WebView | WinUI 3）设计

## 背景

Shizi 是 Windows 优先的大模型翻译工具（Tauri 2 + Vue/WebView2）。翻译弹窗是最高频界面，当前以 WebView 实现，并配合 `windowPrecreate` 与关窗 hide 做常驻，以换取热唤速度。

产品目标是在 Windows 上把弹窗体验做到接近原生（打开速度、视觉与输入跟手、常驻内存），同时保留 Web UI 服务低频页与跨平台路径。设置、文字识别等低频页可接受 WebView 峰值，且关闭即销毁；弹窗则更可能常驻，适合换成更轻的原生栈。

## 目标

1. **仅翻译弹窗**在 Windows 上可选 **WinUI 3（Rust 实现）**，优化打开速度、原生视觉与**常驻态内存**。
2. **设置 / OCR / overlay** 继续 WebView；关窗销毁策略不变。
3. 设置中可切换 **`webview` | `winui`**，便于开发期 A/B；WinUI 为 Windows 长期优化方向，WebView 为稳妥基线与跨平台同源路径。
4. 业务逻辑仍在 **Rust 核心**；弹窗 UI 只负责展示与用户输入。
5. **v1 跨平台**：只预留 `PopupBackend` 接口；macOS / Linux 继续 WebView 弹窗，未来再各自升原生。
6. **不引入 .NET**；运行时依赖为 **Windows App SDK（WinUI）** + 现有 **WebView2**（低频页）。
7. **CI**：GitHub Actions Windows runner 可构建并打出与现网一致的安装形态（NSIS / 现有 `tauri build` 产物）。

## 非目标（v1）

- 不全量迁移设置等 UI 到 WinUI。
- 不实现 macOS / Linux 原生弹窗。
- 不将 WinUI 弹窗拆成独立子进程（接口预留演进空间；v1 同进程内嵌）。
- 不强制改为 MSIX / Store 分发；不做应用内安装 Windows App Runtime 的复杂自更新。
- 不追求与 WebView 弹窗像素级视觉一致（主路径能力对齐优先，视觉以系统原生为准）。
- 不在 v1 做 backend 热切换（见配置节）。

## 规模

**L 档**：新子系统（双弹窗后端 + 配置切换 + 跨层 ViewModel），独立 spec → plan → 编码阶段。

## 架构总览

```
┌─────────────────────────────────────────────┐
│  Tauri 主进程（Rust）                         │
│  core: config / translation / history / …   │
│  app: tray / shortcuts / window lifecycle    │
│                                             │
│  PopupBackend (trait)                       │
│    ├─ WebviewPopupBackend  → 现有 main WebView │
│    └─ WinuiPopupBackend    → WinUI 窗（cfg windows）│
│                                             │
│  低频：settings / ocr / overlay WebView      │
│        （关闭即销毁）                          │
└─────────────────────────────────────────────┘
```

### 原则

1. **`PopupBackend` 是唯一弹窗宿主边界**  
   核心只通过该接口：`ensure/precreate`、`show(position)`、`hide`、`destroy`、推送视图状态、接收用户动作。
2. **同一时刻只激活一个 backend**  
   由配置决定；切换策略见下文（v1 重启生效）。
3. **平台策略**  
   - Windows：WebView 与 WinUI 两实现 + 用户可切换。  
   - 非 Windows：仅 `WebviewPopupBackend`；配置若为 `winui` 则忽略并回退 webview（可记日志）。
4. **进程模型（v1）**  
   WinUI 窗口在 **主进程内嵌**创建；不新增独立 exe。若后续证明消息循环/隔离不可接受，可在不改核心协议的前提下改为子进程宿主。

### 与现有生命周期的对齐

| 窗口 | 现状 | 本设计 |
|------|------|--------|
| 翻译弹窗 `main` | 关 = hide，可预建 | 两 backend 均保持 hide 常驻语义 |
| 设置 / OCR | 关 = 销毁 | 不变，仍 WebView |
| overlay | 按需 / 可预建 | 不变，仍 WebView |

## PopupBackend 接口（语义）

实现语言为 Rust；具体签名可在 plan 阶段落地，语义固定：

| 能力 | 说明 |
|------|------|
| `ensure_created` | 预建或懒建；失败返回错误 |
| `show(mode)` | `NearCursor` / `Restore`；展示并 focus |
| `hide` | 幂等；不销毁 |
| `destroy` | 真正释放（切换 backend / 进程退出） |
| `is_visible` / `is_alive` | 调度与自检 |
| `publish(PopupViewModel)` | 核心 → UI 单向状态（快照或增量） |
| `on_user_action(...)` | UI → 核心：重译、取消、换语言、复制、关闭、打开设置等 |

### PopupViewModel（概念）

与现网 `translation:event` 及弹窗本地状态对齐的视图模型，至少覆盖：

- **session**：sourceText、source/target lang、batch 状态、sourceType 等  
- **cards[]**：serviceInstanceId、name、protocol、model、status、text、usage、error  
- **chrome**：是否翻译中、是否可取消等  

WinUI 与 WebView **共用同一 ViewModel 管道**，避免两套业务事件语义。  
WebView 路径可继续走现有 Tauri 事件 + Vue；由适配层完成 ViewModel ↔ 现有前端协议的映射（允许渐进统一，但语义不得分叉）。

**禁止**在 WinUI 层实现翻译协议、配置持久化或历史写入；这些仍属 core。

## 配置与切换

### 字段

```json
{
  "popupUiBackend": "webview"
}
```

- 类型：`"webview" | "winui"`（serde camelCase：`popupUiBackend`）。
- **默认：`webview`**（降低首发与 CI 风险；用户可主动切到 `winui`）。
- 旧配置缺省 → `webview`。
- 非 Windows：运行时强制按 webview 行为；字段可原样读写。

### 设置 UI

- **仅 Windows** 展示「翻译弹窗 UI」选项：`WebView` / `WinUI`（可标实验或预览）。
- 文案要点：WinUI 更跟手、利于常驻内存；WebView 与现网一致、跨平台同源。
- 非 Windows 不展示该项。

### 切换策略（v1）

**修改后提示重启应用再生效。**

理由：同进程下热拆 WinUI/WebView 与消息循环风险高；重启行为可预期，实现简单。  
后续可选：热切换（hide → destroy A → ensure B）。

### 与 windowPrecreate

`windowPrecreate.*.popup` 作用于 **当前激活的 PopupBackend**（预建 WinUI 或 WebView 弹窗，语义与现网一致：隐藏 ensure，不改变设置/OCR 销毁策略）。

## 生命周期与内存

| 场景 | 行为 |
|------|------|
| 托盘常驻 | 主进程在；弹窗按 precreate 或首次唤起 `ensure_created` |
| 关弹窗 | `hide`，不 `destroy` |
| 划词 / 截图译 | `show(NearCursor)` + 推送 ViewModel |
| 托盘打开 | `show(Restore)` |
| 打开设置 / OCR | 另建 WebView，关即毁 |
| 退出应用 | `destroy` 弹窗 backend 并释放资源 |
| 切换 backend | 用户改配置 → 重启 → 只加载新 backend |

### 内存验收（定性 + 后定量）

对比场景：**仅托盘 + 弹窗预建并隐藏，静置约 30s 后稳态**。

- 指标：工作集 / 专用工作集（实现阶段选定工具并写入记录）。
- 期望：`winui` 常驻态 **不差于** `webview`，目标 **明显更低**。
- 说明：打开设置后的峰值仍可含 WebView，不作为「弹窗常驻更轻」的反证；验收聚焦弹窗常驻路径。

## 数据流

```
快捷键 / 划词 / OCR 完成
  → core 启动翻译批次
  → PopupBackend.show(...)
  → translation 事件流
  → 适配为 PopupViewModel
  → WebView: 现有事件 / Vue
  → WinUI: 绑定更新

用户操作（取消、换语言、复制…）
  → backend 回调
  → 现有 commands / core API
```

批次并发、单服务失败隔离、历史 best-effort 等核心规则不变。

## 失败与降级

| 情况 | 行为 |
|------|------|
| WinUI / Windows App SDK 初始化失败 | 记日志；**回退 webview**；可一次性用户提示 |
| 配置为 `winui` 但非 Windows | 静默 webview |
| 单次 `show` 失败 | 不阻断翻译核心路径；尽量提示或按降级策略处理 |
| WebView 路径 | 保持现网行为 |

### 运行时分发

- **优先框架依赖** Windows App SDK：安装包相对小，安装器检测并引导安装 Runtime（若缺失）。
- **自包含**作为可选后续（体积换离线省心）。
- **不引入 .NET Runtime**。

## 工程结构（示意）

- `src-tauri` 内模块如 `popup_backend`：
  - trait + 调度
  - `webview` 实现（现有 `popup_window` / 前端路径迁入或包装）
  - `#[cfg(windows)]` 的 `winui` 实现
- Cargo feature：如 `popup-winui`（Windows 构建开启策略在 plan 中定）。
- **无** C# / .NET 工程。

实现栈倾向：Rust + `windows` / Windows App SDK 投影（及成熟的声明式封装若评估可用）；具体 crate 选型在实现计划中 spike 后锁定。

## CI 与打包

- 继续 **Tauri NSIS**（或当前 `bundle.targets`），不因 WinUI 强制改 MSIX。
- GitHub Actions：
  - **Windows job**：安装构建依赖（Windows SDK / App SDK 等）→ `cargo test` / `tauri build`（含 winui feature）。
  - 非 Windows job（若有）：不编 winui，仅 webview 路径。
- 产物上传 artifact；代码签名与现网发布流程对齐（与是否 WinUI 正交）。
- 文档写清本机开发依赖。

## 测试策略

1. **单元**：`PopupBackend` mock 调度（show/hide/切换语义、非 Windows 强制 webview）。
2. **保留**：弹窗定位等纯逻辑单测；WebView 前端既有测试不删。
3. **Windows CI**：含 winui feature 的编译；少量 ensure/show/hide 集成（能力范围内）。
4. **手动清单**：划词、截图译、流式多服务卡片、取消、语言切换、复制、设置打开关闭、backend 切换后重启。

## 里程碑

| 阶段 | 内容 |
|------|------|
| **M1** | 抽出 `PopupBackend`，WebView 实现迁入；行为相对现网零变化 |
| **M2** | WinUI 最小壳：ensure / show / hide / 源文 + 至少一流式卡片 |
| **M3** | 主路径功能对齐 + 设置切换 + 降级回退 |
| **M4** | 视觉打磨、内存/启动对比数据、CI 稳定、架构/README 文档同步 |

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| Tauri 与 WinUI 同进程消息循环 / 线程模型 | M2 最小壳验证；接口保持可演进到子进程 |
| Rust WinUI 生态与示例少于 C# | 压缩 v1 控件面；只覆盖弹窗主路径 |
| 双 UI 维护成本 | 单一 ViewModel 管道；WebView 为兼容与跨平台基线 |
| Windows App Runtime 缺失 | 安装引导 + 启动降级 webview |
| 用户误解「整应用内存一定大降」 | 验收与文案聚焦 **弹窗常驻态** |

## 已定取舍摘要

1. 架构：**同进程** `PopupBackend` 双实现，非独立弹窗子进程。  
2. 默认 backend：**webview**；WinUI 用户显式选择。  
3. 切换：**重启生效**（v1）。  
4. 跨平台 v1：**仅预留 trait**；macOS/Linux 先 WebView。  
5. 语言：**Rust**，无 .NET。  
6. 打包/CI：延续 Tauri/NSIS + Windows GHA。

## 未决（留 plan / spike）

- WinUI 具体 crate 与是否采用声明式封装（如 windows-rs 生态 UI 层）的最终选型。  
- `popup-winui` feature 默认开/关与发布矩阵。  
- 内存对比的具体工具与数值门槛（M4 用测量填入）。  
- 安装器侧 Windows App Runtime 引导的具体实现方式（bootstrapper / 文档链接 / 内嵌安装包）。

## 验收清单

1. Windows 上可在设置中选择 `webview` / `winui`，重启后弹窗走对应 backend。  
2. 非 Windows 仅 webview；配置写 `winui` 不崩溃。  
3. WinUI 初始化失败时回退 webview，翻译主路径仍可用。  
4. 设置 / OCR 关闭仍销毁 WebView；弹窗关仍 hide 常驻。  
5. 划词 / 截图译 / 托盘打开主路径在两种 backend 下可用（WinUI 按里程碑逐步对齐）。  
6. Windows CI 能构建带 WinUI 特性的安装产物；无 .NET 运行时依赖。  
7. 文档（架构说明 / 开发依赖）已同步。
