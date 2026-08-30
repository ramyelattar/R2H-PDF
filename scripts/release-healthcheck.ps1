# R2H PDF AI Workstation - Release Healthcheck
# Runs all validation gates in sequence.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$results = @()
$failed = $false

Write-Host "=== R2H PDF - Release Healthcheck ===" -ForegroundColor Cyan
Write-Host "Root: $root" -ForegroundColor DarkGray

function Run-Step($label, $cmd) {
    Write-Host ""
    Write-Host "--- $label ---" -ForegroundColor White
    try {
        $output = Invoke-Expression $cmd 2>&1 | Out-String
        if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne $null) {
            Write-Host "  FAILED (exit $LASTEXITCODE)" -ForegroundColor Red
            $script:results += @{ Step = $label; Status = "FAILED"; Exit = $LASTEXITCODE }
            $script:failed = $true
        } else {
            Write-Host "  PASSED" -ForegroundColor Green
            $script:results += @{ Step = $label; Status = "PASSED"; Exit = 0 }
        }
    } catch {
        Write-Host "  ERROR: $_" -ForegroundColor Red
        $script:results += @{ Step = $label; Status = "ERROR"; Exit = -1 }
        $script:failed = $true
    }
}

Push-Location $root

Run-Step "TypeScript typecheck" "pnpm typecheck"
Run-Step "ESLint" "pnpm lint"
Run-Step "Vitest" "npx vitest --run --fileParallelism=false"
Run-Step "Cargo check" "cd src-tauri; cargo check 2>&1 | Out-Null; cd .."
Run-Step "Cargo test" "cd src-tauri; cargo test --lib 2>&1 | Out-Null; cd .."
Run-Step "Python compile" "py -m compileall local-ai\workers"
Run-Step "Local AI validation" "powershell -ExecutionPolicy Bypass -File scripts\validate-local-ai.ps1"
Run-Step "Offline safety scan" "powershell -ExecutionPolicy Bypass -File scripts\validate-offline-safety.ps1"

Pop-Location

Write-Host ""
Write-Host "=== RESULTS ===" -ForegroundColor Cyan
foreach ($r in $results) {
    $color = if ($r.Status -eq "PASSED") { "Green" } elseif ($r.Status -eq "FAILED") { "Red" } else { "Yellow" }
    Write-Host "  [$($r.Status)] $($r.Step)" -ForegroundColor $color
}

$passCount = ($results | Where-Object { $_.Status -eq "PASSED" }).Count
$totalCount = $results.Count
if ($failed) { $summaryColor = "Red" } else { $summaryColor = "Green" }
Write-Host ""
Write-Host "$passCount / $totalCount checks passed." -ForegroundColor $summaryColor

if ($failed) {
    Write-Host ""
    Write-Host "RELEASE HEALTHCHECK: FAILED" -ForegroundColor Red
    exit 1
} else {
    Write-Host ""
    Write-Host "RELEASE HEALTHCHECK: PASSED" -ForegroundColor Green
    exit 0
}
