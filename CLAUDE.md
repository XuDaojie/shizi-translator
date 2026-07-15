## AGENTS.md instructions for C:\Users\xdj\IdeaProjects\LLM\shizi

<INSTRUCTIONS>
所有回答、任务进度说明及 Git 提交信息均须使用中文。

## 项目介绍

这是一个基于大模型的翻译软件，灵感来自 macOS 端的 Bob 以及 Windows 端的 Pot（后者衍生自 macOS 端的 Pot）。

项目目标并非做一个体验更好的 Pot，而是开发一个体验更好的 Windows 端大模型翻译软件，用于替代 Pot，希望在体验上尽量接近 Bob。第一版优先考虑 Windows 端，基于跨平台的 Tauri 技术栈实现。

## 项目结构

```
frontend/          Vite 工程：设置页 settings.html 为 Vue 3 + Tailwind v4 + shadcn-vue 入口；translate.html / overlay.html 平铺在 frontend/public/ 保持纯静态（overlay 永久不迁）。构建产物 frontend/dist/。
  settings.html    独立 settings 窗口设置页：provider / 目标语言 / API Key 等配置
  translate.html   main 窗口翻译弹窗入口（Vue 3，加载 /src/popup/main.ts）
  overlay.html     截图 OCR overlay：canvas 整屏 BGRA 渲染 + 鼠标框选，回传 CSS 矩形
  settings.* / translate.* 各自的 JS 与 CSS
  src/popup/       翻译弹窗 Vue 组件体系（根组件 TranslationPopup.vue + 8 子组件 + 3 composable + 共享 CSS），与设置页共享 src/ 工程；HistoryPanel.vue 复用其 SourceCardView/ResultCardView/LanguageToolbar
src-tauri/         Rust 后端 + Tauri 配置
  src/lib.rs       应用装配入口：注册插件、commands、托盘、快捷键、窗口生命周期
  src/main.rs      薄入口，调用 shizi_lib::run()
  src/app/         应用编排：托盘、全局快捷键、窗口控制、AppState（含 capture/translation 锁）
  src/core/config/ 本地配置模型与 JSON 存储
  src/core/history/ SQLite 翻译历史：session/result 两表，HistoryStore 聚合查询与裁剪
  src/core/llm/    LLM provider（mock、OpenAI-compatible、Claude 流式）；通用 provider 抽象已迁至 core/translation/provider.rs
  src/core/selection/ 划词复制：剪贴板文本读取、Ctrl+C 模拟
  src/core/translation/ 翻译请求、事件、TranslationService、通用 provider 抽象（provider.rs）、auto 解析（auto_lang.rs）、协议分发（protocol.rs）
  src/core/mt/        机器翻译 provider（mod.rs EdgeTranslateEnv + microsoft.rs MicrosoftMtProvider，Edge 引擎免 Key）
  src/core/capture/ 截图抽象：CapturedImage、crop 内存裁剪、css_rect_to_physical DPI 换算
  src/core/ocr/ OCR engine 抽象、Windows.Media.Ocr 实现
  src/core/ocr_translation.rs 截图+OCR→TranslationInput 编排（含区域裁剪 workflow）
  src/platform/ 平台缝：windows/（DXGI 抓帧、Windows OCR）+ unsupported/
  src/ui/          Tauri commands 与 WebView 事件桥：翻译、配置、OCR 编排、overlay 框选
  tauri.conf.json  窗口尺寸、标题、产品标识；frontendDist 指向 ../frontend
  capabilities/    Tauri 权限清单（core + global-shortcut，含 screenshot-overlay 窗口）
pot-desktop/       参考实现（Pot 源码，仅供学习对照，不要直接修改）
plugins.md         已安装插件/技能清单（新增或升级后必须同步）
```

Tauri 2 + 原生静态前端 —— 没有 Vite/webpack/打包步骤，前端文件被 Tauri 直接当作静态资源加载。

## 开发环境

