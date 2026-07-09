# 插件与技能清单

> 新增或升级插件/技能后，必须同步更新本文件。

## 插件

通过 Claude Code plugin marketplace、npm 等方式安装，可运行对应升级命令更新。

### superpowers-zh

- **安装方式**：`npx superpowers-zh@latest`
- **功能**：AI 编程超能力中文版，提供 20 个 skills（brainstorming、TDD、code-review、systematic-debugging 等），覆盖开发全流程
- **来源**：https://github.com/jnMetaCode/superpowers-zh
- **升级命令**：`npx superpowers-zh@latest`

### Chrome DevTools MCP

- **配置方式**：已添加到 ~/.codex/config.toml 的 [mcp_servers.chrome-devtools]
- **功能**：让 AI 编码助手控制和检查 Chrome 浏览器，提供 DevTools 级别的能力：性能分析、网络请求、截图、控制浏览器等
- **来源**：https://github.com/diegorafs/Chrome-DevTools-MCP
- **升级命令**：
px chrome-devtools-mcp@latest（自动使用最新版）

### Tauri MCP

- **配置方式**：用户级 Claude Code 配置（`~/.claude.json` 的 `mcpServers.tauri`），启动命令 `cmd /c npx -y @hypothesi/tauri-mcp-server`
- **功能**：让 AI 助手构建/测试/调试 Tauri v2 应用，提供 21 个工具：UI 自动化（截图、点击、输入、DOM 快照、元素选取）、IPC 监控与执行、窗口管理、控制台/系统日志、移动设备列表
- **来源**：https://github.com/hypothesi/mcp-server-tauri
- **升级命令**：`npx @hypothesi/tauri-mcp-server`（npx 自动取最新版）
- **项目集成**：shizi 已在 `src-tauri` 集成 `tauri-plugin-mcp-bridge = "0.2"`（实际 v0.2.2），`#[cfg(debug_assertions)]` 下注册、WebSocket 绑 `127.0.0.1`，默认端口 9223，被占用时在 9223-9322 自动顺延；release 包不带此插件。前端无需 TS 绑定

### ponytail

- **安装方式**：`claude plugin marketplace add DietrichGebert/ponytail` 后运行 `claude plugin install ponytail@ponytail`
- **功能**：Claude Code 简化实现规则集，强调复用现有能力、避免过度工程化，并提供 `/ponytail`、`/ponytail-review`、`/ponytail-audit` 等命令
- **来源**：https://github.com/DietrichGebert/ponytail
- **作用域**：user
- **升级命令**：`claude plugin update ponytail@ponytail`

## 技能

手动安装到 `.claude/skills/` 目录，按自身文档升级。

### AnySearch

- **安装方式**：手动安装（从 GitHub 下载文件到 `.claude/skills/anysearch/`）
- **来源**：https://github.com/anysearch-ai/anysearch-skill
- **版本**：v2.0.0
- **功能**：实时搜索引擎，支持网页搜索、垂直领域搜索、并行批量搜索、URL 内容提取
- **运行环境**：Node.js（因项目 `"type": "module"`，已将脚本复制为 `.cjs` 兼容）
- **配置说明**：
  - API Key 存储在 `.env`，已加入 `.gitignore` 不提交
  - 运行环境配置存储在 `runtime.conf`，已加入 `.gitignore` 不提交
  - 匿名可用，配置 Key 可获得更高调用额度
- **升级方式**：从 GitHub 重新下载覆盖所有文件，保留 `.env` 和 `runtime.conf`，重新复制 `.cjs`

### my-commit（自定义）

- **安装方式**：手动创建
- **功能**：根据当前 git 工作区修改自动分析生成 Conventional Commits 格式的提交信息并提交
- **升级方式**：直接编辑 `.claude/skills/my-commit/SKILL.md`

## 前端依赖（Vite 工程）

设置页引入 Vite 构建步骤，依赖如下：

| 依赖 | 版本 | 用途 |
|------|------|------|
| Vite | ^7 | 前端构建工具与 dev server |
| Vue | ^3.5 | 前端框架 |
| `@vitejs/plugin-vue` | ^6 | Vite Vue 3 SFC 编译插件 |
| Tailwind CSS | ^4 | 原子化 CSS 框架 |
| `@tailwindcss/vite` | ^4 | Tailwind v4 Vite 插件 |
| shadcn-vue | latest | UI 组件库（new-york 风格，按需拷贝源码至 `frontend/src/components/ui/`） |
| reka-ui | ^2 | shadcn-vue 底层无样式组件库 |
| `@iconify/vue` | ^4 | 图标组件 |
| class-variance-authority | ^0.7 | 组件变体管理 |
| clsx | ^2 | 类名合并工具 |
| tailwind-merge | ^3 | Tailwind 类名冲突合并 |
| TypeScript | ^5.6 | 类型系统 |
| vue-tsc | ^2 | Vue TypeScript 类型检查 |
| vitest | ^3 | 单元测试框架 |
