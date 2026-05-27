param(
    [Parameter(Mandatory = $true)]
    [string]$RuntimePath
)

if (-not (Test-Path $RuntimePath)) {
    Write-Error "Runtime binary not found: $RuntimePath"
    exit 1
}

Write-Host "Checking llama-server flags for: $RuntimePath"
Write-Host ""

$help = ""
try {
    $help = & $RuntimePath --help 2>&1 | Out-String
} catch {
    Write-Error "Could not run '$RuntimePath --help'. Error: $_"
    exit 1
}

function Test-Flag {
    param(
        [string]$Name,
        [string[]]$Patterns,
        [bool]$Required = $true
    )

    foreach ($pattern in $Patterns) {
        if ($help -match [regex]::Escape($pattern)) {
            Write-Host "[ok]   $Name ($pattern)"
            return $true
        }
    }

    if ($Required) {
        Write-Host "[miss] $Name -- required"
    } else {
        Write-Host "[warn] $Name -- optional/compatibility"
    }
    return $false
}

$ok = $true
$ok = (Test-Flag "API key auth" @("--api-key")) -and $ok
$ok = (Test-Flag "parallel/server slots" @("--parallel")) -and $ok
$uiModern = Test-Flag "modern UI disable spelling" @("--no-ui", "--ui") $false
$uiLegacy = Test-Flag "legacy UI disable spelling" @("--no-webui", "--webui") $false

Write-Host ""

if (-not ($uiModern -or $uiLegacy)) {
    Write-Host "[miss] No recognized UI disable flag found. LocalChatBox may still run, but the bundled llama-server web UI may remain enabled."
    $ok = $false
}

if ($ok) {
    Write-Host "Runtime flag check passed."
    exit 0
}

Write-Host "Runtime flag check found missing required or compatibility flags."
exit 2