- Node.js（用于运行 `@tauri-apps/cli`）
- Rust toolchain（stable, edition 2021）
- Windows 端需安装 WebView2 Runtime（Windows 11 已自带）
- 关键依赖：`tauri 2`（`tray-icon` feature）、`tauri-plugin-global-shortcut 2`、`reqwest`、`arboard`、`enigo`

## 常用命令

```bash
npm install                 # 首次需装前端依赖
npm run tauri dev           # 开发模式（拉起 Vite dev server + 后端）
npm run tauri build         # 生成 release 安装包（MSI/NSIS）
cd src-tauri && cargo build           # 仅构建后端 debug
cd src-tauri && cargo build --release # 构建后端（dev 模式 exe，加载 localhost:5173，需先 npm run dev 启动 Vite）
cd src-tauri && cargo clean           # 清理 Rust 编译缓存
```

调试运行需先 `npm run dev` 启动 Vite dev server，再执行 `./src-tauri/target/release/shizi.exe`（`cargo build --release` 生成的是 dev 模式 exe，加载 localhost:5173；不依赖 Vite 的真正 release 包用 `npm run tauri build`）；或用 [.vscode/launch.json](.vscode/launch.json) 的 F5 启动 `npm run tauri dev`。

## 架构关键点

- **分层结构**：
  - **核心层**：Rust 实现，承载翻译业务、配置管理、通用 provider 抽象（LLM/ML 平级）、划词复制等能力。
  - **UI 层**：已拆成 ①**翻译弹窗**（`main` 窗口加载 `translate.html`）与 ②**设置页**（独立 `settings` 窗口加载 `settings.html`）两个可替换模块，截图 overlay 为第三个独立页面。翻译弹窗已 Vue 化，与设置页共享 `src/popup/` 组件（`ResultCardView`/`SourceCardView`/`LanguageToolbar`），历史面板右侧详情复用这些组件。新增能力时不要把核心逻辑写进前端，也不要让 UI 模块互相耦合。
