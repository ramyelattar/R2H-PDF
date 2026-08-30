# R2H PDF - Release Candidate Validation Matrix
# Runs implemented release gates and records PASS / PASS_WITH_LIMITATION /
# BLOCKED_BY_MISSING_CERTIFICATE / FAIL / NOT_IMPLEMENTED /
# MANUAL_CHECKLIST_ONLY / SKIPPED_WITH_REASON.

$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "release"
$logsDir = Join-Path $releaseDir "logs"
$matrixPath = Join-Path $releaseDir "R2H-PDF-VALIDATION-MATRIX.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
$runId = Get-Date -Format "yyyyMMdd-HHmmss"

New-Item -ItemType Directory -Force -Path $releaseDir, $logsDir | Out-Null

$results = New-Object System.Collections.Generic.List[object]
$implementedBlockingFailure = $false

function Test-PackageScript {
    param([string]$Name)
    $pkgPath = Join-Path $root "package.json"
    if (-not (Test-Path -LiteralPath $pkgPath -PathType Leaf)) { return $false }
    try {
        $pkg = Get-Content -LiteralPath $pkgPath -Raw | ConvertFrom-Json
        return [bool]($pkg.scripts.PSObject.Properties.Name -contains $Name)
    } catch {
        return $false
    }
}

function ConvertTo-SafeFileName {
    param([string]$Value)
    return (($Value -replace '[^A-Za-z0-9_.-]+', '-') -replace '^-|-$', '').ToLowerInvariant()
}

function Escape-MarkdownCell {
    param([string]$Value)
    if ($null -eq $Value) { return "" }
    return (($Value -replace '\|', '\|') -replace "`r?`n", "<br>")
}

function Format-GateDuration {
    param([System.TimeSpan]$Elapsed)
    $seconds = [math]::Round($Elapsed.TotalSeconds, 2)
    if ($seconds -eq 0 -and $Elapsed.TotalMilliseconds -gt 0) {
        $seconds = 0.01
    }
    return "$($seconds)s"
}

function Add-Result {
    param(
        [string]$Gate,
        [string]$Command,
        [string]$Status,
        [string]$Duration,
        [string]$EvidencePath,
        [string]$Notes,
        [bool]$Blocking = $true
    )

    $results.Add([pscustomobject]@{
        Gate = $Gate
        Command = $Command
        Status = $Status
        Duration = $Duration
        EvidencePath = $EvidencePath
        Notes = $Notes
        Blocking = $Blocking
    }) | Out-Null

    $color = switch ($Status) {
        "PASS" { "Green" }
        "PASS_WITH_LIMITATION" { "Yellow" }
        "FAIL" { "Red" }
        "BLOCKED_BY_MISSING_CERTIFICATE" { "Magenta" }
        "MANUAL_CHECKLIST_ONLY" { "Yellow" }
        "SKIPPED_WITH_REASON" { "Yellow" }
        default { "DarkYellow" }
    }
    Write-Host ("[{0}] {1} ({2})" -f $Status, $Gate, $Duration) -ForegroundColor $color
    if ($Notes) { Write-Host "  $Notes" }
}

