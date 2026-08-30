# R2H PDF - IPC Contract Validation
# Runs focused IPC contract tests that verify payload shapes match Tauri 2 expectations.
# Must fail fast if any camelCase/snake_case mismatch exists.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

Write-Host "=== R2H PDF - IPC Contract Validation ===" -ForegroundColor Cyan

Push-Location $root

try {
    Write-Host "  Running IPC contract tests..."
    $output = npx vitest --run src/lib/ipc-contract.test.ts 2>&1 | Out-String

    if ($LASTEXITCODE -ne 0) {
        Write-Host "  FAILED: IPC contract tests have failures" -ForegroundColor Red
        Write-Host $output
        exit 1
    }

    # Extract test count
    if ($output -match "(\d+) passed") {
        Write-Host "  PASSED: $($Matches[1]) IPC contract tests" -ForegroundColor Green
    } else {
        Write-Host "  PASSED" -ForegroundColor Green
    }

    exit 0
}
catch {
    Write-Host "  ERROR: $_" -ForegroundColor Red
    exit 1
}
finally {
    Pop-Location
}
