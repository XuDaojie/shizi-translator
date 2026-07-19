# 渐进式开发里程碑计划

## 目标

Shizi 的目标是在 Windows 上打造一款响应极快、启动丝滑、体验接近 Bob 的大模型桌面翻译软件。

整体路线采用渐进式演进：

1. **MVP 阶段**：完全基于 Tauri v2，使用 Rust 承载系统能力和核心业务，使用 Web 技术快速完成设置页与翻译弹窗。
2. **原生能力扩展阶段**：补齐截图、OCR、跨平台系统能力。
3. **原生 UI 优化阶段**：保持 Rust 核心和 Web 设置页不变，将性能敏感的翻译弹窗替换为 Slint。
4. **产品化阶段**：完善多模型管理、打包、跨平台适配和性能抛光。

核心原则：**先跑通业务闭环，再替换性能敏感模块；Rust 核心稳定，UI 可插拔。**

## 里程碑 1：Tauri + Web 纯血 MVP 版

### 目标

快速跑通第一条完整链路：

```text
全局快捷键
  -> 强制划词复制
  -> 构造翻译请求
  -> 统一大模型流式 API 调度
  -> Rust 发送翻译事件
  -> Web 翻译弹窗展示流式结果
```

### 核心交付物

优先实现以下模块：

```text
src-tauri/src/core/translation/types.rs
src-tauri/src/core/translation/service.rs
src-tauri/src/core/llm/provider.rs
src-tauri/src/core/llm/openai_compatible.rs
src-tauri/src/core/llm/anthropic.rs
src-tauri/src/core/config/types.rs
src-tauri/src/core/config/store.rs
src-tauri/src/core/clipboard/mod.rs
src-tauri/src/app/shortcuts.rs
src-tauri/src/ui/popup_port.rs
src-tauri/src/ui/web_popup.rs
src-tauri/src/ui/translator_commands.rs
src-tauri/src/ui/settings_commands.rs
```

前端 MVP 可继续使用静态 HTML / JS / CSS；如果后续引入 React + Tailwind，也必须保持 UI 层只负责展示和交互，不承载翻译业务逻辑。

### 关键能力

- 托盘常驻。
- 全局快捷键触发翻译。
- Windows 下强制划词复制。
- Provider 配置读取。
- OpenAI-compatible 流式翻译。
- Claude / Anthropic 流式翻译。
- Web 翻译弹窗通过 Tauri Event 接收统一 `TranslationEvent`。
- 设置页管理 API Key、base URL、model、默认语言。

### 当前完成状态

截至当前 MVP，里程碑 1 已基本完成主链路：

已完成 / 基本完成：

- Rust `app` / `core` / `ui` 初始分层。
- `TranslationRequest` / `TranslationEvent` / `LlmProvider` / `TranslationService` 基础抽象。
- Mock provider，用于本地流式事件验证。
- OpenAI-compatible Chat Completions 流式 provider。
- WebView 通过 `translation:event` 接收 `Started` / `Delta` / `Finished` / `Failed` 并流式渲染。
- 配置最小闭环：本地 JSON 配置、`get_app_config` / `save_app_config`、独立 `settings` 窗口设置页。
- `Alt+T` 划词复制翻译：模拟 `Ctrl+C`、读取选中文本、触发翻译。
- 翻译窗口交互状态收敛：busy 保护、按钮状态、事件 session 过滤、输出自动滚动。
- 服务模块打磨（v0.2.1）：协议 id 前后端统一为 `openai_chat`/`claude_messages`/`mock`，未知协议报错；设置页挂载时与后端 `config.json` 双向同步；未对接渠道标记"开发中"并置灰启用；翻译弹窗卡片图标按渠道 id 区分。
- provider 抽象层重构与微软翻译渠道（已完成，2026-07）：LLM 专用 `LlmProvider` 重构为 LLM/ML 平级通用 `TranslationProvider`（含 `BatchTranslateProvider` + `StreamingAdapter` 非流式适配）；auto 解析下沉至 `core/translation/auto_lang.rs`；新增微软翻译渠道（`core/mt/`，`MicrosoftMtProvider` impl `BatchTranslateProvider`，Edge 引擎免 Key）；`provider_for_service` 迁移至 `core/translation/protocol.rs` 并接入 Microsoft 分支；前端 UA 采集 + `save_edge_translate_env` command；设置页 microsoft 渠道详情页精简、免 Key 校验放行。

