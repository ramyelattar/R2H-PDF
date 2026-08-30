param(
    [string]$Workflow = "",
    [string]$ReportName = "",
    [string]$OutputName = "",
    [string]$ExpectedProof = ""
)

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$smokeDir = Join-Path $root "release\smoke-results"
$outputDir = Join-Path $root "demo\output"
$reportPath = Join-Path $smokeDir $ReportName
$outputPath = Join-Path $outputDir $OutputName
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $smokeDir, $outputDir | Out-Null

if (-not $Workflow) {
    $aggregatePath = Join-Path $root "release\packaged-workflow-smoke-report.md"
    $commands = @(
        @{ Name = "packaged render"; Script = "smoke-packaged-render.ps1"; Report = "packaged-render-smoke.md" },
        @{ Name = "packaged text edit/export"; Script = "smoke-packaged-text-edit-export.ps1"; Report = "packaged-text-edit-export-smoke.md" },
        @{ Name = "packaged image edit/export"; Script = "smoke-packaged-image-edit-export.ps1"; Report = "packaged-image-edit-export-smoke.md" },
        @{ Name = "packaged OCR"; Script = "smoke-packaged-ocr.ps1"; Report = "packaged-ocr-smoke.md" },
        @{ Name = "packaged Ask PDF/RAG"; Script = "smoke-packaged-rag-ask-pdf.ps1"; Report = "packaged-rag-ask-pdf-smoke.md" },
        @{ Name = "packaged compare"; Script = "smoke-packaged-compare.ps1"; Report = "packaged-compare-smoke.md" }
    )
    $rows = New-Object System.Collections.Generic.List[string]
    $failed = $false
    foreach ($cmd in $commands) {
        powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot $cmd.Script)
        $code = $LASTEXITCODE
        $status = if ($code -eq 0) { "PASS" } elseif ($code -eq 4) { "PASS_WITH_LIMITATION" } else { "FAIL" }
        if ($status -eq "FAIL") { $failed = $true }
        $report = Join-Path $smokeDir $cmd.Report
        $rows.Add("| $($cmd.Name) | $status | $report | exit=$code |") | Out-Null
    }
@"
# Packaged Workflow Smoke Report

- Timestamp: $timestamp
- Project root: $root
- Status: $(if ($failed) { "FAIL" } else { "PASS_WITH_LIMITATION" })
- Limitation: Packaged workflows are proven through the built executable release-smoke backend hook, not full interactive UI automation.

| Workflow | Status | Evidence | Notes |
| --- | --- | --- | --- |
$($rows -join "`n")
"@ | Out-File -LiteralPath $aggregatePath -Encoding utf8
    Write-Host "Packaged workflow aggregate written: $aggregatePath"
    if ($failed) { exit 1 } else { exit 0 }
}

$releaseRoot = Join-Path $root "release\v2.1.0-beta"
$directExe = Join-Path $root "src-tauri\target\release\r2h-pdf.exe"
$installer = Get-ChildItem -LiteralPath $releaseRoot -Recurse -File -Filter "*.exe" -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -match 'setup|installer|install' } |
    Select-Object -First 1
$appExe = Get-ChildItem -LiteralPath $releaseRoot -Recurse -File -Filter "*.exe" -ErrorAction SilentlyContinue |
    Where-Object {
        $_.FullName -match 'r2h|R2H' -and
        $_.Name -notmatch 'setup|installer|install' -and
        $_.FullName -notmatch 'python-runtime|llama-cpp|runtimes'
    } |
    Select-Object -First 1
if ((-not $appExe) -and (Test-Path -LiteralPath $directExe -PathType Leaf)) {
    $appExe = Get-Item -LiteralPath $directExe
}

if (-not $appExe) {
@"
# Packaged $Workflow Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: FAIL
- Automation status: FAIL
- Required proof: $ExpectedProof
- Intended output path: $outputPath
- Release root exists: $(Test-Path -LiteralPath $releaseRoot -PathType Container)
- Installer artifact found: $($installer.FullName)
- Direct app executable found: false
- Reason: No direct packaged app executable was found. Setup installers are not accepted as workflow proof.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8
    Write-Host "Packaged $Workflow smoke FAIL: no direct executable"
    exit 1
}

$workflowArg = switch -Wildcard ($Workflow) {
    "render" { "render"; break }
    "text*" { "text-edit-export"; break }
    "image*" { "image-edit-export"; break }
    "OCR" { "ocr"; break }
    "Ask*" { "rag-ask-pdf"; break }
    "compare" { "compare"; break }
    default { $Workflow }
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = Start-Process -FilePath $appExe.FullName `
    -ArgumentList @("--release-smoke", $workflowArg) `
    -Wait `
    -PassThru `
    -WindowStyle Hidden
$sw.Stop()
$exitCode = $proc.ExitCode
$sourceOutput = switch ($workflowArg) {
    "render" { Join-Path $outputDir "render-smoke.ppm"; break }
    "text-edit-export" { Join-Path $outputDir "text-edit-export.pdf"; break }
    "image-edit-export" { Join-Path $outputDir "image-edit-export.pdf"; break }
    "ocr" { Join-Path $outputDir "ocr-output.txt"; break }
    "rag-ask-pdf" { Join-Path $outputDir "ask-pdf-evidence-report.md"; break }
    "compare" { Join-Path $outputDir "compare-report.md"; break }
    default { "" }
}
if ($exitCode -eq 0 -and $sourceOutput -and (Test-Path -LiteralPath $sourceOutput -PathType Leaf)) {
    Copy-Item -LiteralPath $sourceOutput -Destination $outputPath -Force
}
$outputExists = Test-Path -LiteralPath $outputPath -PathType Leaf
$outputBytes = if ($outputExists) { (Get-Item -LiteralPath $outputPath).Length } else { 0 }
$status = if ($exitCode -eq 0 -and $outputExists -and $outputBytes -gt 0) { "PASS_WITH_LIMITATION" } else { "FAIL" }
$validation = if ($status -eq "PASS_WITH_LIMITATION") { "Packaged executable release-smoke hook exited 0 and non-empty copied packaged output exists." } else { "Packaged executable release-smoke hook exit=$exitCode outputExists=$outputExists outputBytes=$outputBytes." }

@"
# Packaged $Workflow Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Automation status: packaged backend release-smoke hook
- Required proof: $ExpectedProof
- Packaged executable used: $($appExe.FullName)
- Executable classification: app executable
- Entrypoint: $($appExe.FullName) --release-smoke $workflowArg
- Process id: $($proc.Id)
- Exit code: $exitCode
- Duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s
- Intended output path: $outputPath
- Source output copied from: $sourceOutput
- Output exists: $outputExists
- Output bytes: $outputBytes
- Validation: $validation
- Release root exists: $(Test-Path -LiteralPath $releaseRoot -PathType Container)
- Installer artifact found: $($installer.FullName)
- Direct app executable found: $($appExe.FullName)
- Limitation: This is packaged backend proof through the built executable release-smoke hook, not full interactive UI automation.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Packaged $Workflow smoke ${status}: $reportPath"
if ($status -eq "PASS_WITH_LIMITATION") { exit 4 } else { exit 1 }
