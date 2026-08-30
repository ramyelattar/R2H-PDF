$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\clean-install-smoke.md"
$installTarget = Join-Path $root "release\phase-36.5\install-target"
$installer = Join-Path $root "src-tauri\target\release\bundle\nsis\r2h-pdf_2.1.0_x64-setup.exe"
$modelPackSource = Join-Path $root "release\v2.1.0-beta\full-offline-inno-staging\local-ai"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath), (Split-Path $installTarget) | Out-Null

if (Test-Path -LiteralPath $installTarget) {
    Remove-Item -LiteralPath $installTarget -Recurse -Force -ErrorAction SilentlyContinue
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$proc = $null
if (Test-Path -LiteralPath $installer -PathType Leaf) {
    $proc = Start-Process -FilePath $installer -ArgumentList @("/S", "/D=$installTarget") -Wait -PassThru
}
$modelPackExit = $null
$installedLocalAi = Join-Path $installTarget "local-ai"
if ($proc -and $proc.ExitCode -eq 0 -and (Test-Path -LiteralPath $modelPackSource -PathType Container)) {
    if (Test-Path -LiteralPath $installedLocalAi) {
        Remove-Item -LiteralPath $installedLocalAi -Recurse -Force -ErrorAction SilentlyContinue
    }
    New-Item -ItemType Directory -Force -Path $installedLocalAi | Out-Null
    $modelPackOutput = robocopy $modelPackSource $installedLocalAi /MIR /R:1 /W:1 /XD .git __pycache__ 2>&1
    $modelPackExit = $LASTEXITCODE
    if ($modelPackExit -le 7) { $LASTEXITCODE = 0 }
} else {
    $modelPackOutput = @("Model pack source missing or app installer failed: $modelPackSource")
}
$sw.Stop()

$installedExe = Join-Path $installTarget "r2h-pdf.exe"
$uninstaller = Join-Path $installTarget "uninstall.exe"
$requiredInstalledAi = @(
    "local-ai\config\models.json",
    "local-ai\models\llm\Qwen3-4B-GGUF\Qwen3-4B-Q4_K_M.gguf",
    "local-ai\models\embeddings\Qwen3-Embedding-4B-GGUF\Qwen3-Embedding-4B-Q4_K_M.gguf",
    "local-ai\models\ocr\PaddleOCR-VL\model.safetensors",
    "local-ai\models\ocr\PaddleOCR-VL\config.json",
    "local-ai\runtimes\llama-cpp\llama-cli.exe",
    "local-ai\runtimes\llama-cpp\llama-server.exe",
    "local-ai\workers\paddleocr_vl_worker.py"
)
$missingInstalledAi = @($requiredInstalledAi | Where-Object { -not (Test-Path -LiteralPath (Join-Path $installTarget $_) -PathType Leaf) })
$installedSize = (Get-ChildItem -LiteralPath $installTarget -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
$status = if ($proc -and $proc.ExitCode -eq 0 -and (Test-Path -LiteralPath $installedExe -PathType Leaf) -and (Test-Path -LiteralPath $uninstaller -PathType Leaf) -and $missingInstalledAi.Count -eq 0 -and ($modelPackExit -eq $null -or $modelPackExit -le 7)) { "PASS" } else { "FAIL" }

@"
# Clean Install Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Installer path: $installer
- Installer type: NSIS setup EXE
- Offline model pack source: $modelPackSource
- Install target: $installTarget
- Installer exit code: $($proc.ExitCode)
- Model pack copy exit code: $modelPackExit
- Duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s
- Installed executable: $installedExe
- Installed executable exists: $(Test-Path -LiteralPath $installedExe -PathType Leaf)
- Uninstaller exists: $(Test-Path -LiteralPath $uninstaller -PathType Leaf)
- Installed local-ai path: $installedLocalAi
- Installed target total size bytes: $installedSize
- Missing installed local AI files: $($missingInstalledAi -join "; ")
- Packaging model: app installer plus local offline model pack copied beside the installed app for deterministic release validation.
- Limitation: This proves a deterministic clean install target on the current machine, not a separate pristine VM image.

## Model Pack Copy Output

``````
$($modelPackOutput -join "`n")
``````
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Clean install smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
