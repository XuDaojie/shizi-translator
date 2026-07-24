# 构建 WinUI 弹窗并拷贝到 Tauri 可打包布局。
# 用法（仓库根目录）:
#   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/copy-popup-native.ps1
# 环境变量:
#   SHIZI_POPUP_NATIVE_STRICT=1  — 构建/拷贝失败时以非 0 退出（CI 打包用）
# 默认 best-effort：失败仅警告并 exit 0，避免阻断未装 .NET 的环境。

$ErrorActionPreference = "Stop"
$Strict = $env:SHIZI_POPUP_NATIVE_STRICT -eq "1"

function Write-WarnMsg([string]$msg) {
    Write-Host "[copy-popup-native] WARN: $msg" -ForegroundColor Yellow
}

function Write-Info([string]$msg) {
    Write-Host "[copy-popup-native] $msg"
}

function Ensure-Placeholder([string]$Dest) {
    # 保证 tauri.conf bundle.resources 源路径存在，避免未装 .NET 时打包直接失败
    if (-not (Test-Path $Dest)) {
        New-Item -ItemType Directory -Path $Dest -Force | Out-Null
    }
    $marker = Join-Path $Dest ".shizi-popup-native-placeholder"
    if (-not (Test-Path (Join-Path $Dest "Shizi.Popup.exe"))) {
        Set-Content -Path $marker -Value "Run scripts/copy-popup-native.ps1 after dotnet build to fill this directory." -Encoding utf8
    }
}

function Fail-Or-Warn([string]$msg) {
    Ensure-Placeholder (Join-Path (Resolve-Path (Join-Path $PSScriptRoot "..")) "src-tauri/resources/popup-native")
    if ($Strict) {
        Write-Error $msg
        exit 1
    }
    Write-WarnMsg $msg
    exit 0
}

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$Csproj = Join-Path $RepoRoot "native/windows/popup/Shizi.Popup.csproj"
$SrcWinX64 = Join-Path $RepoRoot "native/windows/popup/bin/x64/Release/net8.0-windows10.0.19041.0/win-x64"
$DestResources = Join-Path $RepoRoot "src-tauri/resources/popup-native"
$DestRelease = Join-Path $RepoRoot "src-tauri/target/release/popup-native"
$DestDebug = Join-Path $RepoRoot "src-tauri/target/debug/popup-native"

if (-not (Test-Path $Csproj)) {
    Fail-Or-Warn "找不到 $Csproj"
}

Write-Info "dotnet build Release x64..."
try {
    & dotnet build $Csproj -c Release -p:Platform=x64 --nologo -v q
    if ($LASTEXITCODE -ne 0) {
        Fail-Or-Warn "dotnet build 失败 (exit=$LASTEXITCODE)"
    }
} catch {
    Fail-Or-Warn "无法运行 dotnet: $_"
}

$Exe = Join-Path $SrcWinX64 "Shizi.Popup.exe"
if (-not (Test-Path $Exe)) {
    Fail-Or-Warn "构建后未找到 $Exe"
}

function Copy-PopupTree([string]$Dest) {
    if (Test-Path $Dest) {
        Remove-Item -Recurse -Force $Dest
    }
    $parent = Split-Path $Dest -Parent
    if (-not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    Copy-Item -Recurse -Force $SrcWinX64 $Dest
    Write-Info "已拷贝 -> $Dest"
}

Copy-PopupTree $DestResources

# 若已有 cargo 输出目录，同步到 exe 旁 popup-native/（与 host.rs 查找一致）
foreach ($d in @($DestRelease, $DestDebug)) {
    $parent = Split-Path $d -Parent
    if (Test-Path $parent) {
        Copy-PopupTree $d
    }
}

Write-Info "完成。打包时 tauri bundle.resources 将纳入 src-tauri/resources/popup-native（体积较大：WASDK 自包含）。"
exit 0
