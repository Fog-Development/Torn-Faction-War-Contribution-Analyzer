# gui.ps1 - Build the GUI stack and either launch dev mode or produce a release installer.
#
# Usage (run from anywhere in the repo):
#   .\gui.ps1          # build CLI (debug) + launch cargo tauri dev
#   .\gui.ps1 release  # build CLI (release) + cargo tauri build -> installer

param(
    [Parameter(Position = 0)]
    [ValidateSet("dev", "release")]
    [string]$Mode = "dev"
)

$ErrorActionPreference = "Stop"

# --------------------------------------------------------------------------
# 0. Locate workspace root (directory that has both Cargo.toml and crates\)
# --------------------------------------------------------------------------
$root = if ($PSScriptRoot) { $PSScriptRoot } else { (Get-Location).Path }

$found = $false
$candidate = $root
for ($i = 0; $i -lt 10; $i++) {
    if ((Test-Path "$candidate\Cargo.toml") -and (Test-Path "$candidate\crates")) {
        $root = $candidate
        $found = $true
        break
    }
    $parent = Split-Path $candidate -Parent
    if ($parent -eq $candidate) { break }
    $candidate = $parent
}

if (-not $found) {
    Write-Host "ERROR: Could not locate workspace root. Run this script from inside the repo." -ForegroundColor Red
    exit 1
}

$guiDir = "$root\crates\twr-gui"

Write-Host ""
Write-Host "=== Torn War Report - GUI build script ===" -ForegroundColor Cyan
Write-Host "Workspace : $root"
Write-Host "Mode      : $Mode"
Write-Host ""

# --------------------------------------------------------------------------
# 1. Ensure tauri-cli is installed
# --------------------------------------------------------------------------
Write-Host "[ 1/4 ] Checking for tauri-cli..." -ForegroundColor Yellow

$tauriVersion = cargo tauri --version 2>&1
$tauriInstalled = ($LASTEXITCODE -eq 0)

if ($tauriInstalled) {
    Write-Host "        OK ($tauriVersion)" -ForegroundColor Green
}

if (-not $tauriInstalled) {
    Write-Host "        Not found - installing tauri-cli (takes a minute the first time)..."
    cargo install tauri-cli --version "^2" --locked
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Failed to install tauri-cli." -ForegroundColor Red
        exit 1
    }
    Write-Host "        Installed." -ForegroundColor Green
}

# --------------------------------------------------------------------------
# 2. Detect host target triple
# --------------------------------------------------------------------------
Write-Host ""
Write-Host "[ 2/4 ] Detecting target triple..." -ForegroundColor Yellow

$rustcOutput = rustc -vV 2>&1
$tripleLine  = $rustcOutput | Where-Object { $_ -match "^host:" }
$triple      = ($tripleLine -replace "host:\s*", "").Trim()

if (-not $triple) {
    Write-Host "ERROR: Could not determine host triple from 'rustc -vV'." -ForegroundColor Red
    exit 1
}
Write-Host "        $triple" -ForegroundColor Green

# --------------------------------------------------------------------------
# 3. Build twr-cli and copy sidecar into crates\twr-gui\binaries\
# --------------------------------------------------------------------------
Write-Host ""
Write-Host "[ 3/4 ] Building twr-cli ($Mode)..." -ForegroundColor Yellow

if ($Mode -eq "release") {
    cargo build -p twr-cli --release
    $srcBin = "$root\target\release\torn-war-report.exe"
} else {
    cargo build -p twr-cli
    $srcBin = "$root\target\debug\torn-war-report.exe"
}

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: twr-cli build failed." -ForegroundColor Red
    exit 1
}

$binDir  = "$guiDir\binaries"
$destBin = "$binDir\torn-war-report-$triple.exe"

New-Item -ItemType Directory -Force $binDir | Out-Null
Copy-Item -Force $srcBin $destBin
Write-Host "        Sidecar copied -> $destBin" -ForegroundColor Green

# --------------------------------------------------------------------------
# 4. Run Tauri
# --------------------------------------------------------------------------
Write-Host ""
Set-Location $guiDir

if ($Mode -eq "release") {
    Write-Host "[ 4/4 ] Building Tauri installer (release)..." -ForegroundColor Yellow
    Write-Host "        Output: $root\target\release\bundle\" -ForegroundColor Cyan
    cargo tauri build
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: cargo tauri build failed." -ForegroundColor Red
        exit 1
    }
    Write-Host ""
    Write-Host "Done! Installer -> $root\target\release\bundle\" -ForegroundColor Green
} else {
    Write-Host "[ 4/4 ] Launching dev window (Ctrl+C to stop)..." -ForegroundColor Yellow
    cargo tauri dev --no-watch
}
