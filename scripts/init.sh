#!/bin/bash
# 配置 git pre-commit hook

TARGET_DIR=${1:-.}

if [ ! -d "$TARGET_DIR/.git" ]; then
    echo "Error: $TARGET_DIR is not a git repository"
    exit 1
fi

# 设置 git hooks 目录
cd "$TARGET_DIR"
git config core.hooksPath .claude/hooks
chmod +x .claude/hooks/pre-commit

echo "Git pre-commit hook configured successfully!"