暂未完成 / 后续演进：

**2026-07 当前 UI 未实现能力打磨**（已完成）：

- 设置页服务模块：API Key 校验和模型拉取已接通真实服务接口。
- 翻译行为：自动复制结果、划词剪贴板恢复、OCR 历史已接通。
- 提示词与思维链：系统提示词、用户提示词模板、chainOfThought 已接入 LLM 请求；Claude 支持 manual/adaptive thinking。
- 未接通入口已从 UI 隐藏或降级为只读展示。
- 高级页配置导出/导入已接通（导出剔除 API Key，导入保留本地 Key）。
- 高级日志系统已落地：前后端独立日志（Shizi.log / frontend.log）、运行时等级切换、API Key 与翻译正文脱敏、5MB 轮转 + 7 天清理、导出 zip。
- 开发中功能 dev/release 可见性分离（2026-07-12）：未对接渠道与 wip 功能块（思维链 / 反思 / 主题）在 release 包（`npm run tauri build`）隐藏、dev 包可见，`config.json` 数据保留、dev 切回仍可见、已配值后端行为不变；判据 `import.meta.env.DEV`（`useDevMode` composable + `<DevOnly>` 组件）。**自动检查更新**已于 2026-07-16 落地并移出 wip / DevOnly。
- GitHub 检查更新（已完成，2026-07-16）：`AppConfig` 持久化 `updateChannel` / `autoCheckUpdate`；command `check_for_update` 拉取 GitHub Releases 并按通道做 semver 比较；设置页手动检查（toast / Dialog → 浏览器打开 Release）；启动时若开启自动检查则用系统 dialog 提示，确认后跳转下载；**不做**应用内安装，**未**引入 `tauri-plugin-updater`、**未**改 release CI。

- ~~Anthropic / Claude 专用 provider~~ ✅。
- `TranslationInput` / `TranslationMode` 的完整输入模型。
- ~~`Cancelled` 事件~~ ✅、~~usage/token 统计~~ ✅、~~取消/重试交互~~ ✅。
- 独立设置页与独立翻译弹窗拆分 ✅。
- OCR / 截图 / Slint 原生弹窗。

当前 MVP 的实际文件结构与本计划早期命名略有差异：

- 剪贴板与选区读取落在 `src-tauri/src/core/selection/clipboard.rs`、`keyboard.rs`、`mod.rs`。
- 翻译 WebView 编排落在 `src-tauri/src/ui/web_popup.rs`。
- 设置 commands 落在 `src-tauri/src/ui/config.rs`。
- 当前 `TranslationEvent` 是 MVP 简化版，不含 `usage` 字段。

### 技术难点

#### 强制划词复制

推荐流程：

```text
保存当前剪贴板
  -> 模拟 Ctrl+C
  -> 短暂等待
  -> 读取剪贴板文本
  -> 按配置恢复原剪贴板
```

需要处理：

- 用户原剪贴板保护。
- 空选区。
- 不同应用对模拟复制的响应差异。
- 不阻塞 UI 线程。

#### 流式 LLM 调度

不同 provider 的流式格式必须在 adapter 内部消化，核心层只输出统一事件：

```rust
pub enum TranslationEvent {
    Started { session_id: TranslationSessionId, source_text: String },
    Delta { session_id: TranslationSessionId, text: String },
    Finished { session_id: TranslationSessionId, full_text: String, usage: Option<TokenUsage> },
    Failed { session_id: TranslationSessionId, message: String, retryable: bool },
    Cancelled { session_id: TranslationSessionId },
}
```

### 推荐开发任务

```text
任务 1：重构 Rust 项目结构，引入 core / ui / app 分层，不改变现有行为。
任务 2：实现 TranslationRequest / TranslationEvent / LlmProvider / TranslationService 抽象。
任务 3：实现 mock provider，验证流式事件链路。
任务 4：实现 OpenAI-compatible 流式翻译 adapter。
任务 5：实现 Claude / Anthropic 流式翻译 adapter。
任务 6：实现 WebPopup adapter，通过 Tauri Event 推送 TranslationEvent。
任务 7：实现前端翻译弹窗，监听 translation:event 并流式渲染。
任务 8：实现设置页 command，支持读取 / 保存 provider 配置。
任务 9：实现 Alt+T 后的强制划词复制与翻译弹窗展示。
```

### 验收标准

