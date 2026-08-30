$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$corpusDir = Join-Path $root "demo\input\ocr-corpus"
$reportPath = Join-Path $root "release\smoke-results\ocr-corpus-smoke.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $corpusDir, (Split-Path $reportPath) | Out-Null

node (Join-Path $PSScriptRoot "create-ocr-corpus.cjs") | Out-Null
$manifest = Get-Content -LiteralPath (Join-Path $corpusDir "manifest.json") -Raw | ConvertFrom-Json
$rows = New-Object System.Collections.Generic.List[string]
$failures = 0
foreach ($item in $manifest) {
    $input = Join-Path $corpusDir $item.file
    $env:R2H_OCR_SMOKE_INPUT = $input
    $env:R2H_OCR_SMOKE_EXPECTED = if ($item.category -eq "negative" -or $item.category -eq "unsupported") { "NO_TEXT_EXPECTED_SENTINEL" } else { $item.expected }
    powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") -TestName "release_smoke_ocr" -ReportPath (Join-Path $root "release\smoke-results\ocr-smoke.md") -Ignored
    $code = $LASTEXITCODE
    $textPath = Join-Path $root "demo\output\ocr-output.txt"
    $detected = if (Test-Path -LiteralPath $textPath) { (Get-Content -LiteralPath $textPath -Raw).Trim() } else { "" }
    $normalizedDetected = ($detected -replace '\s+', '').ToUpperInvariant()
    $normalizedExpected = (($item.expected -as [string]) -replace '\s+', '').ToUpperInvariant()
    $passed = $false
    $limitation = ""
    if ($item.category -eq "negative") {
        $passed = ($code -ne 0 -or [string]::IsNullOrWhiteSpace($detected) -or $detected -match '^FAIL:')
        $limitation = "negative fixture; no OCR text expected"
    } elseif ($item.category -eq "unsupported") {
        $passed = $true
        $limitation = "unsupported corpus case recorded; not claimed as supported"
    } else {
        $passed = ($code -eq 0 -and $normalizedDetected.Contains($normalizedExpected))
        $limitation = "supported corpus case"
    }
    if (-not $passed) { $failures++ }
    $status = if ($passed) { "PASS" } else { "FAIL" }
    $rows.Add("| $($item.file) | $($item.category) | $($item.expected) | $detected | $status | $limitation |") | Out-Null
}
Remove-Item Env:\R2H_OCR_SMOKE_INPUT -ErrorAction SilentlyContinue
Remove-Item Env:\R2H_OCR_SMOKE_EXPECTED -ErrorAction SilentlyContinue

$statusOverall = if ($failures -eq 0) { "PASS_WITH_LIMITATION" } else { "FAIL" }
@"
# OCR Corpus Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $statusOverall
- Corpus path: $corpusDir
- Engine: existing local OCR release-smoke path using rendered pixels and local worker
- Supported cases failed: $failures
- Limitation: This corpus is synthetic and distinguishes supported, unsupported, and negative cases. It is not a production OCR accuracy benchmark.

| File | Category | Expected | Detected | Status | Limitation |
| --- | --- | --- | --- | --- | --- |
$($rows -join "`n")
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "OCR corpus smoke: $statusOverall ($reportPath)"
if ($statusOverall -eq "PASS_WITH_LIMITATION") { exit 4 }
exit 1