function Invoke-Gate {
    param(
        [string]$Gate,
        [string]$Command,
        [bool]$Implemented,
        [bool]$Blocking = $true,
        [bool]$RequiresRelease = $false,
        [string]$NotImplementedReason = "No matching script or command was found.",
        [string]$SkipReason = ""
    )

    if (-not $Implemented) {
        Add-Result -Gate $Gate -Command $Command -Status "NOT_IMPLEMENTED" -Duration "0.0s" -EvidencePath "" -Notes $NotImplementedReason -Blocking $Blocking
        return
    }

    if ($RequiresRelease -and -not (Test-Path -LiteralPath (Join-Path $releaseDir "v2.1.0-beta") -PathType Container)) {
        Add-Result -Gate $Gate -Command $Command -Status "SKIPPED_WITH_REASON" -Duration "0.0s" -EvidencePath "" -Notes $SkipReason -Blocking $Blocking
        return
    }

    $safeName = ConvertTo-SafeFileName $Gate
    $logPath = Join-Path $logsDir "$runId-$safeName.log"
    $stdoutPath = Join-Path $logsDir "$runId-$safeName.stdout.tmp"
    $stderrPath = Join-Path $logsDir "$runId-$safeName.stderr.tmp"
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $escapedRoot = $root -replace "'", "''"
    $scriptText = "`$ProgressPreference = 'SilentlyContinue'; Set-Location -LiteralPath '$escapedRoot'; $Command; `$gateExit = if (`$null -ne `$LASTEXITCODE) { `$LASTEXITCODE } else { 0 }; exit `$gateExit"
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($scriptText))

    Write-Host "--- $Gate ---" -ForegroundColor Cyan
    Write-Host "Command: $Command"

    $proc = Start-Process -FilePath "powershell" `
        -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-EncodedCommand", $encoded) `
        -Wait `
        -PassThru `
        -NoNewWindow `
        -RedirectStandardOutput $stdoutPath `
        -RedirectStandardError $stderrPath
    $exitCode = $proc.ExitCode
    $sw.Stop()

    $commandOutput = New-Object System.Collections.Generic.List[string]
    if (Test-Path -LiteralPath $stdoutPath) {
        foreach ($line in (Get-Content -LiteralPath $stdoutPath -ErrorAction SilentlyContinue)) {
            $commandOutput.Add($line) | Out-Null
        }
    }
    if (Test-Path -LiteralPath $stderrPath) {
        $stderrLines = @(Get-Content -LiteralPath $stderrPath -ErrorAction SilentlyContinue)
        if ($stderrLines.Count -gt 0) {
            $commandOutput.Add("=== STDERR ===") | Out-Null
            foreach ($line in $stderrLines) {
                $commandOutput.Add($line) | Out-Null
            }
        }
    }

    $header = @(
        "Timestamp: $timestamp",
        "Project root: $root",
        "Gate: $Gate",
        "Command: $Command",
        "Exit code: $exitCode",
        "Duration: $(Format-GateDuration $sw.Elapsed)",
        ""
    )
    @($header + "=== COMMAND OUTPUT ===" + $commandOutput) | Out-File -LiteralPath $logPath -Encoding utf8
    Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue

    foreach ($line in $commandOutput) {
        Write-Host $line
    }

    if ($exitCode -eq 0 -or $null -eq $exitCode) {
        Add-Result -Gate $Gate -Command $Command -Status "PASS" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Exit code 0." -Blocking $Blocking
    } elseif ($exitCode -eq 2) {
        Add-Result -Gate $Gate -Command $Command -Status "NOT_IMPLEMENTED" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Script returned exit code 2 for an honest NOT_IMPLEMENTED gate." -Blocking $Blocking
    } elseif ($exitCode -eq 3) {
        Add-Result -Gate $Gate -Command $Command -Status "SKIPPED_WITH_REASON" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Script returned exit code 3 for a skipped gate with reason." -Blocking $Blocking
    } elseif ($exitCode -eq 4) {
        Add-Result -Gate $Gate -Command $Command -Status "PASS_WITH_LIMITATION" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Script returned exit code 4 for verified output with documented limitation." -Blocking $Blocking
    } elseif ($exitCode -eq 5) {
        Add-Result -Gate $Gate -Command $Command -Status "MANUAL_CHECKLIST_ONLY" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Script returned exit code 5 for manual checklist only; no automated packaged workflow PASS claimed." -Blocking $Blocking
    } elseif ($exitCode -eq 6) {
        Add-Result -Gate $Gate -Command $Command -Status "BLOCKED_BY_MISSING_CERTIFICATE" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Script returned exit code 6 because no signing certificate is configured." -Blocking $Blocking
    } else {
        Add-Result -Gate $Gate -Command $Command -Status "FAIL" -Duration "$(Format-GateDuration $sw.Elapsed)" -EvidencePath $logPath -Notes "Exit code $exitCode." -Blocking $Blocking
        if ($Blocking) { $script:implementedBlockingFailure = $true }
    }
}

