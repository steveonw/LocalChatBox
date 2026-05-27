<#
LocalChatBox helper script.

This script does not download binaries automatically. It prepares the runtime folder and prints the
expected filenames. Use llama.cpp releases or your own llama.cpp build, then copy/rename the files.

Why manual?
- CUDA builds must match the target machine's driver/runtime assumptions.
- AVX2, AVX-only, and basic CPU builds should be selected for the oldest machines you want to support.
- Shipping third-party binaries may require your own license review.
#>

$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$RuntimeDir = Join-Path $ProjectRoot "runtime\win"
New-Item -ItemType Directory -Force -Path $RuntimeDir | Out-Null

Write-Host ""
Write-Host "LocalChatBox runtime folder:" -ForegroundColor Cyan
Write-Host "  $RuntimeDir"
Write-Host ""
Write-Host "Copy llama.cpp server executables here using these names:" -ForegroundColor Yellow
Write-Host "  llama-server-cuda.exe"
Write-Host "  llama-server-cpu-avx2.exe"
Write-Host "  llama-server-cpu-avx.exe"
Write-Host "  llama-server-cpu-basic.exe"
Write-Host "  llama-server.exe              # optional generic fallback"
Write-Host ""
Write-Host "Then run from the project root:" -ForegroundColor Cyan
Write-Host "  npm install"
Write-Host "  npm run tauri:dev"
Write-Host ""
