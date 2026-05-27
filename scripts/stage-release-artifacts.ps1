param(
  [string]$Version = ""
)

$ErrorActionPreference = "Stop"

$dist = Join-Path (Get-Location) "dist-release"
if (Test-Path $dist) {
  Remove-Item $dist -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $dist | Out-Null

$nsis = Get-ChildItem ".\src-tauri\target\release\bundle\nsis" -Filter "*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$msiFiles = Get-ChildItem ".\src-tauri\target\release\bundle\msi" -Filter "*.msi" -ErrorAction SilentlyContinue

if (-not $nsis) {
  throw "No NSIS setup.exe artifact found under src-tauri\target\release\bundle\nsis."
}

Copy-Item $nsis.FullName -Destination (Join-Path $dist "LocalChatBoxSetup.exe") -Force

foreach ($msi in $msiFiles) {
  Copy-Item $msi.FullName -Destination (Join-Path $dist $msi.Name) -Force
}

# Portable developer/tester ZIP. This is not the installer. It includes the built exe plus
# runtime/model folders if they exist, so testers can unzip and run without installing.
$portableRoot = Join-Path $env:TEMP ("LocalChatBoxPortable_" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $portableRoot | Out-Null

try {
  $exe = Get-ChildItem ".\src-tauri\target\release" -Filter "localchatbox.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $exe) {
    $exe = Get-ChildItem ".\src-tauri\target\release" -Filter "*.exe" -ErrorAction SilentlyContinue | Where-Object { $_.Name -notlike "*setup*" } | Select-Object -First 1
  }

  if ($exe) {
    Copy-Item $exe.FullName -Destination (Join-Path $portableRoot "LocalChatBox.exe") -Force
  }

  foreach ($folder in @("runtime", "models", "data", "logs", "LICENSES", "docs")) {
    if (Test-Path ".\$folder") {
      Copy-Item ".\$folder" -Destination (Join-Path $portableRoot $folder) -Recurse -Force
    }
  }

  Set-Content -Path (Join-Path $portableRoot "portable.localchatbox") -Value "Portable LocalChatBox marker. Delete this file only if you know what you are doing."
  Set-Content -Path (Join-Path $portableRoot "README_PORTABLE.txt") -Value @"
LocalChatBox portable test build

1. Put a GGUF model in models\ if one is not already included.
2. Put a llama-server runtime in runtime\win\ if one is not already included.
3. Run LocalChatBox.exe.
"@

  Compress-Archive -Path (Join-Path $portableRoot "*") -DestinationPath (Join-Path $dist "LocalChatBoxPortable.zip") -Force
}
finally {
  Remove-Item $portableRoot -Recurse -Force -ErrorAction SilentlyContinue
}

$files = Get-ChildItem $dist -File
foreach ($file in $files) {
  $hash = Get-FileHash $file.FullName -Algorithm SHA256
  "$($hash.Hash)  $($file.Name)" | Set-Content -Path "$($file.FullName).sha256"
}

Write-Host "Release artifacts staged:"
Get-ChildItem $dist | ForEach-Object { Write-Host " - $($_.Name)" }