- 应用启动后托盘常驻。
- `Alt+T` 能触发翻译弹窗。
- 任意应用中选中文本后能进入翻译流程。
- 翻译结果可以流式显示。
- API Key、base URL、model 可配置。
- 切换 provider 不影响 UI 层代码。
- Web 弹窗不直接依赖具体 LLM provider。
- Rust 侧可以通过 mock provider 验证事件流。

## 里程碑 2：系统原生能力扩展

### 目标

加入截图与 OCR 能力：

```text
截图 / OCR 快捷键
  -> 系统截图
  -> 平台内置 OCR
  -> 提取文本
  -> 复用 TranslationService
  -> 翻译弹窗展示结果
```

### 核心交付物

```text
src-tauri/src/core/capture/mod.rs
src-tauri/src/core/ocr/mod.rs
src-tauri/src/platform/windows/capture.rs
src-tauri/src/platform/windows/ocr.rs
src-tauri/src/platform/macos/capture.rs
src-tauri/src/platform/macos/ocr.rs
```

### 推荐抽象

```rust
#[async_trait::async_trait]
pub trait ScreenCapture: Send + Sync {
    async fn capture_region(&self, region: CaptureRegion) -> Result<CapturedImage, CaptureError>;
    async fn capture_interactive(&self) -> Result<Option<CapturedImage>, CaptureError>;
}

#[async_trait::async_trait]
pub trait OcrEngine: Send + Sync {
    async fn recognize(&self, image: CapturedImage, hints: OcrHints) -> Result<OcrResult, OcrError>;
}
```

### 技术策略

Windows 优先使用系统内置 OCR：

```text
Windows.Media.Ocr
```

macOS 后续使用：

```text
Vision Framework
```

截图能力建议先 Windows 优先，不要一开始就追求所有平台一致体验。初版可以先支持全屏或鼠标附近区域，再演进到交互式框选。

### 推荐开发任务

```text
任务 1：新增 TranslationInput，统一划词、OCR、手动输入三类来源。
任务 2：新增 ScreenCapture trait 和 Windows 初版实现。
任务 3：新增 OcrEngine trait 和 Windows.Media.Ocr 初版实现。
任务 4：新增 OCR 快捷键，触发截图 -> OCR -> TranslationService。
任务 5：前端弹窗展示输入来源：Selected / OCR / Manual。 ✅
任务 6：加入 OCR 错误状态展示，例如无语言包、权限不足、识别为空。 ✅
```

### 验收标准

- Windows 上能触发截图 OCR。
- OCR 文本复用已有翻译流程。
- LLM provider 不知道输入来自划词还是 OCR。
- Web 弹窗只展示统一事件。
- OCR 失败不会导致应用崩溃。
- 后续 macOS Vision 只需新增一个 `OcrEngine` 实现。

## 里程碑 3：高性能原生 UI 重构

### 目标

将性能敏感的翻译弹窗从 WebView 替换为 Slint：

```text
TranslationService 不变
LlmProvider 不变
配置系统不变
Web 设置页不变
只替换 TranslationPopupPort 实现
```

需要实现：

- Slint 弹窗常驻内存。
- 默认隐藏。
- 毫秒级 show / hide。
- 无焦点置顶。
- 鼠标跟随或选区附近定位。
- 流式 token 与 Slint property 绑定。
- Web 翻译窗可保留为 fallback。

### 核心交付物

```text
src-tauri/ui/popup.slint
src-tauri/src/ui/slint_popup.rs
src-tauri/src/ui/popup_backend.rs
```

Slint 状态建议：

```slint
export component TranslationPopup inherits Window {
    in-out property <string> source_text;
    in-out property <string> translated_text;
    in-out property <string> status;
    in-out property <bool> is_streaming;
    in-out property <bool> has_error;
    in-out property <string> error_message;

    callback retry();
    callback copy_result();
    callback close_popup();
}
```

### 技术难点

#### 无焦点置顶

Windows 侧重点关注：

- `WS_EX_NOACTIVATE`
- `WS_EX_TOPMOST`
- `ShowWindow`
- `SetWindowPos`
- 不抢输入焦点
- 不打断用户当前应用

#### 毫秒级唤醒

热路径避免：

- 创建窗口。
- 重新加载 UI。
- 重新初始化字体或图形资源。
- 重新创建 LLM client。

热路径只做：

```text
set position
clear state
show
push events
```

#### UI 线程更新

Slint property 更新应由 Slint adapter 内部负责线程切换，不让 core 层知道 UI 运行时细节。