- **托盘驻留模型**：窗口的 `CloseRequested` 被拦截改为 `hide()`，应用通过托盘菜单「退出」才会真正退出；详见 [src-tauri/src/app/window.rs](src-tauri/src/app/window.rs) 与 [src-tauri/src/app/tray.rs](src-tauri/src/app/tray.rs)。
- **启动窗口与设置窗口**：`main` 窗口加载 `translate.html`，应用启动后默认显示翻译弹窗；`main` 在 `tauri.conf.json` 中 `visible: false`，setup **不**再立即 `show`。冷启动由前端 `TranslationPopup`：`initCards` → 至少一次 `setSize` → 双 rAF → `show` + `setFocus`，约 2s 超时强制 show；二次唤起仍走 `show_popup`（热窗，不等 ready）。设置页由独立 `settings` 窗口加载 `settings.html`，通过弹窗设置按钮、托盘「设置」或 `open_settings` command 打开。
- **翻译弹窗窗口**：`main` 窗口配置 `decorations(false)` + `transparent(true)` + `resizable(false)`，去除 Windows 原生标题栏；顶部 `.toolbar` 加 `data-tauri-drag-region` 实现自绘标题栏拖拽（Tauri 2 原生，零 JS）。`.popup` 宽 420px 固定，`body` 设 `padding:16px` + `background:transparent` 留阴影空间；高度由 `usePopupHeight` 经 `ResizeObserver` 监听 `.popup` 实测 DOM（随 N 张收缩/展开卡变化，非 conf 常量）后 `getCurrentWindow().setSize({ type:"Logical", width:452, height:h+32 })` 动态调整，上限屏幕高 80%（超出由 `.content` `overflow-y:auto` 滚动）。图钉按钮 `setAlwaysOnTop` 需 `core:window:allow-set-always-on-top` 权限，`setSize` 需 `core:window:allow-set-size`，前端冷启动 `show`/`setFocus` 需 `core:window:allow-show` / `core:window:allow-set-focus`，均已在 `capabilities/default.json` 授权。
- **服务协议配置**：后端配置以 `services[]` 为事实来源，每个服务实例包含 `protocol`、`endpoint`、`model`、`apiKey` 与启用状态；旧单 provider 配置不再作为运行路径。`provider_for_service`（位于 `core/translation/protocol.rs`）将协议标识映射到对应的 `TranslationProvider`（LLM/ML 平级通用 trait）：`openai_chat`/`claude_messages`/`mock` 走 LLM 流式 provider，`microsoft_edge` 经 `BatchTranslateProvider` + `StreamingAdapter` 适配为流式。
- **前后端配置同步**：`config.json` 的 `services[]` 是事实来源。设置页 `SettingsPage` 挂载时调 `settings.syncFromBackend()`：后端 `services` 为空（旧格式残留 / 首次启动）→ 前端 `projectToAppConfig` 推 `invokeSaveAppConfig` 覆盖后端；后端非空 → `mergeBackendIntoServices` 按 id 合并（后端 `enabled/apiKey/endpoint/model/protocol` 覆盖前端同 id 实例，前端 `prompts/keyStatus/chainOfThought/pulledModels/note` 保留；后端多出补进、前端多出删除）。协议 id 前后端统一为 `openai_chat`/`claude_messages`/`mock`/`microsoft_edge`，后端 `provider_for_service` 未知协议返回错误。开发中功能 dev 可见 / release 隐藏：未对接渠道（`ServiceMeta.protocols.length === 0`）在添加 Dialog / 服务列表 / 详情页三处，dev 包标 amber"开发中"并 `disabled` 启用开关，release 包（`npm run tauri build`）不渲染（`config.json` 数据保留，dev 切回仍可见）；wip 功能块（思维链 / 反思 / 主题 / 自动检查更新）由 `<DevOnly>`（读 `useDevMode` 即 `import.meta.env.DEV`）同样 dev 可见 / release 隐藏，`status="wip"` 仅作 dev 下徽标，已配值后端行为不变。
- **配置变更事件**：`save_app_config` 保存成功后广播 `app-config:changed`，翻译弹窗监听该事件并同步启用服务卡片；非翻译中即时新增、删除、排序和更新卡片，翻译进行中保留正在输出的卡片，不新增未参与当前批次的服务卡片。
- **应用国际化**：`config.json.interfaceLanguage` 是界面语言事实来源，`auto` 跟随 OS locale，未知 locale 回退 `zh-CN`；语言运行时通过 `interface-language:changed { locale, revision }` 即时同步设置页、翻译弹窗、托盘和窗口标题，不 reload。前端静态加载 `zh-CN` / `en-US`，其余 6 种内置语言使用动态 chunk；用户包位于 `<app_config_dir>/lang/*.json`，单文件最大 1 MiB，按「user -> 同 locale 内置 -> 内置 zh-CN」回退。源/目标语言共享 19 种翻译语言规范，源语言另含 `auto`；LLM prompt 使用稳定英文语言名，Edge 使用严格显式映射并对未知 code 报错。
- **批次翻译**：翻译入口过滤启用服务并保持列表顺序，为每个服务创建 `{batch_id}:{service_id}` session，事件携带 `serviceInstanceId/serviceName/serviceType/protocol`。各服务并发执行，单服务失败不影响其他。
- **翻译弹窗**：弹窗打开时通过 `initCards` 调 `get_app_config` 获取启用服务列表并预建占位卡片；结果卡默认 `collapsed`，`started` 不展开，首非空正文 / failed / finished 需展示时展开，用户手动折叠本 batch 内优先（`collapseUserOverride`）；`getCard` 按 `serviceInstanceId` 复用已有卡片原地更新，new batch 重置而非销毁卡片。单服务失败只更新对应卡片。
- **翻译弹窗语言下拉**：inline 搜索式 combobox（`.lang-picker`，非浮层，规避 `.content` overflow 裁剪），带搜索框、英文名双列、键盘 ↑↓/Enter/Esc 导航。
- **源语言自动检测**：`TranslationEvent::Finished` 含 `detectedSourceLang: Option<String>`（source=auto 时由 provider 事件 `TranslationStreamEvent::DetectedSourceLang` 填充：LLM 流式首行解析状态机或 ML 响应 `detectedLanguage`，序列化为 camelCase `detectedSourceLang`），前端译文区右下角 `.lang-badge` 动态显示。
- **默认目标语言**：`AppConfig::default` 默认 `target_lang` 读 OS 语言（`sys-locale` + `map_os_lang_to_translation`），`normalized` 兜底用常量 `FALLBACK_TARGET_LANG = "zh-CN"`（不读 OS）；存量用户已选目标语言不受影响。
- **全局快捷键**：`Alt+D` 划词复制并自动翻译；`Alt+S` 触发截图 OCR 翻译（DXGI 抓光标所在显示器整屏帧 → 自建 overlay 区域框选 → crop → 当前启用 OCR 引擎 → 复用翻译链路）；`Alt+O` 打开独立文字识别窗口并可直接进入框选识别（文件/剪贴板亦走同一 `recognize` 编排，不翻译、不写历史）。截图路径以 `AppState.CapturePurpose`（`Translate` | `RecognizeOnly`）在 `submit_capture_region` 分叉。由 `tauri-plugin-global-shortcut` 注册，逻辑集中在 `src-tauri/src/app/shortcuts.rs`。启动注册为 best-effort：单条快捷键被其他应用占用只记录到 `AppState.shortcut_conflicts`，不阻止启动；冲突列表经 `get_shortcut_conflicts` command 供设置页快捷键模块展示（保存路径仍为 all-or-nothing，失败回滚旧配置）。新增快捷键时需在 `capabilities/default.json` 同步授权。
- **前后端通信**：当前已有 Tauri commands：`start_translation`、`take_pending_source_text`、`get_app_config`、`save_app_config`、`get_shortcut_conflicts`，以及截图 overlay 四命令 `get_capture_frame_meta` / `get_capture_frame_bytes` / `submit_capture_region` / `cancel_capture`，日志两命令 `write_frontend_log` / `export_logs`，Edge UA 采集一命令 `save_edge_translate_env`（前端采集 UA/Accept-Language 存 AppState 进程级内存）。后端通过 `translation:event` 向前端推送 `Started` / `Delta` / `Finished` / `Failed`（`Finished` 含 `detectedSourceLang: Option<String>`，由 provider 事件 `TranslationStreamEvent::DetectedSourceLang` 回传，LLM 首行解析或 ML 响应 `detectedLanguage` 填充）。
- **配置存储**：当前设置面板将 provider 配置保存到 Tauri app config dir 下的 `config.json`，含 `logLevel` 字段（error/warn/info/debug，默认 info）。支持 OpenAI-compatible 和 Claude 两种 provider。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。
- **翻译历史**：历史数据由后端 `core/history::HistoryStore` 写入 `app_config_dir()/history.sqlite3`，`AppState` 与 `ConfigStore` 同级持有 store。`web_popup.rs` 仅在统一翻译入口触发 session/result 写入，不包含 SQL；设置页通过 `list_translation_history` / `clear_translation_history` 查询和清空，不再保存 `ocrHistory` 到前端 localStorage。
- **日志系统**：前后端各一套日志，物理分开保存到 `app_config_dir()/logs/`。后端 `Shizi.log` 由 `tauri-plugin-log` 写（注册时内部 level Debug 不挡，全局 `log::set_max_level` 控制；5MB `KeepAll` 轮转，轮转文件 `Shizi_<timestamp>.log`；文件名按 `productName` 固定为 `Shizi.log`，不支持自定义）。前端 `frontend.log` 由 `write_frontend_log` command 直接 `std::fs::append` 写、不走 log facade（5MB 轮转 `frontend.log.1`/`.2`）。运行时等级切换：`save_app_config` 保存 `logLevel` 后调 `log::set_max_level` 即时生效，前端 `logger.setLevel` 订阅 `app-config:changed`。脱敏：API Key 前 4+后 4（`redact_api_key`），翻译正文 info 记摘要（长度+前 20 字）、debug 记全文（`redact_text`）。启动清理 7 天旧日志（`cleanup_old_logs`）。`export_logs` 打包 zip 含日志 + `config-snapshot.json`（apiKey 脱敏）+ `system-info.txt`。日志系统任何环节失败 best-effort，不影响翻译主流程。

