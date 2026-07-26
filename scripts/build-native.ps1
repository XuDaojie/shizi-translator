# Build Windows native UI host(s) under native/windows/ and stage next to the Tauri app.
# Currently stages the translation popup process (Shizi.Popup); more native surfaces
# can be added here later without renaming the npm entrypoints (native:dev / native:release).
#
# Usage (repo root):
#   powershell -NoProfile -ExecutionPolicy Bypass -File ./scripts/build-native.ps1
#   powershell ... -File ./scripts/build-native.ps1 -Configuration Debug -SkipBuildIfFresh
# Env:
#   SHIZI_POPUP_NATIVE_STRICT=1  -> non-zero exit on failure (CI)
# Default: best-effort (warn + exit 0) so machines without .NET still run tauri.

param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",

    # Skip dotnet build when existing exe is newer than sources (dev speed).
    [switch]$SkipBuildIfFresh
)

$ErrorActionPreference = "Stop"
$Strict = $env:SHIZI_POPUP_NATIVE_STRICT -eq "1"
$LogPrefix = "[native]"

function Write-WarnMsg([string]$msg) {
    Write-Host "$LogPrefix WARN: $msg" -ForegroundColor Yellow
}

function Write-Info([string]$msg) {
    Write-Host "$LogPrefix $msg"
}

function Ensure-Placeholder([string]$Dest) {
    if (-not (Test-Path $Dest)) {
        New-Item -ItemType Directory -Path $Dest -Force | Out-Null
    }
    $marker = Join-Path $Dest ".shizi-popup-native-placeholder"
    if (-not (Test-Path (Join-Path $Dest "Shizi.Popup.exe"))) {
        Set-Content -Path $marker -Value "Run npm run native:release (or native:dev) after installing .NET SDK." -Encoding utf8
    }
}

function Fail-Or-Warn([string]$msg) {
    $repo = Resolve-Path (Join-Path $PSScriptRoot "..")
    Ensure-Placeholder (Join-Path $repo "src-tauri/resources/popup-native")
    if ($Strict) {
        Write-Error $msg
        exit 1
    }
    Write-WarnMsg $msg
    exit 0
}

$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
# --- Windows native hosts (extend this list when more native apps ship) ---
$Csproj = Join-Path $RepoRoot "native/windows/popup/Shizi.Popup.csproj"
$SrcWinX64 = Join-Path $RepoRoot "native/windows/popup/bin/x64/$Configuration/net8.0-windows10.0.19041.0/win-x64"
# Deploy folder name is still popup-native: Rust host looks here for the popup process.
# Future multi-host layout can grow under resources/native/ without changing npm command names.
$DestResources = Join-Path $RepoRoot "src-tauri/resources/popup-native"
$DestRelease = Join-Path $RepoRoot "src-tauri/target/release/popup-native"
$DestDebug = Join-Path $RepoRoot "src-tauri/target/debug/popup-native"
$PopupProjDir = Join-Path $RepoRoot "native/windows/popup"

if (-not (Test-Path $Csproj)) {
    Fail-Or-Warn "missing $Csproj"
}

function Test-SourcesNewerThan([string]$ExePath, [string]$ProjDir) {
    if (-not (Test-Path $ExePath)) {
        return $true
    }
    $exeTime = (Get-Item $ExePath).LastWriteTimeUtc
    $sources = Get-ChildItem -Path $ProjDir -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Extension -match '\.(cs|xaml|csproj)$' -and
            $_.FullName -notmatch '[\\/](bin|obj)[\\/]'
        }
    foreach ($s in $sources) {
        if ($s.LastWriteTimeUtc -gt $exeTime) {
            return $true
        }
    }
    return $false
}

# Stop leftover native popup host so the next ensure loads a fresh binary.
try {
    Get-Process -Name "Shizi.Popup" -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
} catch {
    # ignore
}

$Exe = Join-Path $SrcWinX64 "Shizi.Popup.exe"
$needBuild = $true
if ($SkipBuildIfFresh -and -not (Test-SourcesNewerThan $Exe $PopupProjDir)) {
    $needBuild = $false
    Write-Info "sources unchanged vs $Configuration exe; skip dotnet build"
}

if ($needBuild) {
    Write-Info "dotnet build $Configuration x64 (native/windows/popup)..."
    try {
        & dotnet build $Csproj -c $Configuration -p:Platform=x64 --nologo -v q
        if ($LASTEXITCODE -ne 0) {
            Fail-Or-Warn "dotnet build failed (exit=$LASTEXITCODE)"
        }
    } catch {
        Fail-Or-Warn "dotnet not available: $_"
    }
}

if (-not (Test-Path $Exe)) {
    Fail-Or-Warn "build output missing: $Exe"
}

function Copy-NativeTree([string]$Dest) {
    if (Test-Path $Dest) {
        Remove-Item -Recurse -Force $Dest
    }
    $parent = Split-Path $Dest -Parent
    if (-not (Test-Path $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }
    Copy-Item -Recurse -Force $SrcWinX64 $Dest
    if ($Dest -eq $DestResources) {
        $keep = Join-Path $Dest ".gitkeep"
        if (-not (Test-Path $keep)) {
            New-Item -ItemType File -Path $keep -Force | Out-Null
        }
    }
    Write-Info "staged -> $Dest"
}

Copy-NativeTree $DestResources

# Always stage beside both cargo profiles. host.rs prefers <shizi.exe>/popup-native first.
foreach ($d in @($DestDebug, $DestRelease)) {
    Copy-NativeTree $d
}

Write-Info "done ($Configuration). Tauri loads staged native hosts next to shizi.exe."
exit 0
