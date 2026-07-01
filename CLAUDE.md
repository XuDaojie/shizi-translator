# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目介绍

这是一个基于大模型的翻译软件，灵感来自 macOS 端的 Bob 以及 Windows 端的 Pot（后者衍生自 macOS 端的 Pot）。

项目目标并非做一个体验更好的 Pot，而是开发一个体验更好的 Windows 端大模型翻译软件，用于替代 Pot，希望在体验上尽量接近 Bob。第一版优先考虑 Windows 端，基于跨平台的 Tauri 技术栈实现。

## 项目结构

```
frontend/          静态前端（原生 HTML/JS/CSS，无构建步骤）
  index.html       主窗口 UI：翻译输入、输出区、内嵌设置面板
  overlay.html     截图 OCR overlay：canvas 整屏 BGRA 渲染 + 鼠标框选，回传 CSS 矩形
  main.js          前端交互：配置读写、翻译触发、translation:event 监听与流式渲染
  style.css        主窗口样式
src-tauri/         Rust 后端 + Tauri 配置
  src/lib.rs       应用装配入口：注册插件、commands、托盘、快捷键、窗口生命周期
  src/main.rs      薄入口，调用 shizi_lib::run()
  src/app/         应用编排：托盘、全局快捷键、窗口控制、AppState（含 capture/translation 锁）
  src/core/config/ 本地配置模型与 JSON 存储
  src/core/llm/    LLM provider 抽象、mock、OpenAI-compatible 流式 provider
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
npm run tauri dev           # 开发模式（启动后端 + 加载 frontend/）
npm run tauri build         # 生成 release 安装包（MSI/NSIS）
cd src-tauri && cargo build           # 仅构建后端 debug
cd src-tauri && cargo build --release # 仅构建后端 release（产物：src-tauri/target/release/shizi.exe）
cd src-tauri && cargo clean           # 清理 Rust 编译缓存
```

调试运行可直接执行 `./src-tauri/target/release/shizi.exe`，或用 [.vscode/launch.json](.vscode/launch.json) 中的 F5 启动 `npm run tauri dev`。

## 架构关键点

- **分层结构**：
  - **核心层**：Rust 实现，承载翻译业务、配置管理、LLM provider、划词复制等能力。
  - **UI 层**：当前仍是单 WebView 主窗口，包含翻译区与内嵌设置面板；后续目标是拆成 ①**翻译弹窗** 与 ②**设置页面** 两个可替换模块。新增能力时不要把核心逻辑写进前端，也不要让未来两个 UI 模块互相耦合。
- **托盘驻留模型**：窗口的 `CloseRequested` 被拦截改为 `hide()`，应用通过托盘菜单「退出」才会真正退出；详见 [src-tauri/src/app/window.rs](src-tauri/src/app/window.rs) 与 [src-tauri/src/app/tray.rs](src-tauri/src/app/tray.rs)。
- **全局快捷键**：`Alt+T` 划词复制并自动翻译；`Alt+O` 触发截图 OCR 翻译（DXGI 抓光标所在显示器整屏帧 → 自建 overlay 区域框选 → crop → Windows.Media.Ocr → 复用翻译链路）。由 `tauri-plugin-global-shortcut` 注册，逻辑集中在 `src-tauri/src/app/shortcuts.rs`。新增快捷键时需在 `capabilities/default.json` 同步授权。
- **前后端通信**：当前已有 Tauri commands：`start_translation`、`take_pending_source_text`、`get_app_config`、`save_app_config`，以及截图 overlay 四命令 `get_capture_frame_meta` / `get_capture_frame_bytes` / `submit_capture_region` / `cancel_capture`。后端通过 `translation:event` 向前端推送 `Started` / `Delta` / `Finished` / `Failed`。
- **配置存储**：当前设置面板将 OpenAI-compatible 配置保存到 Tauri app config dir 下的 `config.json`。API Key 在 MVP 阶段明文保存，后续产品化需迁移到系统 SecretStore。

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
node --check frontend/main.js
```

前端尚未引入测试框架，当前以前端语法检查 + Tauri dev 手动验证为主。

## 协作规范

1. **问题分组提交**：将需求按功能分组，每组 2-4 个逻辑相关的优化一并提出
2. **文档同步时机**：每组改动完成（测试通过 → 提交）后、切换至下一组任务前，同步更新设计文档
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