## 开发说明

1. `AGENTS.md` 与 `CLAUDE.md` 需保持内容同步，修改任一文件后应立即同步更新另一个。
2. Superpowers 生成 spec（设计规格）或 plan（实现计划）的 markdown 文件后，应立即将其提交到 git，无需再次询问。
3. 自定义 skill 以 `my-` 为前缀，避免与安装的 skill 冲突（如 `/my-commit` 自动生成提交信息）。
4. Superpowers-ZH 标记区域（`<!-- superpowers-zh:begin -->` 至 `<!-- superpowers-zh:end -->`）由 superpowers-zh 插件自动维护，日常开发不得修改。仅在执行 superpowers-zh 插件版本升级时，由升级流程自动更新该区域。
5. **Pot 源码使用约束**：`pot-desktop/` 是参考实现，**任何时候都不要直接按照 Pot 的源码翻译代码**。涉及 Pot 源码时必须按以下四步工作流处理：
   1. **阅读**：先阅读源码，**不要写代码**，输出：功能分析、模块分析、架构分析、数据流分析、可以借鉴的地方、建议放弃的设计。
   2. **设计**：根据我们的目标重新设计架构，输出 *Architecture Proposal*。
   3. **等待确认**：交付 Proposal 后停下来等用户确认，不得自行进入实现。
   4. **编码**：确认通过后才能开始编码，且按重新设计的方案实现，不照搬 Pot 代码。


