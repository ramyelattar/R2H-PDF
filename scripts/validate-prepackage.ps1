# R2H PDF - Pre-Package Validation
# Runs ALL validation gates that must pass before any installer build.
# Does NOT build any installer.

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$failed = $false
$results = @()

Write-Host "=== R2H PDF - Pre-Package Validation ===" -ForegroundColor Cyan
Write-Host "Root: $root"
Write-Host ""

function Run-Gate($label, $cmd) {
    Write-Host "--- $label ---" -ForegroundColor White
    try {
        Invoke-Expression $cmd 2>&1 | Out-Null
        if ($LASTEXITCODE -ne 0 -and $null -ne $LASTEXITCODE) {
            Write-Host "  FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
            $script:results += @{ Step = $label; Status = "FAILED" }
            $script:failed = $true
        } else {
            Write-Host "  PASSED" -ForegroundColor Green
            $script:results += @{ Step = $label; Status = "PASSED" }
        }
    } catch {
        Write-Host "  ERROR: $_" -ForegroundColor Red
        $script:results += @{ Step = $label; Status = "ERROR" }
        $script:failed = $true
    }
}

Push-Location $root

Run-Gate "TypeScript typecheck" "pnpm typecheck"
Run-Gate "ESLint" "pnpm lint"
Run-Gate "Frontend tests" "pnpm test -- --fileParallelism=false"
Run-Gate "IPC contract tests" "npx vitest --run src/lib/ipc-contract.test.ts"
Run-Gate "Cargo check" "Push-Location src-tauri; cargo check 2>&1 | Out-Null; Pop-Location"
Run-Gate "Cargo test" "Push-Location src-tauri; cargo test --lib 2>&1 | Out-Null; Pop-Location"
Run-Gate "Python compile" "py -m compileall local-ai\workers"
Run-Gate "Local AI validation" "powershell -ExecutionPolicy Bypass -File scripts\validate-local-ai.ps1"
Run-Gate "Offline safety" "powershell -ExecutionPolicy Bypass -File scripts\validate-offline-safety.ps1"
Run-Gate "Version check" "powershell -ExecutionPolicy Bypass -File scripts\check-version.ps1"

Pop-Location

Write-Host ""
Write-Host "=== RESULTS ===" -ForegroundColor Cyan
foreach ($r in $results) {
    $color = if ($r.Status -eq "PASSED") { "Green" } else { "Red" }
    Write-Host "  [$($r.Status)] $($r.Step)" -ForegroundColor $color
}

$passCount = ($results | Where-Object { $_.Status -eq "PASSED" }).Count
$totalCount = $results.Count

Write-Host ""
Write-Host "$passCount / $totalCount gates passed."

if ($failed) {
    Write-Host ""
    Write-Host "PREPACKAGE STATUS: FAIL" -ForegroundColor Red
    exit 1
} else {
    Write-Host ""
    Write-Host "PREPACKAGE STATUS: PASS" -ForegroundColor Green
    exit 0
}
