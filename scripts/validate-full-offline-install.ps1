# R2H PDF AI Workstation - Validate Full Offline Installation
# Checks that local-ai was installed correctly and all assets are present.

$ErrorActionPreference = "Continue"

Write-Host "=== R2H PDF - Full Offline Install Validation ===" -ForegroundColor Cyan

# Resolve local-ai root
$localAiRoot = $env:R2H_LOCAL_AI_ROOT
if (-not $localAiRoot -or -not (Test-Path $localAiRoot)) {
    $localAiRoot = Join-Path $env:LOCALAPPDATA "R2H-PDF\local-ai"
}

Write-Host "  Local AI root: $localAiRoot"
Write-Host ""

$failures = 0
$warnings = 0

function Check-File($relPath, $label, $required) {
    $fullPath = Join-Path $localAiRoot $relPath
    if (Test-Path $fullPath) {
        $size = (Get-Item $fullPath).Length
        $sizeMB = [math]::Round($size / 1MB, 1)
        Write-Host "  OK: $label ($sizeMB MB)" -ForegroundColor Green
    } else {
        if ($required) {
            Write-Host "  FAIL: $label - NOT FOUND: $relPath" -ForegroundColor Red
            $script:failures++
        } else {
            Write-Host "  WARN: $label - not found (optional): $relPath" -ForegroundColor Yellow
            $script:warnings++
        }
    }
}

function Check-Dir($relPath, $label) {
    $fullPath = Join-Path $localAiRoot $relPath
    if (Test-Path $fullPath) {
        Write-Host "  OK: $label" -ForegroundColor Green
    } else {
        Write-Host "  FAIL: $label - NOT FOUND: $relPath" -ForegroundColor Red
        $script:failures++
    }
}

# Check root exists
if (-not (Test-Path $localAiRoot)) {
    Write-Host "  FAIL: Local AI root does not exist: $localAiRoot" -ForegroundColor Red
    Write-Host ""
    Write-Host "=== VALIDATION FAILED ===" -ForegroundColor Red
    exit 1
}

Write-Host "--- Configuration ---"
Check-File "config\models.json" "models.json config" $true

Write-Host ""
Write-Host "--- Models ---"
Check-File "models\llm\Qwen3-4B-GGUF\Qwen3-4B-Q4_K_M.gguf" "Qwen3 4B LLM" $true
Check-File "models\embeddings\Qwen3-Embedding-4B-GGUF\Qwen3-Embedding-4B-Q4_K_M.gguf" "Qwen3 Embedding 4B" $true
Check-File "models\rerankers\Qwen3-Reranker-0.6B-GGUF\qwen3-reranker-0.6b-q8_0.gguf" "Qwen3 Reranker 0.6B" $false
Check-File "models\ocr\PaddleOCR-VL\model.safetensors" "PaddleOCR-VL model" $true
Check-File "models\ocr\PaddleOCR-VL\config.json" "PaddleOCR-VL config" $true

Write-Host ""
Write-Host "--- Runtimes ---"
Check-File "runtimes\llama-cpp\llama-cli.exe" "llama-cli.exe" $true
Check-File "runtimes\llama-cpp\llama-server.exe" "llama-server.exe" $true
Check-File "runtimes\llama-cpp\llama.dll" "llama.dll" $true
Check-File "runtimes\llama-cpp\ggml.dll" "ggml.dll" $true

Write-Host ""
Write-Host "--- Workers ---"
Check-File "workers\paddleocr_vl_worker.py" "PaddleOCR-VL worker" $true

Write-Host ""
Write-Host "--- Runtime Smoke Tests ---"

$llamaCli = Join-Path $localAiRoot "runtimes\llama-cpp\llama-cli.exe"
if (Test-Path $llamaCli) {
    try {
        $output = & $llamaCli --version 2>&1 | Out-String
        if ($output -match "version|llama|ggml") {
            Write-Host "  OK: llama-cli responds" -ForegroundColor Green
        } else {
            Write-Host "  WARN: llama-cli output unexpected" -ForegroundColor Yellow
            $warnings++
        }
    } catch {
        Write-Host "  WARN: llama-cli execution failed: $_" -ForegroundColor Yellow
        $warnings++
    }
} else {
    Write-Host "  SKIP: llama-cli not found" -ForegroundColor DarkGray
}

$llamaServer = Join-Path $localAiRoot "runtimes\llama-cpp\llama-server.exe"
if (Test-Path $llamaServer) {
    try {
        $output = & $llamaServer --version 2>&1 | Out-String
        if ($output -match "version|llama|ggml") {
            Write-Host "  OK: llama-server responds" -ForegroundColor Green
        } else {
            Write-Host "  WARN: llama-server output unexpected" -ForegroundColor Yellow
            $warnings++
        }
    } catch {
        Write-Host "  WARN: llama-server execution failed: $_" -ForegroundColor Yellow
        $warnings++
    }
} else {
    Write-Host "  SKIP: llama-server not found" -ForegroundColor DarkGray
}

Write-Host ""
Write-Host "--- Environment ---"
$envVar = [System.Environment]::GetEnvironmentVariable("R2H_LOCAL_AI_ROOT", "User")
if ($envVar) {
    Write-Host "  OK: R2H_LOCAL_AI_ROOT = $envVar" -ForegroundColor Green
    if (-not (Test-Path $envVar)) {
        Write-Host "  WARN: R2H_LOCAL_AI_ROOT path does not exist" -ForegroundColor Yellow
        $warnings++
    }
} else {
    Write-Host "  WARN: R2H_LOCAL_AI_ROOT not set in user environment" -ForegroundColor Yellow
    $warnings++
}

Write-Host ""
Write-Host "--- Disk Usage ---"
$totalSize = (Get-ChildItem $localAiRoot -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
$totalSizeGB = [math]::Round($totalSize / 1GB, 2)
Write-Host "  Total local-ai size: $totalSizeGB GB"

if ($totalSizeGB -lt 5) {
    Write-Host "  WARN: Total size seems low (expected 7+ GB). Some models may be missing." -ForegroundColor Yellow
    $warnings++
}

Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host "  Failures: $failures"
Write-Host "  Warnings: $warnings"

if ($failures -eq 0) {
    Write-Host ""
    Write-Host "VALIDATION PASSED" -ForegroundColor Green
    exit 0
} else {
    Write-Host ""
    Write-Host "VALIDATION FAILED ($failures critical issues)" -ForegroundColor Red
    exit 1
}