## 测试

项目当前已有 Rust 单元测试。常用验证命令：

```bash
cd src-tauri && cargo test
cd src-tauri && cargo build
npm run dev                 # 仅启动前端 Vite dev server
npm run build               # 仅构建前端到 frontend/dist/
npm run typecheck           # vue-tsc 类型检查
npm run test                # vitest 单测
```

前端 validateConfig 纯函数已覆盖 vitest 单测（`npm run test`）；Vue 组件通过 vue-tsc 类型检查（`npm run typecheck`）和 Tauri dev 手动验证。

## 协作规范

1. **问题分组提交**：将需求按功能分组，每组 2-4 个逻辑相关的优化一并提出
2. **文档同步是收尾硬门禁**：每组改动测试通过并提交后，必须同步更新对应设计文档（spec/plan 复选框回填、README 当前能力与限制、roadmap 完成状态、架构文档）。执行 `finishing-a-development-branch`（或等价 finish 流程）前，先确认本组文档已同步；若中途遗漏，finish 收尾的第一步必须是补齐文档同步，再进入合并/清理。文档同步是流程步骤，不是可选善后，不依赖 AI 自检。
3. **图片使用规范**：对话中包含图片时，必须先确认才能继续；获取关键信息后应提炼为文字
4. **版本更新同步**：版本号变更时须同步相关文件
5. **插件同步规范**：新增或升级插件/技能后，必须同步更新 `plugins.md` 文件
6. **子代理模型调度规范**：调度子代理前必须遵循 [docs/agent-model-policy.md](docs/agent-model-policy.md)。
   - Claude Code / Codex 使用弱模型、中模型、强模型三档，默认使用中模型；Codex 依次使用 `gpt-5.6-luna` / `gpt-5.6-terra` / `gpt-5.6-sol`。
   - 任务卡必须写明实际模型名；Codex 还必须写明推理强度（默认依次为 `low` / `medium` / `high`），禁止只写“弱模型 / 中模型 / 强模型”。
   - Grok Build 不执行模型决策门，始终继承当前会话模型。
   - 子代理连续失败后不得自行升级模型，必须停机回报主会话。

