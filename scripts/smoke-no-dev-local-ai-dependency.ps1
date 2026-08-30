$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$devLocalAi = Join-Path $root "local-ai"
$disabledLocalAi = Join-Path $root "local-ai.DISABLED-FOR-INSTALLED-SMOKE"
$installRoot = Join-Path $root "release\phase-36.5\install-target"
$installedLocalAi = Join-Path $installRoot "local-ai"
$reportDir = Join-Path $root "release\smoke-results"
$mdPath = Join-Path $reportDir "no-dev-local-ai-dependency.md"
$jsonPath = Join-Path $reportDir "no-dev-local-ai-dependency.json"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null

$renamed = $false
$ocrExit = $null
$ragExit = $null
$llamaVersionExit = $null
$ocrReport = Join-Path $reportDir "ocr-smoke.md"
$ragReport = Join-Path $reportDir "rag-ask-pdf-smoke.md"
$ocrReportText = ""
$ragReportText = ""
$llamaVersionOutput = ""
$status = "FAIL"
$failure = ""

$oldLocalAiHome = $env:R2H_PDF_LOCAL_AI_HOME
$oldStrict = $env:R2H_PDF_STRICT_INSTALLED_AI
$oldDisable = $env:R2H_PDF_DISABLE_DEV_FALLBACK
$oldLegacy = $env:R2H_LOCAL_AI_ROOT

try {
    if (-not (Test-Path -LiteralPath $installedLocalAi -PathType Container)) {
        throw "Installed local-ai path is missing: $installedLocalAi"
    }
    if (Test-Path -LiteralPath $disabledLocalAi) {
        throw "Disabled local-ai path already exists; refusing to overwrite: $disabledLocalAi"
    }
    if (Test-Path -LiteralPath $devLocalAi -PathType Container) {
        Rename-Item -LiteralPath $devLocalAi -NewName "local-ai.DISABLED-FOR-INSTALLED-SMOKE" -ErrorAction Stop
        $renamed = $true
    } else {
        throw "Development local-ai path was not present before the smoke: $devLocalAi"
    }

    $env:R2H_PDF_LOCAL_AI_HOME = $installedLocalAi
    $env:R2H_PDF_STRICT_INSTALLED_AI = "1"
    $env:R2H_PDF_DISABLE_DEV_FALLBACK = "1"
    Remove-Item Env:\R2H_LOCAL_AI_ROOT -ErrorAction SilentlyContinue

    powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "smoke-ocr.ps1")
    $ocrExit = $LASTEXITCODE
    if (Test-Path -LiteralPath $ocrReport -PathType Leaf) {
        $ocrReportText = Get-Content -LiteralPath $ocrReport -Raw
    }

    powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "smoke-rag-ask-pdf.ps1")
    $ragExit = $LASTEXITCODE
    if (Test-Path -LiteralPath $ragReport -PathType Leaf) {
        $ragReportText = Get-Content -LiteralPath $ragReport -Raw
    }

    $llamaCli = Join-Path $installedLocalAi "runtimes\llama-cpp\llama-cli.exe"
    if (Test-Path -LiteralPath $llamaCli -PathType Leaf) {
        $llamaVersionOutput = (& $llamaCli --version 2>&1 | Out-String).Trim()
        $llamaVersionExit = $LASTEXITCODE
    } else {
        $llamaVersionExit = 1
        $llamaVersionOutput = "Missing llama-cli.exe at $llamaCli"
    }

    $devPathPattern = [regex]::Escape($devLocalAi)
    $disabledPathPattern = [regex]::Escape($disabledLocalAi)
    $ocrUsedInstalled = $ocrReportText -match [regex]::Escape($installedLocalAi)
    $ocrUsedDev = $ocrReportText -match $devPathPattern -or $ocrReportText -match $disabledPathPattern
    $ragOutput = Join-Path $root "demo\output\ask-pdf-evidence-report.md"
    $ragOutputOk = (Test-Path -LiteralPath $ragOutput -PathType Leaf) -and ((Get-Item -LiteralPath $ragOutput).Length -gt 0)

    if ($ocrExit -eq 0 -and $ragExit -eq 0 -and $llamaVersionExit -eq 0 -and $ocrUsedInstalled -and -not $ocrUsedDev -and $ragOutputOk) {
        $status = "PASS"
    } else {
        $failure = "ocrExit=$ocrExit; ragExit=$ragExit; llamaVersionExit=$llamaVersionExit; ocrUsedInstalled=$ocrUsedInstalled; ocrUsedDev=$ocrUsedDev; ragOutputOk=$ragOutputOk"
    }
}
catch {
    $failure = $_.Exception.Message
}
finally {
    if ($renamed -and (Test-Path -LiteralPath $disabledLocalAi -PathType Container)) {
        Rename-Item -LiteralPath $disabledLocalAi -NewName "local-ai" -ErrorAction SilentlyContinue
    }
    if ($null -eq $oldLocalAiHome) { Remove-Item Env:\R2H_PDF_LOCAL_AI_HOME -ErrorAction SilentlyContinue } else { $env:R2H_PDF_LOCAL_AI_HOME = $oldLocalAiHome }
    if ($null -eq $oldStrict) { Remove-Item Env:\R2H_PDF_STRICT_INSTALLED_AI -ErrorAction SilentlyContinue } else { $env:R2H_PDF_STRICT_INSTALLED_AI = $oldStrict }
    if ($null -eq $oldDisable) { Remove-Item Env:\R2H_PDF_DISABLE_DEV_FALLBACK -ErrorAction SilentlyContinue } else { $env:R2H_PDF_DISABLE_DEV_FALLBACK = $oldDisable }
    if ($null -eq $oldLegacy) { Remove-Item Env:\R2H_LOCAL_AI_ROOT -ErrorAction SilentlyContinue } else { $env:R2H_LOCAL_AI_ROOT = $oldLegacy }
}

$result = [pscustomobject]@{
    timestamp = $timestamp
    projectRoot = $root
    installRoot = $installRoot
    devLocalAiPath = $devLocalAi
    devLocalAiDisabledDuringSmoke = $renamed
    installedLocalAiPath = $installedLocalAi
    strictInstalledAi = $true
    ocrExitCode = $ocrExit
    ragExitCode = $ragExit
    llamaVersionExitCode = $llamaVersionExit
    llamaVersionOutput = $llamaVersionOutput
    ocrReport = $ocrReport
    ragReport = $ragReport
    devFallbackUsed = ($ocrReportText -match [regex]::Escape($devLocalAi)) -or ($ocrReportText -match [regex]::Escape($disabledLocalAi))
    failure = $failure
    status = $status
}
$result | ConvertTo-Json -Depth 5 | Out-File -LiteralPath $jsonPath -Encoding utf8

@"
# No Development Local-AI Dependency Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Development local-ai path: $devLocalAi
- Development local-ai disabled during smoke: $renamed
- Installed app/resource path used: $installedLocalAi
- Strict installed AI env: R2H_PDF_STRICT_INSTALLED_AI=1; R2H_PDF_DISABLE_DEV_FALLBACK=1
- OCR exit code: $ocrExit
- RAG/Ask PDF exit code: $ragExit
- llama-cli --version exit code: $llamaVersionExit
- Dev fallback used: $($result.devFallbackUsed)
- Failure: $failure
- OCR report: $ocrReport
- RAG report: $ragReport
- JSON report: $jsonPath

## llama-cli --version

``````
$llamaVersionOutput
``````
"@ | Out-File -LiteralPath $mdPath -Encoding utf8

Write-Host "No development local-ai dependency smoke: $status ($mdPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
