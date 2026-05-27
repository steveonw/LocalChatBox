param(
  [switch]$SkipFrontendBuild
)

$ErrorActionPreference = "Stop"

Write-Host "LocalChatBox Windows installer build helper" -ForegroundColor Cyan

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
  throw "Node.js was not found on PATH."
}

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
  throw "npm was not found on PATH."
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
  throw "Rust/Cargo was not found on PATH. Install the Rust MSVC toolchain."
}

if (-not (Test-Path ".\src-tauri\tauri.conf.json")) {
  throw "Run this script from the LocalChatBox project root."
}

if (-not $SkipFrontendBuild) {
  npm install
  npm run build
}

Push-Location ".\src-tauri"
try {
  cargo check
}
finally {
  Pop-Location
}

npm run tauri:build

Write-Host "Build complete. Check src-tauri\target\release\bundle for NSIS/MSI artifacts." -ForegroundColor Green
