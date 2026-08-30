param(
    [Parameter(Mandatory=$true)][string]$TestName,
    [Parameter(Mandatory=$true)][string]$ReportPath,
    [string[]]$RequiredOutputs = @(),
    [switch]$Ignored
)

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$logsDir = Join-Path $root "release\logs"
$runId = Get-Date -Format "yyyyMMdd-HHmmss"
$safeName = ($TestName -replace '[^A-Za-z0-9_.-]+', '-').ToLowerInvariant()
$logPath = Join-Path $logsDir "$runId-$safeName.log"

New-Item -ItemType Directory -Force -Path `
    (Join-Path $root "demo\input"), `
    (Join-Path $root "demo\output"), `
    (Join-Path $root "release\smoke-results"), `
    $logsDir | Out-Null

powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "create-demo-pdfs.ps1")
if ($LASTEXITCODE -ne 0) {
    "Demo fixture generation failed." | Out-File -LiteralPath $logPath -Encoding utf8
    exit 1
}

Push-Location (Join-Path $root "src-tauri")
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$cargoArgs = @("test", "--lib", $TestName, "--", "--nocapture")
if ($Ignored) {
    $cargoArgs += "--ignored"
}
$output = cargo @cargoArgs 2>&1
$exitCode = $LASTEXITCODE
$sw.Stop()
Pop-Location

@(
    "Timestamp: $(Get-Date -Format 'yyyy-MM-dd HH:mm:ss K')"
    "Project root: $root"
    "Cargo test: $TestName"
    "Exit code: $exitCode"
    "Duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s"
    ""
    "=== OUTPUT ==="
    $output
) | Out-File -LiteralPath $logPath -Encoding utf8

$output | ForEach-Object { Write-Host $_ }

if ($exitCode -ne 0) {
    exit 1
}

if (-not (Test-Path -LiteralPath $ReportPath -PathType Leaf)) {
    Write-Host "Smoke report missing: $ReportPath" -ForegroundColor Red
    exit 1
}

$report = Get-Content -LiteralPath $ReportPath -Raw
$status = if ($report -match "(?m)^- Status: ([A-Z_]+)") { $Matches[1] } else { "UNKNOWN" }

foreach ($required in $RequiredOutputs) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        Write-Host "Required smoke output missing: $required" -ForegroundColor Red
        exit 1
    }
    $item = Get-Item -LiteralPath $required
    if ($item.Length -le 0) {
        Write-Host "Required smoke output is empty: $required" -ForegroundColor Red
        exit 1
    }
}

switch ($status) {
    "PASS" { exit 0 }
    "PASS_WITH_LIMITATION" { exit 4 }
    "NOT_IMPLEMENTED" { exit 2 }
    "MANUAL_CHECKLIST_ONLY" { exit 5 }
    "SKIPPED_WITH_REASON" { exit 3 }
    "SKIPPED" { exit 3 }
    default {
        Write-Host "Smoke report did not record PASS/NOT_IMPLEMENTED/SKIPPED: $status" -ForegroundColor Red
        exit 1
    }
}