### 推荐开发任务

```text
任务 1：确认 TranslationPopupPort 接口稳定。
任务 2：新增 Slint 依赖与 popup.slint，不接入业务，仅能启动隐藏窗口。
任务 3：实现 SlintPopup adapter，支持 show / hide / push_event。
任务 4：接入 PopupBackend 配置，允许 Web 和 Slint 切换。
任务 5：实现 Windows 下无焦点置顶 show/hide。
任务 6：优化热路径：应用启动时预创建 popup，快捷键触发时只更新状态和显示。
任务 7：删除 Web 翻译窗中的业务状态，只保留设置页 WebView。
```

### 验收标准

- 切换到 Slint 后，翻译事件仍可流式显示。
- 里程碑 1 的 `TranslationService` / `LlmProvider` 不需要重写。
- 弹窗显示不抢焦点。
- 热启动显示体感接近瞬时。
- 设置页仍使用 WebView，不受影响。
- Web 翻译窗可以作为 fallback 保留一段时间。

## 里程碑 4：多端打包与细节抛光

### 目标

完成产品化：

- 多模型 Key 管理。
- 设置页完善。
- Windows / macOS / Linux 差异适配。
- 打包体积优化。
- 启动性能优化。
- 快捷键、OCR、截图、弹窗体验打磨。
- **快捷键绑定配置**（已完成，2026-07；2026-07 修订）：设置页快捷键接入后端 AppConfig.shortcuts；划词翻译、截图 OCR 翻译、剪贴板翻译为全局快捷键；打开设置为程序快捷键（窗口聚焦时生效）；已移除显示/隐藏主窗口；重复或系统占用时阻止保存并回填对应行错误；word-lookup 仅保存绑定。

### 核心交付物

#### Provider 管理

支持：

- Anthropic / Claude。
- OpenAI-compatible。
- DeepSeek。
- Gemini。
- Ollama。
- Custom。

每个 provider 支持：

- display name。
- base URL。
- API Key。
- model。
- timeout。
- proxy。
- default target language。
- enabled / disabled。

#### SecretStore

产品化阶段不要长期明文存储 API Key。推荐抽象：

```rust
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync {
    async fn get_secret(&self, key: &str) -> Result<Option<String>, SecretError>;
    async fn set_secret(&self, key: &str, value: &str) -> Result<(), SecretError>;
    async fn delete_secret(&self, key: &str) -> Result<(), SecretError>;
}
```

MVP 可以先用配置文件，产品化阶段迁移到：

- Windows Credential Manager。
- macOS Keychain。
- Linux Secret Service。

#### 性能指标

建议从产品化阶段前就开始埋点：

```text
cold_start_ms
hotkey_to_popup_visible_ms
hotkey_to_first_token_ms
llm_first_token_latency_ms
popup_show_ms
ocr_latency_ms
translation_total_ms
```

### 推荐开发任务

```text
任务 1：完善 AppConfig / ProviderConfig，并迁移旧配置。
任务 2：新增 SecretStore trait，Windows 优先实现 Credential Manager。
任务 3：设置页支持 provider 增删改查、测试连接、默认模型选择。
任务 4：新增性能埋点，统计快捷键到弹窗显示、快捷键到首 token。
任务 5：优化 Tauri 启动配置和 release 打包配置。
任务 6：完善 Windows 安装包、图标、托盘、开机自启（应用图标与开机自启 `launchAtLogin` 已完成；安装包等其余子项继续推进）。
任务 7：整理跨平台能力矩阵，明确 Windows / macOS / Linux 支持差异。
```

### 验收标准

- 多 provider 配置可用。
- API Key 进入系统安全存储。
- release 包可安装并正常运行。
- Windows 快捷键、OCR、弹窗体验稳定。
- 有基础性能数据支撑后续优化。
- 跨平台能力差异有明确说明。

## 参考源码分析流程

## 前端体验优化（Tauri UI 路线）

在 Tauri UI（WebView）路线下持续提升前端体验，暂不切 Slint。

