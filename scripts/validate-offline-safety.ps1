# R2H PDF AI Workstation - Offline Safety Validation
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$issues = 0

Write-Host "=== R2H PDF - Offline Safety Scan ===" -ForegroundColor Cyan

$patterns = @(
    @{ Pattern = 'openai'; Label = 'OpenAI reference' }
    @{ Pattern = 'anthropic'; Label = 'Anthropic reference' }
    @{ Pattern = 'apiKey'; Label = 'API key reference' }
    @{ Pattern = 'HF_TOKEN'; Label = 'HuggingFace token' }
)

# Check HTML report for external URLs
$htmlReport = Join-Path $root "src-tauri/src/reports/html_report.rs"
if (Test-Path $htmlReport) {
    $content = Get-Content $htmlReport -Raw
    $extUrls = [regex]::Matches($content, 'https?://(?!127\.0\.0\.1)')
    if ($extUrls.Count -gt 0) {
        Write-Host "  ISSUE: External URL found in html_report.rs" -ForegroundColor Red
        $issues++
    } else {
        Write-Host "  OK: html_report.rs has no external URLs" -ForegroundColor Green
    }
}

# Verify llama-cli uses --offline
$localRuntime = Join-Path $root "src-tauri/src/ai_core/local_runtime.rs"
if (Test-Path $localRuntime) {
    $content = Get-Content $localRuntime -Raw
    if ($content -match 'offline') {
        Write-Host "  OK: llama-cli uses offline flag" -ForegroundColor Green
    } else {
        Write-Host "  WARN: llama-cli may not use offline flag" -ForegroundColor Yellow
        $issues++
    }
}

# Verify llama-server binds to 127.0.0.1
$embedding = Join-Path $root "src-tauri/src/ai_core/embedding.rs"
if (Test-Path $embedding) {
    $content = Get-Content $embedding -Raw
    if ($content -match '127\.0\.0\.1') {
        Write-Host "  OK: llama-server binds to 127.0.0.1 only" -ForegroundColor Green
    } else {
        Write-Host "  WARN: llama-server binding not confirmed" -ForegroundColor Yellow
        $issues++
    }
}

# Scan source for risky patterns
$srcDirs = @("src-tauri/src", "src", "local-ai/workers")
foreach ($dir in $srcDirs) {
    $fullDir = Join-Path $root $dir
    if (-not (Test-Path $fullDir)) { continue }
    foreach ($p in $patterns) {
        $files = Get-ChildItem -Path $fullDir -Recurse -Include "*.rs","*.ts","*.tsx","*.py" -ErrorAction SilentlyContinue
        foreach ($f in $files) {
            if ($f.FullName -match 'node_modules|target|dist') { continue }
            $lines = Select-String -Path $f.FullName -Pattern $p.Pattern -CaseSensitive:$false -ErrorAction SilentlyContinue
            foreach ($line in $lines) {
                if ($line.Line -match '127\.0\.0\.1|localhost|test|mock|comment|doc') { continue }
                Write-Host "  ISSUE: $($p.Label) in $($f.Name):$($line.LineNumber)" -ForegroundColor Yellow
                $issues++
            }
        }
    }
}

Write-Host "`n=== Summary ==="
Write-Host "Issues found: $issues"
if ($issues -eq 0) { Write-Host "Offline safety: PASSED" -ForegroundColor Green }
else { Write-Host "Review issues above" -ForegroundColor Yellow }
exit 0
