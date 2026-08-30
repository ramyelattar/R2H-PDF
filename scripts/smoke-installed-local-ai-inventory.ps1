$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$installRoot = Join-Path $root "release\phase-36.5\install-target"
$reportDir = Join-Path $root "release\smoke-results"
$mdPath = Join-Path $reportDir "installed-local-ai-inventory.md"
$jsonPath = Join-Path $reportDir "installed-local-ai-inventory.json"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $reportDir | Out-Null

$candidateRoots = @(
    (Join-Path $installRoot "local-ai"),
    (Join-Path $installRoot "resources\local-ai"),
    (Join-Path (Split-Path $installRoot -Parent) "R2H-PDF-LocalAI\local-ai")
)

$localAiPath = $candidateRoots | Where-Object { Test-Path -LiteralPath $_ -PathType Container } | Select-Object -First 1
if (-not $localAiPath) {
    $localAiPath = Join-Path $installRoot "local-ai"
}

$required = @(
    @{ kind = "manifest"; label = "models.json"; rel = "config\models.json"; required = $true },
    @{ kind = "runtime"; label = "llama-cli.exe"; rel = "runtimes\llama-cpp\llama-cli.exe"; required = $true },
    @{ kind = "runtime"; label = "llama-server.exe"; rel = "runtimes\llama-cpp\llama-server.exe"; required = $true },
    @{ kind = "runtime"; label = "llama.dll"; rel = "runtimes\llama-cpp\llama.dll"; required = $true },
    @{ kind = "runtime"; label = "ggml.dll"; rel = "runtimes\llama-cpp\ggml.dll"; required = $true },
    @{ kind = "model"; label = "LLM Qwen3 4B"; rel = "models\llm\Qwen3-4B-GGUF\Qwen3-4B-Q4_K_M.gguf"; required = $true },
    @{ kind = "model"; label = "Embedding Qwen3"; rel = "models\embeddings\Qwen3-Embedding-4B-GGUF\Qwen3-Embedding-4B-Q4_K_M.gguf"; required = $true },
    @{ kind = "model"; label = "OCR PaddleOCR-VL weights"; rel = "models\ocr\PaddleOCR-VL\model.safetensors"; required = $true },
    @{ kind = "model"; label = "OCR PaddleOCR-VL config"; rel = "models\ocr\PaddleOCR-VL\config.json"; required = $true },
    @{ kind = "model"; label = "Reranker Qwen3"; rel = "models\rerankers\Qwen3-Reranker-0.6B-GGUF\qwen3-reranker-0.6b-q8_0.gguf"; required = $false },
    @{ kind = "worker"; label = "PaddleOCR worker"; rel = "workers\paddleocr_vl_worker.py"; required = $true }
)

$installedFiles = @(Get-ChildItem -LiteralPath $installRoot -Recurse -File -ErrorAction SilentlyContinue)
$localAiFiles = @(Get-ChildItem -LiteralPath $localAiPath -Recurse -File -ErrorAction SilentlyContinue)
$totalInstalledSize = ($installedFiles | Measure-Object -Property Length -Sum).Sum
$localAiSize = ($localAiFiles | Measure-Object -Property Length -Sum).Sum

$assets = foreach ($asset in $required) {
    $path = Join-Path $localAiPath $asset.rel
    $item = Get-Item -LiteralPath $path -ErrorAction SilentlyContinue
    [pscustomobject]@{
        kind = $asset.kind
        label = $asset.label
        relativePath = $asset.rel
        path = $path
        required = [bool]$asset.required
        exists = [bool]$item
        sizeBytes = if ($item) { [int64]$item.Length } else { 0 }
    }
}

$runtimeFiles = @($localAiFiles | Where-Object { $_.FullName -match "\\runtimes\\" } | Select-Object -ExpandProperty FullName)
$modelFiles = @($localAiFiles | Where-Object { $_.FullName -match "\\models\\" -and $_.Extension -in @(".gguf", ".safetensors", ".json", ".pdmodel", ".pdiparams", ".model") } | Select-Object -ExpandProperty FullName)
$largestFiles = @($localAiFiles | Sort-Object Length -Descending | Select-Object -First 20 | ForEach-Object {
    [pscustomobject]@{ path = $_.FullName; sizeBytes = [int64]$_.Length }
})
$missingRequired = @($assets | Where-Object { $_.required -and -not $_.exists })
$devPath = Join-Path $root "local-ai"
$usesDevPath = $false
if ((Test-Path -LiteralPath $localAiPath) -and (Test-Path -LiteralPath $devPath)) {
    try {
        $usesDevPath = ((Resolve-Path -LiteralPath $localAiPath).Path -eq (Resolve-Path -LiteralPath $devPath).Path)
    } catch {}
}

$status = if ((Test-Path -LiteralPath $localAiPath -PathType Container) -and $missingRequired.Count -eq 0 -and -not $usesDevPath -and $localAiSize -gt 5GB) { "PASS" } else { "FAIL" }

$result = [pscustomobject]@{
    timestamp = $timestamp
    projectRoot = $root
    installRoot = $installRoot
    totalInstalledSizeBytes = [int64]$totalInstalledSize
    localAiPath = $localAiPath
    localAiSizeBytes = [int64]$localAiSize
    manifestPath = Join-Path $localAiPath "config\models.json"
    runtimeFiles = $runtimeFiles
    modelFiles = $modelFiles
    largestFiles = $largestFiles
    requiredAssets = $assets
    missingRequiredResources = @($missingRequired | Select-Object -ExpandProperty relativePath)
    usesDevelopmentLocalAi = $usesDevPath
    status = $status
}

$result | ConvertTo-Json -Depth 8 | Out-File -LiteralPath $jsonPath -Encoding utf8

$assetRows = $assets | ForEach-Object {
    "| $($_.kind) | $($_.label) | $($_.required) | $($_.exists) | $($_.sizeBytes) | $($_.path) |"
}
$largestRows = $largestFiles | ForEach-Object {
    "| $($_.sizeBytes) | $($_.path) |"
}

@"
# Installed Local AI Inventory

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Install root: $installRoot
- Install root exists: $(Test-Path -LiteralPath $installRoot -PathType Container)
- Total installed size bytes: $totalInstalledSize
- local-ai path: $localAiPath
- local-ai size bytes: $localAiSize
- Manifest path: $(Join-Path $localAiPath "config\models.json")
- Uses development local-ai path: $usesDevPath
- Missing required resources: $($result.missingRequiredResources -join "; ")
- JSON report: $jsonPath

## Required Assets

| Kind | Label | Required | Exists | Size bytes | Path |
| --- | --- | --- | --- | --- | --- |
$($assetRows -join "`n")

## Largest Local AI Files

| Size bytes | Path |
| --- | --- |
$($largestRows -join "`n")
"@ | Out-File -LiteralPath $mdPath -Encoding utf8

Write-Host "Installed local AI inventory: $status ($mdPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
