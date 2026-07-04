## 项目介绍

这是一个基于大模型的翻译软件，灵感来自 macOS 端的 Bob 以及 Windows 端的 Pot（后者衍生自 macOS 端的 Pot）。

项目目标并非做一个体验更好的 Pot，而是开发一个体验更好的 Windows 端大模型翻译软件，用于替代 Pot，希望在体验上尽量接近 Bob。第一版优先考虑 Windows 端，基于跨平台的 Tauri 技术栈实现。

## 项目结构

```
frontend/          Vite 工程：设置页 settings.html 为 Vue 3 + Tailwind v4 + shadcn-vue 入口；translate.html / overlay.html 平铺在 frontend/public/ 保持纯静态（overlay 永久不迁）。构建产物 frontend/dist/。
  settings.html    主窗口设置页：provider / 目标语言 / API Key 等配置
  translate.html   独立翻译弹窗：监听 translation:event 流式渲染、取消重试、来源徽章
  overlay.html     截图 OCR overlay：canvas 整屏 BGRA 渲染 + 鼠标框选，回传 CSS 矩形
  settings.* / translate.* 各自的 JS 与 CSS
src-tauri/         Rust 后端 + Tauri 配置
  src/lib.rs       应用装配入口：注册插件、commands、托盘、快捷键、窗口生命周期
  src/main.rs      薄入口，调用 shizi_lib::run()
  src/app/         应用编排：托盘、全局快捷键、窗口控制、AppState（含 capture/translation 锁）
  src/core/config/ 本地配置模型与 JSON 存储
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible、Claude 流式 provider
  src/core/selection/ 划词复制：剪贴板文本读取、Ctrl+C 模拟
  src/core/translation/ 翻译请求、事件与 TranslationService
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
cd src-tauri && cargo build --release # 仅构建后端 release（产物：src-tauri/target/release/shizi.exe）
cd src-tauri && cargo clean           # 清理 Rust 编译缓存
```

调试运行可直接执行 `./src-tauri/target/release/shizi.exe`，或用 [.vscode/launch.json](.vscode/launch.json) 中的 F5 启动 `npm run tauri dev`。

## 架构关键点

- **分层结构**：
  - **核心层**：Rust 实现，承载翻译业务、配置管理、LLM provider、划词复制等能力。
  - **UI 层**：已拆成 ①**设置页**（主窗口 `settings.html`）与 ②**翻译弹窗**（独立 `translate.html`，划词 / OCR 触发时创建并跟随光标定位）两个可替换模块，截图 overlay 为第三个独立页面。新增能力时不要把核心逻辑写进前端，也不要让 UI 模块互相耦合。
- **托盘驻留模型**：窗口的 `CloseRequested` 被拦截改为 `hide()`，应用通过托盘菜单「退出」才会真正退出；详见 [src-tauri/src/app/window.rs](src-tauri/src/app/window.rs) 与 [src-tauri/src/app/tray.rs](src-tauri/src/app/tray.rs)。
- **翻译弹窗窗口**：`build_popup` 配置 `decorations(false)` + `transparent(true)` + `resizable(false)` + `inner_size(452, 512)`，去除 Windows 原生标题栏；顶部 `.toolbar` 加 `data-tauri-drag-region` 实现自绘标题栏拖拽（Tauri 2 原生，零 JS）。`.popup` 宽 420px 固定，`body` 设 `padding:16px` + `background:transparent` 留阴影空间；高度由前端 `ResizeObserver` 监听 `.popup` 内容高度变化后 `getCurrentWindow().setSize({ type:"Logical", width:452, height:h+32 })` 动态调整，上限屏幕高 80%（超出由 `.content` `overflow-y:auto` 滚动）。图钉按钮 `setAlwaysOnTop` 需 `core:window:allow-set-always-on-top` 权限，`setSize` 需 `core:window:allow-set-size`，均已在 `capabilities/default.json` 授权。
- **服务协议配置**：后端配置以 `services[]` 为事实来源，每个服务实例包含 `protocol`、`endpoint`、`model`、`apiKey` 与启用状态；旧单 provider 配置不再作为运行路径。`provider_for_service` 将协议标识映射到对应的 LLM provider。
- **批次翻译**：翻译入口过滤启用服务并保持列表顺序，为每个服务创建 `{batch_id}:{service_id}` session，事件携带 `serviceInstanceId/serviceName/serviceType/protocol`。各服务并发执行，单服务失败不影响其他。
- **翻译弹窗**：弹窗按服务实例渲染多个结果卡（`getCard` 按 `serviceInstanceId` 创建/复用），new batch 清空旧卡片，同 batch 内追加新服务卡片。单服务失败只更新对应卡片。
- **全局快捷键**：`Alt+T` 划词复制并自动翻译；`Alt+O` 触发截图 OCR 翻译（DXGI 抓光标所在显示器整屏帧 → 自建 overlay 区域框选 → crop → Windows.Media.Ocr → 复用翻译链路）。由 `tauri-plugin-global-shortcut` 注册，逻辑集中在 `src-tauri/src/app/shortcuts.rs`。新增快捷键时需在 `capabilities/default.json` 同步授权。
- **前后端通信**：当前已有 Tauri commands：`start_translation`、`take_pending_source_text`、`get_app_config`、`save_app_config`，以及截图 overlay 四命令 `get_capture_frame_meta` / `get_capture_frame_bytes` / `submit_capture_region` / `cancel_capture`。后端通过 `translation:event` 向前端推送 `Started` / `Delta` / `Finished` / `Failed`。
- **配置存储**：当前设置面板将 provider 配置保存到 Tauri app config dir 下的 `config.json`。支持 OpenAI-compatible 和 Claude 两种 provider。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。

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
6. **Superpowers 子代理模型分配策略**：调度 Superpowers 子代理前，必须先执行“强弱模型决策门”。默认选择弱模型；禁止因为主会话使用强模型就让子代理继承强模型；只有通过升级判断后，才能显式指定强模型。
   - **决策门必填项**：每次调度子代理前，先判断并在任务提示或编排说明中体现：任务类型、风险等级、是否可用测试或人工审查验证、弱模型是否足够、是否触发强模型升级条件。
   - **默认弱模型**：开发、测试、文档、检索、脚本化验证、规格草案、普通规格审查、普通代码质量审查、重复性检查等任务默认使用弱模型。若工具需要显式模型参数，必须显式填写弱模型；若工具默认继承主会话模型，必须覆盖默认值，避免无意使用强模型。
   - **审查先弱后强**：规格审查和代码质量审查默认先由弱模型完成；只有弱模型输出显示高不确定性、跨模块影响、关键架构取舍、安全/性能风险、反复失败或验证成本明显高于复核成本时，才升级强模型做复核。
   - **强模型升级记录**：调用强模型前必须写明升级原因和强模型的限定职责，例如“仅复核弱模型发现的高风险点”或“仅判断架构取舍”，不得把常规实现、常规审查或批量重复任务直接交给强模型。
   - **通用分级原则**：先判断任务需要“执行层模型”还是“把关层模型”，再映射到当前工具可用的同级模型；新增其他模型供应方时，也按其公开/本地约定的能力梯队归入弱、中、强三档，不为单个工具写特殊规则。
   - **当前常用梯队**：Claude Code 按 `fable > opus > sonnet > haiku` 分级；Codex 按当前工具/团队约定的 `GPT 5.5 > GPT 5.4` 分级。若外部工具实际可用模型名称变化，以该工具当前可用的同级最高/次高级模型替代，不改变强弱分配原则。

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

Skills 位于 `.codex/skills/` 目录，每个 skill 有独立的 `SKILL.md` 文件。

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
