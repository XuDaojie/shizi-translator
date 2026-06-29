# AI 开发脚手架

基于 Claude Code 的通用项目配置模板，兼容 Codex、opencode。

## 包含内容

- **CLAUDE.md** — Claude Code 主配置（opencode 直接复用）
- **AGENTS.md** — Codex 兼容配置（自动同步自 CLAUDE.md）
- **plugins.md** — 推荐插件与技能清单
- **.claude/** — Claude Code 配置目录
  - `settings.json` — 权限配置
  - `memory/MEMORY.md` — 记忆索引模板
  - `hooks/pre-commit` — 提交前检查
  - `skills/my-commit/` — 自定义提交技能示例
- **scripts/** — 工具脚本
  - `init.sh` — 配置 git pre-commit hook（macOS/Linux）
  - `init.ps1` — 配置 git pre-commit hook（Windows PowerShell）
  - `sync-agents.sh` — 同步 AGENTS.md

## 快速开始

### 1. 复制配置到项目

```bash
# 复制所有配置文件
cp ai-scaffold/CLAUDE.md ./your-project/
cp ai-scaffold/AGENTS.md ./your-project/
cp ai-scaffold/plugins.md ./your-project/
cp -r ai-scaffold/.claude/ ./your-project/
cp -r ai-scaffold/scripts/ ./your-project/
```

### 2. 配置 Git Hook

**Windows (PowerShell):**

```powershell
cd your-project
.\scripts\init.ps1
```

**macOS/Linux:**

```bash
cd your-project
chmod +x scripts/init.sh
chmod +x scripts/sync-agents.sh
chmod +x .claude/hooks/pre-commit
./scripts/init.sh
```

### 3. 验证配置

```bash
# 检查 Claude Code 是否识别配置
claude --version

# 测试 pre-commit hook
git add .
git commit -m "test: 验证 pre-commit hook"
```

## 自定义配置

### CLAUDE.md

根据项目需求填写以下部分：
- 项目介绍
- 项目结构
- 开发环境
- 开发说明
- 测试规范
- 协作规范
- 提交规范

### plugins.md

根据项目需求安装推荐的插件和技能。

### .claude/settings.json

根据项目需求调整权限配置。

## 跨平台支持

- ✅ Windows（Git for Windows）
- ✅ macOS
- ✅ Linux

## 注意事项

1. `AGENTS.md` 会自动同步 `CLAUDE.md` 的内容，请勿手动修改
2. `pre-commit` hook 会在提交时自动检查并同步配置
3. 请确保 `scripts/` 目录下的脚本有执行权限
