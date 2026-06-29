# 插件与技能清单

> 新增或升级插件/技能后，必须同步更新本文件。

## 插件

通过 npm 等包管理器安装，可运行升级命令更新。

### superpowers-zh

- **安装方式**：`npx superpowers-zh@latest`
- **功能**：AI 编程超能力中文版，提供 20 个 skills（brainstorming、TDD、code-review、systematic-debugging 等），覆盖开发全流程
- **来源**：https://github.com/jnMetaCode/superpowers-zh
- **升级命令**：`npx superpowers-zh@latest`

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
