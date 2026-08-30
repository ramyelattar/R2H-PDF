$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\trial-expiry-smoke.md"
$licenseDir = Join-Path $root "release\smoke-results\license-trial-expiry-store"
$jsonPath = Join-Path $root "release\smoke-results\trial-expiry-smoke.json"
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
    & $exe --license-smoke trial-expiry | Out-Null
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
# Trial Expiry Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Executable: $exe
- Trial state store: $licenseDir
- JSON evidence: $jsonPath
- Trial policy: $($parsed.trial_policy)
- Expired trial proof:
$probes
- Date rollback mitigation: future last-seen trial state is treated as locked.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Trial expiry smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
