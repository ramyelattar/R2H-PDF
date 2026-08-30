# R2H PDF AI Workstation - Local AI Asset Validation
$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$failures = 0
$warnings = 0

Write-Host "=== R2H PDF - Local AI Validation ===" -ForegroundColor Cyan

function Check-File {
    param([string]$path, [string]$label)
    $full = Join-Path $root $path
    if (Test-Path $full) {
        $size = (Get-Item $full).Length
        if ($size -eq 0) {
            Write-Host "  WARN: $label exists but is empty" -ForegroundColor Yellow
            $script:warnings++
            return
        }
        $sizeMB = [math]::Round($size / 1MB, 1)
        Write-Host "  OK: $label - $sizeMB MB" -ForegroundColor Green
    } else {
        Write-Host "  FAIL: $label NOT FOUND" -ForegroundColor Red
        $script:failures++
    }
}

Write-Host "`n--- Models ---"
Check-File "local-ai/models/llm/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf" "LLM Qwen3-4B"
Check-File "local-ai/models/embeddings/Qwen3-Embedding-4B-GGUF/Qwen3-Embedding-4B-Q4_K_M.gguf" "Embedding Qwen3-Embedding-4B"
Check-File "local-ai/models/rerankers/Qwen3-Reranker-0.6B-GGUF/qwen3-reranker-0.6b-q8_0.gguf" "Reranker Qwen3-Reranker-0.6B"
Check-File "local-ai/models/ocr/PaddleOCR-VL/model.safetensors" "OCR PaddleOCR-VL model"
Check-File "local-ai/models/ocr/PaddleOCR-VL/config.json" "OCR PaddleOCR-VL config"

Write-Host "`n--- Runtimes ---"
Check-File "local-ai/runtimes/llama-cpp/llama-cli.exe" "llama-cli.exe"
Check-File "local-ai/runtimes/llama-cpp/llama-server.exe" "llama-server.exe"
Check-File "local-ai/runtimes/llama-cpp/llama.dll" "llama.dll"
Check-File "local-ai/runtimes/llama-cpp/ggml.dll" "ggml.dll"

Write-Host "`n--- Workers ---"
Check-File "local-ai/workers/paddleocr_vl_worker.py" "PaddleOCR-VL worker"

Write-Host "`n--- Smoke Tests ---"
$llamaCli = Join-Path $root "local-ai/runtimes/llama-cpp/llama-cli.exe"
if (Test-Path $llamaCli) {
    try {
        $null = & $llamaCli --version 2>&1
        Write-Host "  OK: llama-cli responds" -ForegroundColor Green
    } catch {
        Write-Host "  WARN: llama-cli test failed" -ForegroundColor Yellow
        $warnings++
    }
}

try {
    $pyOut = py (Join-Path $root "local-ai/workers/paddleocr_vl_worker.py") --version 2>&1 | Out-String
    if ($pyOut -match "PaddleOCR") {
        Write-Host "  OK: Python OCR worker responds" -ForegroundColor Green
    } else {
        Write-Host "  WARN: Python worker unexpected output" -ForegroundColor Yellow
        $warnings++
    }
} catch {
    Write-Host "  WARN: Python worker test failed" -ForegroundColor Yellow
    $warnings++
}

Write-Host "`n=== Summary ==="
Write-Host "Failures: $failures"
Write-Host "Warnings: $warnings"
if ($failures -gt 0) { exit 1 } else { exit 0 }