function Find-SmokeScript {
    param([string[]]$Keywords)
    if (-not (Test-Path -LiteralPath (Join-Path $root "scripts") -PathType Container)) { return $null }
    $files = Get-ChildItem -LiteralPath (Join-Path $root "scripts") -File -Include *.ps1,*.js,*.mjs,*.ts -ErrorAction SilentlyContinue
    foreach ($file in $files) {
        $name = $file.Name.ToLowerInvariant()
        $matchesAll = $true
        foreach ($keyword in $Keywords) {
            if (-not $name.Contains($keyword.ToLowerInvariant())) {
                $matchesAll = $false
                break
            }
        }
        if ($matchesAll) { return $file.FullName }
    }
    return $null
}

Write-Host "=== R2H PDF - Release Candidate Validation ===" -ForegroundColor Cyan
Write-Host "Timestamp: $timestamp"
Write-Host "Project root: $root"
Write-Host "Report: $matrixPath"
Write-Host ""

$releaseExists = Test-Path -LiteralPath (Join-Path $releaseDir "v2.1.0-beta") -PathType Container

Invoke-Gate "TypeScript typecheck" "pnpm typecheck" (Test-PackageScript "typecheck") $true $false
Invoke-Gate "ESLint" "pnpm lint" (Test-PackageScript "lint") $true $false
Invoke-Gate "Frontend tests" "pnpm test -- --fileParallelism=false" (Test-PackageScript "test") $true $false
Invoke-Gate "IPC contract tests" "powershell -ExecutionPolicy Bypass -File scripts\validate-ipc-contract.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\validate-ipc-contract.ps1")) $true $false
Invoke-Gate "Cargo check" "Push-Location src-tauri; cargo check -q; Pop-Location" (Test-Path -LiteralPath (Join-Path $root "src-tauri\Cargo.toml")) $true $false
Invoke-Gate "Cargo test" "Push-Location src-tauri; cargo test --lib -q; Pop-Location" (Test-Path -LiteralPath (Join-Path $root "src-tauri\Cargo.toml")) $true $false
Invoke-Gate "Python compile" "py -m compileall local-ai\workers" (Test-Path -LiteralPath (Join-Path $root "local-ai\workers")) $true $false
Invoke-Gate "Local AI validation" "powershell -ExecutionPolicy Bypass -File scripts\validate-local-ai.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\validate-local-ai.ps1")) $true $false
Invoke-Gate "Offline safety validation" "powershell -ExecutionPolicy Bypass -File scripts\validate-offline-safety.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\validate-offline-safety.ps1")) $true $false
Invoke-Gate "Version check" "powershell -ExecutionPolicy Bypass -File scripts\check-version.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\check-version.ps1")) $true $false
Invoke-Gate "Existing prepackage validation" "powershell -ExecutionPolicy Bypass -File scripts\validate-prepackage.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\validate-prepackage.ps1")) $true $false
Invoke-Gate "Existing freshness verification" "powershell -ExecutionPolicy Bypass -File scripts\verify-built-app-freshness.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\verify-built-app-freshness.ps1")) $true $true "Freshness script exists, but no release\v2.1.0-beta folder exists." "Requires release\v2.1.0-beta packaged output."

$smokeDefinitions = @(
    @{ Gate = "OCR smoke"; Script = "scripts\smoke-ocr.ps1" },
    @{ Gate = "RAG/Ask PDF smoke"; Script = "scripts\smoke-rag-ask-pdf.ps1" },
    @{ Gate = "PDF render smoke"; Script = "scripts\smoke-pdf-render.ps1" },
    @{ Gate = "Text edit/export smoke"; Script = "scripts\smoke-text-edit-export.ps1" },
    @{ Gate = "Image edit/export smoke"; Script = "scripts\smoke-image-edit-export.ps1" },
    @{ Gate = "Compare smoke"; Script = "scripts\smoke-compare.ps1" },
    @{ Gate = "Packaged UI launch smoke"; Script = "scripts\smoke-packaged-ui-launch.ps1" },
    @{ Gate = "Packaged render smoke"; Script = "scripts\smoke-packaged-render.ps1" },
    @{ Gate = "Packaged text edit/export smoke"; Script = "scripts\smoke-packaged-text-edit-export.ps1" },
    @{ Gate = "Packaged image edit/export smoke"; Script = "scripts\smoke-packaged-image-edit-export.ps1" },
    @{ Gate = "Packaged OCR smoke"; Script = "scripts\smoke-packaged-ocr.ps1" },
    @{ Gate = "Packaged RAG/Ask PDF smoke"; Script = "scripts\smoke-packaged-rag-ask-pdf.ps1" },
    @{ Gate = "Packaged compare smoke"; Script = "scripts\smoke-packaged-compare.ps1" }
)

