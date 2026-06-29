# 配置 git pre-commit hook

param(
    [string]$TargetDir = "."
)

if (-not (Test-Path "$TargetDir\.git")) {
    Write-Error "Error: $TargetDir is not a git repository"
    exit 1
}

# 设置 git hooks 目录
Set-Location $TargetDir
git config core.hooksPath .claude/hooks

# 设置执行权限（Windows 上通过 git 更新索引）
$preCommitPath = ".claude/hooks/pre-commit"
if (Test-Path $preCommitPath) {
    git update-index --chmod=+x $preCommitPath
}

Write-Host "Git pre-commit hook configured successfully!"
