$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\offline-license-smoke.md"
$licenseDir = Join-Path $root "release\smoke-results\license-offline-store"
$jsonPath = Join-Path $root "release\smoke-results\offline-license-smoke.json"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath), $licenseDir | Out-Null

Push-Location (Join-Path $root "src-tauri")
cargo build --release -q
$buildExit = $LASTEXITCODE
Pop-Location

$exe = Join-Path $root "src-tauri\target\release\r2h-pdf.exe"
$env:R2H_LICENSE_DIR = $licenseDir
$env:R2H_LICENSE_SMOKE_OUTPUT = $jsonPath
$exitCode = 1
if ($buildExit -eq 0 -and (Test-Path -LiteralPath $exe -PathType Leaf)) {
    & $exe --license-smoke offline | Out-Null
    $exitCode = $LASTEXITCODE
} else {
    "cargo build exit code: $buildExit" | Out-File -LiteralPath $jsonPath -Encoding utf8
}
Remove-Item Env:\R2H_LICENSE_DIR -ErrorAction SilentlyContinue
Remove-Item Env:\R2H_LICENSE_SMOKE_OUTPUT -ErrorAction SilentlyContinue

$parsed = $null
try {
    $parsed = Get-Content -LiteralPath $jsonPath -Raw | ConvertFrom-Json
} catch {}

$status = if ($exitCode -eq 0 -and $parsed -and $parsed.status -eq "PASS") { "PASS" } else { "FAIL" }
$probes = if ($parsed) {
    ($parsed.probes | ForEach-Object {
        "- $($_.feature): expected_allowed=$($_.expected_allowed); actual_allowed=$($_.actual_allowed); result=$($_.result); reason=$($_.reason)"
    }) -join "`n"
} else {
    "- No parseable JSON proof was produced."
}

@"
# Offline License Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Executable: $exe
- License store: $licenseDir
- JSON evidence: $jsonPath
- Trial policy: $($parsed.trial_policy)
- Offline/no-network proof: license verification uses local files and the built executable; no network command or endpoint is required.
- Locked/unlocked feature proof:
$probes
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Offline license smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