foreach ($smoke in $smokeDefinitions) {
    $scriptPath = Join-Path $root $smoke.Script
    $cmd = "powershell -ExecutionPolicy Bypass -File $($smoke.Script)"
    Invoke-Gate $smoke.Gate $cmd (Test-Path -LiteralPath $scriptPath -PathType Leaf) $true $false "Dedicated smoke script is missing: $($smoke.Script)"
}

Invoke-Gate "Security source scan" "powershell -ExecutionPolicy Bypass -File scripts\security-source-scan.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\security-source-scan.ps1")) $true $false
Invoke-Gate "Security distribution scan" "powershell -ExecutionPolicy Bypass -File scripts\security-distribution-scan.ps1" (Test-Path -LiteralPath (Join-Path $root "scripts\security-distribution-scan.ps1")) $true $false

$phase365Definitions = @(
    @{ Gate = "Final installer proof"; Script = "scripts\build-final-installer.ps1" },
    @{ Gate = "Clean install smoke"; Script = "scripts\smoke-clean-install.ps1" },
    @{ Gate = "Installed app launch smoke"; Script = "scripts\smoke-installed-app-launch.ps1" },
    @{ Gate = "Installed local AI inventory"; Script = "scripts\smoke-installed-local-ai-inventory.ps1" },
    @{ Gate = "No development local-ai dependency"; Script = "scripts\smoke-no-dev-local-ai-dependency.ps1" },
    @{ Gate = "Full UI workflow automation"; Script = "scripts\smoke-full-ui-workflows.ps1" },
    @{ Gate = "UI automation artifact validation"; Script = "scripts\validate-ui-automation-artifacts.ps1" },
    @{ Gate = "OCR corpus smoke"; Script = "scripts\smoke-ocr-corpus.ps1" },
    @{ Gate = "Searchable OCR PDF smoke"; Script = "scripts\smoke-searchable-ocr-pdf.ps1" },
    @{ Gate = "Offline license smoke"; Script = "scripts\smoke-offline-license.ps1" },
    @{ Gate = "Trial expiry smoke"; Script = "scripts\smoke-trial-expiry.ps1" },
    @{ Gate = "License tamper rejection smoke"; Script = "scripts\smoke-license-tamper.ps1" },
    @{ Gate = "EULA smoke"; Script = "scripts\smoke-eula.ps1" },
    @{ Gate = "Signature verification"; Script = "scripts\verify-signature.ps1" },
    @{ Gate = "Uninstall smoke"; Script = "scripts\smoke-uninstall.ps1" }
)

foreach ($gate in $phase365Definitions) {
    $scriptPath = Join-Path $root $gate.Script
    $cmd = "powershell -ExecutionPolicy Bypass -File $($gate.Script)"
    Invoke-Gate $gate.Gate $cmd (Test-Path -LiteralPath $scriptPath -PathType Leaf) $true $false "Dedicated Phase 36.5 script is missing: $($gate.Script)"
}

