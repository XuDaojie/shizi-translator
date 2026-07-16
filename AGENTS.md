## AGENTS.md instructions for C:\Users\xdj\IdeaProjects\LLM\shizi

<INSTRUCTIONS>
所有回答、任务进度说明及 Git 提交信息均须使用中文。

## 项目介绍

Windows 端大模型翻译软件（Tauri 2 + Vue/Vite），目标体验接近 macOS Bob、替代 Windows Pot。`pot-desktop/` 仅供学习对照，禁止直接按 Pot 源码翻译实现。

## 项目结构（摘要）

```
frontend/          Vite：settings.html（设置）、translate.html（弹窗 Vue）、public/overlay.html（OCR 框选，永久静态）
  src/popup/       翻译弹窗组件与 composable；历史面板复用其卡片组件
src-tauri/         Rust：lib.rs 装配；app/ 托盘快捷键窗口；core/{config,history,llm,mt,translation,selection,capture,ocr,update}；ui/ commands
capabilities/      Tauri 权限（改快捷键/窗口 API 须同步）
plugins.md         已装插件/技能清单（变更须同步）
```

细节与目录索引见 [docs/agent/architecture-notes.md](docs/agent/architecture-notes.md)。

## 开发与验证

- 环境：Node.js、Rust stable、Windows WebView2
- 常用：`npm install` · `npm run tauri dev` · `npm run tauri build` · `cd src-tauri && cargo test|build`
- 前端：`npm run dev` / `build` / `typecheck` / `test`（vitest）
- 调试：先 `npm run dev` 再跑 release 下的 dev 模式 exe（加载 localhost:5173），或 VS Code F5 `tauri dev`

## 架构要点（必守）

完整说明见 [docs/agent/architecture-notes.md](docs/agent/architecture-notes.md)。改模块前先读对应小节。

- **分层**：业务在 Rust 核心；UI 仅弹窗 / 设置 / overlay，勿把核心逻辑写进前端、勿让 UI 模块互耦。
- **托盘驻留**：关窗 = hide；托盘退出才进程结束。`main` 默认不可见，冷启动由前端 show。
- **配置事实来源**：`config.json` 的 `services[]`；协议 `openai_chat` / `claude_messages` / `mock` / `microsoft_edge`（`provider_for_service`）。`AppConfig` 另含 `updateChannel`（`stable`/`beta`）与 `autoCheckUpdate`（默认 `true`）。
- **配置同步**：设置页 `syncFromBackend`；`save_app_config` → `app-config:changed` 刷新弹窗卡片。
- **批次翻译**：启用服务保序并发；事件带 `serviceInstanceId`；单服务失败不影响其他。
- **快捷键**：`Alt+D` 划词、`Alt+S` 截图译、`Alt+O` 仅识别；新快捷键同步 capabilities。
- **历史 / 日志**：SQLite 历史与分文件日志；失败 best-effort，不挡翻译主路径。
- **检查更新**：command `check_for_update`（GitHub Releases + 通道过滤 + semver）；设置页手动检查 → toast/Dialog → `open_url` 浏览器下载；启动时若 `autoCheckUpdate` 则后端 best-effort 检查，有更新才弹系统 dialog（「前往下载」/「稍后」），确认后后端 `open_url`。不做应用内安装、无 updater 插件。

## 开发说明

1. **`AGENTS.md` 与 `CLAUDE.md` 内容同步**（改一处必须改另一处）。
2. Superpowers 产出的 spec/plan 立即 `git commit`（无需再问）。
3. 自定义 skill 前缀 `my-`；**不得编辑** `<!-- superpowers-zh:begin/end -->` 区块（插件维护）。
4. **Pot 四步**：阅读分析 → Architecture Proposal → 等确认 → 按自有设计编码（禁止照搬）。

## 协作规范

1. 需求按功能分组提交（每组 2–4 个相关项）。
2. **文档同步是收尾硬门禁**（spec/plan 勾选、README、roadmap、架构文档）；finish 前必须已同步。
3. 对话含图片时先确认再继续；版本号变更同步相关文件；插件/技能变更同步 `plugins.md`。
4. **子代理模型**：见 [docs/agent-model-policy.md](docs/agent-model-policy.md)。Claude/Codex 默认中模型；Codex 用 `gpt-5.6-luna|terra|sol` + 推理强度；任务卡写实名；Grok Build 继承会话模型；连续失败不得私自升级模型。
5. **任务规模与流程选择**（先分级；**与 skill 冲突时以本规范为准**）。`using-superpowers` 的「1% 检查」≠ 必须跑满 spec→plan→编码。

   | 档 | 判定（多数即可） | 默认节奏 |
   |----|------------------|----------|
   | **S** | bug/回归；≤2 文件或单点；配置/文案；用户说直接修 | 常可直接实现。Bug 先 `systematic-debugging`。**是否 brainstorm 由 agent 判断**（有歧义/取舍时建议先做） |
   | **M** | ~2–4 文件、边界较清的小增强 | 本对话内完成。**建议先判断是否 brainstorm**；需要落设计时写**标准完整 spec**（同 L 的 design 文档，不写「简版 spec」），**通常不写 plan**，用户确认后本对话编码 |
   | **L** | 新子系统/协议或数据模型/大面跨层；或用户要求完整三阶段 | 下方标准三阶段（brainstorm → **完整** spec → plan → 编码 + 交接） |

   **头脑风暴（各档通用）：**
   - 不把 brainstorming 锁死在 L。S/M 也可先 brainstorm；agent 自行判断（意图不清、多方案、接口/行为取舍、怕返工 → 先做；纯笔误/单一确定修法 → 可跳过）。
   - 用户可口令覆盖：`先 brainstorm` / `直接写`。
   - brainstorm 结束后：若设计值得固化，写 **`docs/superpowers/specs/` 下的标准 design spec**（brainstorming skill 常规产出，立即 commit）。**禁止另造「简版/短规格」体裁**——要么不写 spec，要么写完整标准 spec。
   - **S/M brainstorm 后默认不自动 `writing-plans`**（规模通常撑不起正式 plan）；用户批准设计后在本对话实现。仅当实现步骤已多到需要拆任务清单时，再与用户确认是否补 plan 或升 L。

   **硬规则：** 默认偏 S/M，不确定则问一句、**不得默认升 L**；`直接修/小改`→S，`走完整流程/先写 plan/按三阶段`→L；Bug 默认 S（根因变架构/协议再协商升档）；回复可标 `规模：S|M|L`。

   **L 档：标准三阶段**（仅 L 默认走满；每阶段独立对话，禁止同对话跨阶段）
   - 1. `brainstorming` → 标准 design spec → commit  
   - 2. `writing-plans` → 标准 plan → commit  
   - 3. `executing-plans` 或 `subagent-driven-development`
   - 阶段收尾：skill 若要直接进下一阶段，**不得照做**；`AskUserQuestion`——brainstorm 后 ① 本对话 writing-plans / ② spec→plan 交接并停下（推荐）；plan 后 ① 本对话编码 / ② plan→执行交接并停下（推荐）。
   - 编码前再问：子代理驱动 vs 内联（**不得默认内联**）。交接模板见 [handoff-templates](docs/superpowers/handoff-templates.md)（**仅 L**）；只打印不落盘。收尾：文档同步 → `finishing-a-development-branch`。

## 提交规范

Conventional Commits：`<type>(<scope>): <中文描述>`  
type：`feat` / `fix` / `perf` / `refactor` / `docs` / `chore` / `style` / `test` / `ci`

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