7. **阶段化开发与对话交接**：每个功能开发拆成三个阶段，**每阶段在独立对话内完成**，阶段之间通过「交接提示词」在新对话续接，**禁止在当前对话跨阶段继续**。长对话会让上下文漂移、产出质量下降，所以一个阶段结束就必须停下。
   - **阶段划分**：
     1. **需求规划**（`brainstorming` skill）→ 产出 spec（设计规格）markdown
     2. **实现计划**（`writing-plans` skill）→ 产出 plan（实现计划）markdown
     3. **编码执行**（`executing-plans` 或 `subagent-driven-development` skill）→ 按计划落地代码
   - **执行方式必须由用户选择**：进入编码执行阶段后，**必须先用 `AskUserQuestion` 工具询问用户**在「子代理驱动（`subagent-driven-development`）」与「内联执行（`executing-plans`）」之间二选一，**不得自行决定、不得默认内联**；未得到用户答复前不得进入任何执行动作。这一步是 plan→执行 交接提示词「下一步动作」第 2 步的硬性前置。
   - **产出物规则**：spec / plan 文档生成后立即 `git add` + `git commit`（见开发说明第 2 条，无需再次询问），交接提示词中以相对路径引用该文件。新对话靠读文件恢复上下文，不依赖旧对话历史。
   - **交接点**：两个 —— spec→plan（需求规划完成 → 编写实现计划）、plan→执行（实现计划完成 → 编码落地）。
   - **阶段收尾必须先询问用户（优先级声明）**：第三方 skill 的 SKILL.md 末尾常指示「直接调用下一阶段 skill」（`brainstorming`→`writing-plans`；`writing-plans`→`executing-plans`/`subagent-driven-development` 并提供执行选项）。**该类终止指示被本规范第 7 条覆盖，不得自行直接执行**，收尾时必须先停下、用 `AskUserQuestion` 询问用户选择走向：
     - `brainstorming`：用户审查批准 spec 后，询问用户 ① 继续在本对话调用 `writing-plans` 编写实现计划 ② 输出 spec→plan 交接提示词并停下（推荐）。未得到答复前不得进入下一步。
     - `writing-plans`：计划自检通过、保存并 commit 后，询问用户 ① 继续在本对话进入编码执行（再由下一项决定执行方式）② 输出 plan→执行 交接提示词并停下（推荐）。未得到答复前不得进入下一步；选 ① 时仍须按下方「执行方式必须由用户选择」用 `AskUserQuestion` 确认子代理驱动 / 内联执行。
     - 当 skill 终止指令与本规范冲突时，以本规范为准。本规范约束的是单次对话内不得跨阶段，不约束已通过交接进入新对话后的正常 skill 调用。
   - **交接即终止循环**：交接提示词在新对话作为首条消息收到后，该新对话即视为已进入目标阶段，在其中执行该阶段任务并调用对应 skill 是正确的，**不违反「禁止在当前对话跨阶段继续」**——该禁令约束的是单次对话内不得从一阶段直接滑入下一阶段，不约束已通过交接进入新对话后的正常执行；此时不得再生成交接提示词或要求另开新对话。模板内的「给 AI」元指令即此规则的载体。
   - **交接提示词模板**：见 [docs/superpowers/handoff-templates.md](docs/superpowers/handoff-templates.md)，按字段填充后在回复末尾以代码块输出。模板含 spec→plan 与 plan→执行 两套。
   - **交接提示词只打印、不落盘**：直接在回复末尾以代码块输出，供用户复制到新对话，不写入文件、不进 git。新对话恢复上下文唯一依赖是已提交的 spec / plan 文档。
   - **编码执行阶段收尾**：所有任务落地、测试通过后，先按协作规范第 2 条同步文档，再执行 `finishing-a-development-branch`（或等价 finish 流程）。

## 提交规范

遵循 Conventional Commits，格式：`<type>(<scope>): <中文描述>`

常用 type：`feat` / `fix` / `perf` / `refactor` / `docs` / `chore` / `style` / `test` / `ci`

<!-- superpowers-zh:begin (do not edit between these markers) -->
# Superpowers-ZH 中文增强版

本项目已安装 superpowers-zh 技能框架（20 个 skills）。

## 核心规则

1. **收到任务时，先检查是否有匹配的 skill** — 哪怕只有 1% 的可能性也要检查
2. **设计先于编码** — 收到功能需求时，先用 brainstorming skill 做需求分析
3. **测试先于实现** — 写代码前先写测试（TDD）
4. **验证先于完成** — 声称完成前必须运行验证命令