$passCount = @($results | Where-Object Status -eq "PASS").Count
$passWithLimitationCount = @($results | Where-Object Status -eq "PASS_WITH_LIMITATION").Count
$failCount = @($results | Where-Object Status -eq "FAIL").Count
$blockedByCertificateCount = @($results | Where-Object Status -eq "BLOCKED_BY_MISSING_CERTIFICATE").Count
$notImplementedCount = @($results | Where-Object Status -eq "NOT_IMPLEMENTED").Count
$manualChecklistCount = @($results | Where-Object Status -eq "MANUAL_CHECKLIST_ONLY").Count
$skippedCount = @($results | Where-Object Status -eq "SKIPPED_WITH_REASON").Count
$overall = if ($implementedBlockingFailure -or $failCount -gt 0 -or $notImplementedCount -gt 0 -or $manualChecklistCount -gt 0) {
    "FAIL"
} elseif ($blockedByCertificateCount -gt 0) {
    "BLOCKED_BY_MISSING_CERTIFICATE"
} elseif ($passWithLimitationCount -gt 0 -or $skippedCount -gt 0) {
    "PASS_WITH_GAPS"
} else {
    "PASS"
}

$md = New-Object System.Collections.Generic.List[string]
$md.Add("# R2H-PDF Validation Matrix") | Out-Null
$md.Add("") | Out-Null
$md.Add("- Timestamp: $timestamp") | Out-Null
$md.Add("- Project root: $root") | Out-Null
$md.Add("- Release output present: $releaseExists") | Out-Null
$md.Add("- Overall implemented blocking result: $overall") | Out-Null
$md.Add("- Summary: PASS=$passCount; PASS_WITH_LIMITATION=$passWithLimitationCount; BLOCKED_BY_MISSING_CERTIFICATE=$blockedByCertificateCount; FAIL=$failCount; NOT_IMPLEMENTED=$notImplementedCount; MANUAL_CHECKLIST_ONLY=$manualChecklistCount; SKIPPED_WITH_REASON=$skippedCount") | Out-Null
$md.Add("") | Out-Null
$md.Add("| Gate | Command | Status | Duration | Evidence path/log | Notes |") | Out-Null
$md.Add("| --- | --- | --- | --- | --- | --- |") | Out-Null
foreach ($r in $results) {
    $gateCell = Escape-MarkdownCell $r.Gate
    $commandCell = Escape-MarkdownCell $r.Command
    $evidenceCell = Escape-MarkdownCell $r.EvidencePath
    $notesCell = Escape-MarkdownCell $r.Notes
    $md.Add("| $gateCell | ``$commandCell`` | $($r.Status) | $($r.Duration) | $evidenceCell | $notesCell |") | Out-Null
}

$md.Add("") | Out-Null
$md.Add("## Missing Gates / Gaps") | Out-Null
$missing = @($results | Where-Object { $_.Status -eq "NOT_IMPLEMENTED" -or $_.Status -eq "MANUAL_CHECKLIST_ONLY" -or $_.Status -eq "SKIPPED_WITH_REASON" -or $_.Status -eq "PASS_WITH_LIMITATION" -or $_.Status -eq "BLOCKED_BY_MISSING_CERTIFICATE" -or $_.Status -eq "FAIL" })
if ($missing.Count -eq 0) {
    $md.Add("- None recorded.") | Out-Null
} else {
    foreach ($r in $missing) {
        $md.Add("- $($r.Gate): $($r.Status) - $($r.Notes)") | Out-Null
    }
}

$md | Out-File -LiteralPath $matrixPath -Encoding utf8

Write-Host ""
Write-Host "Validation matrix written: $matrixPath" -ForegroundColor Cyan
Write-Host "Summary: PASS=$passCount PASS_WITH_LIMITATION=$passWithLimitationCount BLOCKED_BY_MISSING_CERTIFICATE=$blockedByCertificateCount FAIL=$failCount NOT_IMPLEMENTED=$notImplementedCount MANUAL_CHECKLIST_ONLY=$manualChecklistCount SKIPPED_WITH_REASON=$skippedCount"

if ($overall -eq "FAIL") {
    Write-Host "RELEASE CANDIDATE STATUS: FAIL" -ForegroundColor Red
    exit 1
}

if ($overall -eq "BLOCKED_BY_MISSING_CERTIFICATE") {
    Write-Host "RELEASE CANDIDATE STATUS: BLOCKED_BY_MISSING_CERTIFICATE" -ForegroundColor Magenta
    exit 6
}

Write-Host "RELEASE CANDIDATE STATUS: $overall" -ForegroundColor Green
exit 0
