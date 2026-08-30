$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\full-ui-workflow-smoke.md"
$jsonPath = Join-Path $root "release\smoke-results\full-ui-workflow-smoke.json"
$artifactDir = Join-Path $root "release\smoke-results\ui-automation"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath), $artifactDir | Out-Null

$npx = Get-Command npx -ErrorAction SilentlyContinue
$tauriDriver = Get-Command tauri-driver -ErrorAction SilentlyContinue
$playwrightInPackage = $false
try {
    $pkg = Get-Content -LiteralPath (Join-Path $root "package.json") -Raw | ConvertFrom-Json
    $allDeps = @()
    if ($pkg.devDependencies) { $allDeps += $pkg.devDependencies.PSObject.Properties.Name }
    if ($pkg.dependencies) { $allDeps += $pkg.dependencies.PSObject.Properties.Name }
    $playwrightInPackage = $allDeps -contains "@playwright/test" -or $allDeps -contains "playwright"
} catch {}

$installTarget = Join-Path $root "release\phase-36.5\install-target"
$appExe = Join-Path $installTarget "r2h-pdf.exe"
$installedLocalAi = Join-Path $installTarget "local-ai"
$status = "FAIL"
$failureReason = "Full interactive UI automation is not executable yet because no Tauri desktop WebDriver/Playwright harness is installed or configured."
$result = [pscustomobject]@{
    timestamp = $timestamp
    projectRoot = $root
    status = $status
    appTarget = $appExe
    appTargetExists = Test-Path -LiteralPath $appExe -PathType Leaf
    installedLocalAiPath = $installedLocalAi
    installedLocalAiExists = Test-Path -LiteralPath $installedLocalAi -PathType Container
    automationStack = "none"
    npxAvailable = [bool]$npx
    playwrightDependencyInPackageJson = $playwrightInPackage
    tauriDriverAvailable = [bool]$tauriDriver
    backendHookCountedAsUi = $false
    workflowsValidated = @()
    screenshots = @()
    traces = @()
    failureReason = $failureReason
}
$result | ConvertTo-Json -Depth 6 | Out-File -LiteralPath $jsonPath -Encoding utf8

@"
# Full UI Workflow Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- npx available: $([bool]$npx)
- Playwright dependency in package.json: $playwrightInPackage
- tauri-driver available: $([bool]$tauriDriver)
- App target considered: $appExe
- App target exists: $(Test-Path -LiteralPath $appExe -PathType Leaf)
- Installed local-ai path: $installedLocalAi
- Installed local-ai exists: $(Test-Path -LiteralPath $installedLocalAi -PathType Container)
- Required workflows: open PDF, render, navigate, text edit/export, image edit/export, OCR, Ask PDF/RAG, compare, export, reopen exported output
- Result: $failureReason
- Backend hook proof: available as supporting evidence only and intentionally not counted as full UI automation.
- Required fix: add a Tauri desktop automation harness, stable selectors, and workflow tests that drive the installed app UI and validate exported outputs.
- JSON report: $jsonPath
- Artifact directory: $artifactDir
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Full UI workflow smoke: FAIL ($reportPath)"
exit 1