## 可用 Skills

Skills 位于 `.claude/skills/` 目录，每个 skill 有独立的 `SKILL.md` 文件。

- **brainstorming**: 在任何创造性工作之前必须使用此技能——创建功能、构建组件、添加功能或修改行为。在实现之前先探索用户意图、需求和设计。
- **chinese-code-review**: 中文 review 沟通参考——话术模板、分级标注（必须修复/建议修改/仅供参考）、国内团队常见反模式应对。仅在用户显式 /chinese-code-review 时调用，不要根据上下文自动触发。
- **chinese-commit-conventions**: 中文 commit 与 changelog 配置参考——Conventional Commits 中文适配、commitlint/husky/commitizen 中文模板、conventional-changelog 中文配置。仅在用户显式 /chinese-commit-conventions 时调用，不要根据上下文自动触发。
- **chinese-documentation**: 中文文档排版参考——中英文空格、全半角标点、术语保留、链接格式、中文文案排版指北约定。仅在用户显式 /chinese-documentation 时调用，不要根据上下文自动触发。
- **chinese-git-workflow**: 国内 Git 平台配置参考——Gitee、Coding.net、极狐 GitLab、CNB 的 SSH/HTTPS/凭据/CI 接入差异与镜像同步配置。仅在用户显式 /chinese-git-workflow 时调用，不要根据上下文自动触发。
- **dispatching-parallel-agents**: 当面对 2 个以上可以独立进行、无共享状态或顺序依赖的任务时使用
- **executing-plans**: 当你有一份书面实现计划需要在单独的会话中执行，并设有审查检查点时使用
- **finishing-a-development-branch**: 当实现完成、所有测试通过、需要决定如何集成工作时使用——通过提供合并、PR 或清理等结构化选项来引导开发工作的收尾
- **mcp-builder**: MCP 服务器构建方法论 — 系统化构建生产级 MCP 工具，让 AI 助手连接外部能力
- **receiving-code-review**: 收到代码审查反馈后、实施建议之前使用，尤其当反馈不明确或技术上有疑问时——需要技术严谨性和验证，而非敷衍附和或盲目执行
- **requesting-code-review**: 完成任务、实现重要功能或合并前使用，用于验证工作成果是否符合要求
- **subagent-driven-development**: 当在当前会话中执行包含独立任务的实现计划时使用
- **systematic-debugging**: 遇到任何 bug、测试失败或异常行为时使用，在提出修复方案之前执行
- **test-driven-development**: 在实现任何功能或修复 bug 时使用，在编写实现代码之前
- **using-git-worktrees**: 当需要开始与当前工作区隔离的功能开发，或在执行实现计划之前使用——通过原生工具或 git worktree 回退机制确保隔离工作区存在
- **using-superpowers**: 在开始任何对话时使用——确立如何查找和使用技能，要求在任何响应（包括澄清性问题）之前调用 Skill 工具
- **verification-before-completion**: 在宣称工作完成、已修复或测试通过之前使用，在提交或创建 PR 之前——必须运行验证命令并确认输出后才能声称成功；始终用证据支撑断言
- **workflow-runner**: 在 Claude Code / OpenClaw / Cursor 中直接运行 agency-orchestrator YAML 工作流——无需 API key，使用当前会话的 LLM 作为执行引擎。当用户提供 .yaml 工作流文件或要求多角色协作完成任务时触发。
- **writing-plans**: 当你有规格说明或需求用于多步骤任务时使用，在动手写代码之前
- **writing-skills**: 当创建新技能、编辑现有技能或在部署前验证技能是否有效时使用

## 如何使用

当任务匹配某个 skill 时，使用 `Skill` 工具加载对应 skill 并严格遵循其流程。绝不要用 Read 工具读取 SKILL.md 文件。

如果你认为哪怕只有 1% 的可能性某个 skill 适用于你正在做的事情，你必须调用该 skill 检查。
<!-- superpowers-zh:end -->
