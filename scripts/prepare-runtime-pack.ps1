param(
  [switch]$Ci
)

$ErrorActionPreference = "Stop"

$runtimeDir = Join-Path (Get-Location) "runtime\win"
New-Item -ItemType Directory -Force -Path $runtimeDir | Out-Null

$runtimePackUrl = $env:LOCALCHATBOX_RUNTIME_PACK_URL
if ([string]::IsNullOrWhiteSpace($runtimePackUrl)) {
  Write-Host "LOCALCHATBOX_RUNTIME_PACK_URL is not set. Skipping runtime-pack download."
  return
}

$tempDir = Join-Path $env:TEMP ("LocalChatBoxRuntimePack_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tempDir | Out-Null

try {
  $zipPath = Join-Path $tempDir "runtime-pack.zip"
  Write-Host "Downloading runtime pack..."
  Invoke-WebRequest -Uri $runtimePackUrl -OutFile $zipPath

  $extractDir = Join-Path $tempDir "extract"
  New-Item -ItemType Directory -Force -Path $extractDir | Out-Null
  Expand-Archive -Path $zipPath -DestinationPath $extractDir -Force

  $found = Get-ChildItem $extractDir -Recurse -Filter "llama-server*.exe" -ErrorAction SilentlyContinue
  if ($found.Count -eq 0) {
    throw "Runtime pack downloaded, but no llama-server*.exe files were found."
  }

  foreach ($file in $found) {
    Copy-Item $file.FullName -Destination (Join-Path $runtimeDir $file.Name) -Force
    Write-Host "Installed runtime: $($file.Name)"
  }

  $shaFile = Get-ChildItem $extractDir -Recurse -Filter "*.sha256" -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($shaFile) {
    Copy-Item $shaFile.FullName -Destination (Join-Path $runtimeDir $shaFile.Name) -Force
    Write-Host "Copied checksum file: $($shaFile.Name)"
  }

  Write-Host "Runtime pack prepared."
}
finally {
  Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
