---
name: my-commit
description: "根据当前 git 工作区修改自动分析生成 Conventional Commits 格式的提交信息并提交。用户输入 /my-commit 时触发。"
version: "1.0.0"
license: MIT
---

# 自动生成提交信息

根据当前 git 工作区修改自动分析生成 Conventional Commits 格式的提交信息并提交。

## 使用方式

用户输入 `/my-commit` 时触发。

## 流程

1. 读取 `git diff --cached` 获取暂存区变更
2. 分析变更内容，生成符合 Conventional Commits 的提交信息
3. 执行 `git commit -m "<生成的提交信息>"`
4. 输出提交结果

## 提交信息格式

```
<type>(<scope>): <中文描述>

[可选正文]
[可选脚注]
```

常用 type：
- `feat`: 新功能
- `fix`: 修复 bug
- `perf`: 性能优化
- `refactor`: 重构
- `docs`: 文档更新
- `chore`: 构建/工具变更
- `style`: 代码格式
- `test`: 测试相关
- `ci`: CI 配置

## 示例

```bash
# 用户暂存了修改
git add src/config.js

# 触发 skill
/my-commit

# 自动生成并提交
git commit -m "feat(config): 新增种子配置数据"
```