- **设置页 Vue 3 重构**（骨架可交付，2026-07）：Vite 7 + Vue 3.5 + Tailwind CSS v4 + shadcn-vue（new-york）+ Iconify 替换原纯静态 HTML/JS/CSS。translate / overlay 平铺进 `frontend/public/` 保持纯静态（overlay 永久不迁）。构建产物 `frontend/dist/`。UI 视觉细节待 open design 原型图定稿后打磨。
- **翻译弹窗 UI 打磨**（已完成，2026-07）：按 OpenDesign 原型整套重写 `frontend/public/translate.html` / `translate.js` / `translate.css`——去 Windows 原生标题栏改自绘工具栏（`data-tauri-drag-region` 拖拽）、`decorations:false`+`transparent:true`+`resizable:false`、宽 452/.popup 420 固定 + 高自适应（ResizeObserver → `setSize`）、单卡片 + 预留多卡数据结构、图钉/截图翻译/设置/朗读/复制接真实后端、收藏/书签/语言栏 toast 占位、取消/重试挂状态栏文字按钮；后端仅新增 `trigger_ocr_translation` 薄封装 + 两个窗口权限。
- **翻译弹窗 UI 细节打磨**（已完成，2026-07-07）：结果卡片长内容截断（约 4-5 行）+ 渐隐遮罩 + 「展开全文」按钮（展开/收起，transitionend 修正窗口高度时序）；输入原文限高（约 7 行）内部滚动，不再撑高弹窗；focus 态上边框改 outline 修复被 `.content` overflow 裁剪导致的描边粗细不一致。
- **翻译弹窗语言下拉与语言联动**（已完成，2026-07-09）：语言下拉改为 inline 搜索式 combobox（`.lang-picker`，非浮层，带搜索框、英文名双列、键盘 ↑↓/Enter/Esc 导航，规避 `.content` overflow 裁剪）；源语言选「自动检测」时，`TranslationEvent::Finished` 携带 `detectedSourceLang` 由 `TranslationService` 流式首行解析状态机填充，译文区右下角 `.lang-badge` 动态显示检测结果（翻译中显示「检测中…」）；首次安装默认目标语言读 OS 语言（`sys-locale` + `map_os_lang_to_list`），不在支持列表回退 `FALLBACK_TARGET_LANG = "en-US"`，存量用户已选目标语言不受影响。
- **服务协议与批量翻译**（已完成，2026-07）：配置从单 provider 改为 `services[]` 数组驱动；服务实例按启用状态和列表顺序驱动翻译批次；翻译弹窗支持多服务结果卡；启动后默认显示翻译弹窗，设置页独立窗口打开；服务启用状态保存后，非翻译中翻译弹窗结果卡片即时同步，翻译进行中保留正在输出的卡片且不新增未参与当前批次的服务卡片；服务协议抽象接入 OpenAI Chat 与 Claude Messages。
- **应用国际化**（已完成，2026-07）：内置 8 种界面语言，`auto` 跟随操作系统并在未知 locale 时回退简体中文；设置页、翻译弹窗、托盘和窗口标题即时切换且不 reload；源/目标语言共享 19 种翻译语言规范（源语言另含 `auto`）；支持 `<app_config_dir>/lang/*.json` 用户局部覆盖包及设置页打开目录、即时刷新。
- [x] 服务实例按启用状态和列表顺序驱动翻译批次
- [x] 翻译弹窗支持多服务结果卡
- [x] 服务协议抽象接入 OpenAI Chat 与 Claude Messages
- 翻译页 Vue 迁移（后续）
- 最终视实际体验决定是否切 Slint

参考 Poets / Pot 等项目时，必须先分析再设计，不能边看边搬。推荐流程：

```text
Stage 1：目录结构分析，输出模块关系图。
Stage 2：翻译流程分析，输出调用链和流式处理方式。
Stage 3：OCR 流程分析，输出 OCR engine 和文本进入翻译流程的方式。
Stage 4：截图流程分析，输出多屏、DPI、权限和平台差异。
Stage 5：插件系统分析，输出插件接口、生命周期和配置方式。
Stage 6：配置系统分析，输出配置文件、secret 存储和设置页绑定方式。
Stage 7：UI 分析，输出翻译弹窗、设置页和状态管理方式。
Stage 8：重新设计 Shizi 架构，输出 Architecture Proposal。
Stage 9：确认 Proposal 后开始编码。
```

每个分析阶段只输出分析文档，不写代码。编码必须等 Architecture Proposal 确认后再开始。

## 推荐近期执行顺序

第一阶段不要先做 OCR 或 Slint，先完成插拔底座：

```text
1. Rust 分层。
2. TranslationService。
3. LlmProvider trait。
4. mock 流式事件。
5. Web 弹窗事件展示。
```

这组完成后，后续接真实 LLM、划词、OCR、Slint 替换都不会推倒重来。
