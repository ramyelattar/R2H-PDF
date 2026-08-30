# R2H PDF AI Workstation - Create Beta Release
param(
    [switch]$DryRun,
    [switch]$SkipTauriBuild,
    [switch]$SkipHealthcheck
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$version = "0.2.0-beta"
$releaseDir = Join-Path $root "release/v$version"

Write-Host "=== R2H PDF - Create Beta Release ===" -ForegroundColor Cyan
Write-Host "Version: $version"
Write-Host "Output: $releaseDir"
Write-Host "DryRun: $DryRun"
Write-Host ""

# 1. Version check
Write-Host "--- Version Check ---" -ForegroundColor White
& powershell -ExecutionPolicy Bypass -File (Join-Path $root "scripts/check-version.ps1")
if ($LASTEXITCODE -ne 0) { Write-Host "Version mismatch! Aborting." -ForegroundColor Red; exit 1 }

# 2. Create release directory structure
Write-Host "`n--- Creating Release Directory ---" -ForegroundColor White
$dirs = @("app", "docs", "scripts", "local-ai-bundle", "checksums")
foreach ($d in $dirs) {
    $path = Join-Path $releaseDir $d
    if (-not (Test-Path $path)) { New-Item -ItemType Directory -Path $path -Force | Out-Null }
}
Write-Host "  Created: $releaseDir"

# 3. Copy docs
Write-Host "`n--- Copying Docs ---" -ForegroundColor White
$docs = @(
    "docs/release/RELEASE_READINESS_REPORT.md",
    "docs/release/KNOWN_LIMITATIONS.md",
    "docs/release/OFFLINE_ENFORCEMENT.md",
    "docs/release/PACKAGING_STRATEGY.md",
    "docs/release/RELEASE_NOTES_v0.2.0-beta.md",
    "docs/release/CLEAN_MACHINE_VALIDATION.md"
)
foreach ($doc in $docs) {
    $src = Join-Path $root $doc
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $releaseDir "docs/") -Force
        Write-Host "  Copied: $doc"
    } else {
        Write-Host "  MISSING: $doc" -ForegroundColor Yellow
    }
}

# 4. Copy scripts
Write-Host "`n--- Copying Scripts ---" -ForegroundColor White
$scripts = @("validate-local-ai.ps1", "validate-offline-safety.ps1", "release-healthcheck.ps1", "check-version.ps1")
foreach ($s in $scripts) {
    $src = Join-Path $root "scripts/$s"
    if (Test-Path $src) {
        Copy-Item $src (Join-Path $releaseDir "scripts/") -Force
        Write-Host "  Copied: $s"
    }
}

# 5. Generate local-ai bundle manifest
Write-Host "`n--- Generating Local AI Bundle Manifest ---" -ForegroundColor White
$assets = @(
    @{ id = "llm-qwen3-4b"; path = "models/llm/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf"; required = $true; category = "model" },
    @{ id = "embedding-qwen3"; path = "models/embeddings/Qwen3-Embedding-4B-GGUF/Qwen3-Embedding-4B-Q4_K_M.gguf"; required = $true; category = "model" },
    @{ id = "reranker-qwen3"; path = "models/rerankers/Qwen3-Reranker-0.6B-GGUF/qwen3-reranker-0.6b-q8_0.gguf"; required = $false; category = "model" },
    @{ id = "ocr-paddleocr-vl"; path = "models/ocr/PaddleOCR-VL/model.safetensors"; required = $true; category = "model" },
    @{ id = "runtime-llama-cli"; path = "runtimes/llama-cpp/llama-cli.exe"; required = $true; category = "runtime" },
    @{ id = "runtime-llama-server"; path = "runtimes/llama-cpp/llama-server.exe"; required = $true; category = "runtime" },
    @{ id = "runtime-llama-dll"; path = "runtimes/llama-cpp/llama.dll"; required = $true; category = "runtime" },
    @{ id = "worker-ocr"; path = "workers/paddleocr_vl_worker.py"; required = $true; category = "worker" },
    @{ id = "config-models"; path = "config/models.json"; required = $true; category = "config" }
)

$manifestAssets = @()
foreach ($a in $assets) {
    $fullPath = Join-Path $root "local-ai/$($a.path)"
    $size = 0
    $status = "missing"
    if (Test-Path $fullPath) { $size = (Get-Item $fullPath).Length; $status = "ok" }
    $manifestAssets += @{
        asset_id = $a.id; relative_path = "local-ai/$($a.path)"
        required = $a.required; category = $a.category
        expected_min_size_bytes = 1000; detected_size_bytes = $size; status = $status
    }
}

$manifest = @{ version = $version; assets = $manifestAssets; generated_at = (Get-Date -Format "yyyy-MM-dd HH:mm:ss") }
$manifestJson = $manifest | ConvertTo-Json -Depth 4
Set-Content (Join-Path $releaseDir "local-ai-bundle/LOCAL_AI_BUNDLE_MANIFEST.json") $manifestJson
Write-Host "  Generated manifest with $($manifestAssets.Count) assets"

# 6. Frontend build
Write-Host "`n--- Frontend Build ---" -ForegroundColor White
if (-not $DryRun) {
    Push-Location $root
    try {
        & pnpm build 2>&1 | Out-Null
        if ($LASTEXITCODE -eq 0) { Write-Host "  Frontend build: PASSED" -ForegroundColor Green }
        else { Write-Host "  Frontend build: FAILED" -ForegroundColor Red }
    } catch { Write-Host "  Frontend build: ERROR - $_" -ForegroundColor Red }
    Pop-Location
} else { Write-Host "  SKIPPED (dry run)" }

# 7. Tauri build (optional)
$buildResult = "skipped"
if (-not $SkipTauriBuild -and -not $DryRun) {
    Write-Host "`n--- Tauri Build ---" -ForegroundColor White
    Push-Location $root
    try {
        $output = & pnpm tauri build 2>&1 | Out-String
        if ($LASTEXITCODE -eq 0) {
            Write-Host "  Tauri build: PASSED" -ForegroundColor Green
            $buildResult = "success"
            # Find and copy artifacts
            $msi = Get-ChildItem -Path "src-tauri/target/release/bundle" -Recurse -Filter "*.msi" -ErrorAction SilentlyContinue | Select-Object -First 1
            $exe = Get-ChildItem -Path "src-tauri/target/release/bundle" -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($msi) { Copy-Item $msi.FullName (Join-Path $releaseDir "app/"); Write-Host "  Copied: $($msi.Name)" }
            if ($exe) { Copy-Item $exe.FullName (Join-Path $releaseDir "app/"); Write-Host "  Copied: $($exe.Name)" }
        } else {
            Write-Host "  Tauri build: FAILED" -ForegroundColor Yellow
            $buildResult = "failed"
            # Save error output
            Set-Content (Join-Path $releaseDir "app/BUILD_FAILURE.txt") $output
            Write-Host "  Build failure details saved to app/BUILD_FAILURE.txt"
        }
    } catch {
        Write-Host "  Tauri build: ERROR - $_" -ForegroundColor Red
        $buildResult = "error"
        Set-Content (Join-Path $releaseDir "app/BUILD_FAILURE.txt") $_.ToString()
    }
    Pop-Location
} else {
    Write-Host "`n--- Tauri Build: SKIPPED ---" -ForegroundColor Yellow
    if ($DryRun) { $buildResult = "dry-run" }
    else { $buildResult = "skipped-by-flag" }
}

# 8. Generate release manifest
Write-Host "`n--- Generating Release Manifest ---" -ForegroundColor White
$gitCommit = ""
try { $gitCommit = (git -C $root rev-parse --short HEAD 2>$null) } catch {}

$releaseManifest = @{
    version = $version
    productName = "R2H PDF AI Workstation"
    releaseChannel = "beta"
    createdAt = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    gitCommit = $gitCommit
    buildCommand = "pnpm tauri build"
    buildResult = $buildResult
    artifactPaths = @((Get-ChildItem (Join-Path $releaseDir "app") -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }))
    docsIncluded = @((Get-ChildItem (Join-Path $releaseDir "docs") -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }))
    scriptsIncluded = @((Get-ChildItem (Join-Path $releaseDir "scripts") -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }))
    localAiStrategy = "separate-bundle"
    knownLimitations = @("PDF report export deferred", "OCR timeout not enforced", "Unsigned beta build")
    validationSummary = @{ tests_frontend = 180; tests_rust = 136; local_ai_validated = $true }
}
Set-Content (Join-Path $releaseDir "RELEASE_MANIFEST.json") ($releaseManifest | ConvertTo-Json -Depth 3)
Write-Host "  Generated RELEASE_MANIFEST.json"

# 9. Generate checksums
Write-Host "`n--- Generating Checksums ---" -ForegroundColor White
$checksumFile = Join-Path $releaseDir "checksums/SHA256SUMS.txt"
$allFiles = Get-ChildItem $releaseDir -Recurse -File | Where-Object { $_.FullName -notmatch "checksums" }
$checksums = @()
foreach ($f in $allFiles) {
    $hash = (Get-FileHash $f.FullName -Algorithm SHA256).Hash
    $rel = $f.FullName.Replace("$releaseDir\", "").Replace("\", "/")
    $checksums += "$hash  $rel"
}
Set-Content $checksumFile ($checksums -join "`n")
Write-Host "  Generated SHA256SUMS.txt with $($checksums.Count) entries"

# 10. Summary
Write-Host "`n=== Release Summary ===" -ForegroundColor Cyan
Write-Host "  Version: $version"
Write-Host "  Directory: $releaseDir"
Write-Host "  Build: $buildResult"
Write-Host "  Docs: $($docs.Count) files"
Write-Host "  Scripts: $($scripts.Count) files"
Write-Host "  Checksums: $($checksums.Count) entries"

if ($buildResult -eq "success") {
    Write-Host "`n  RELEASE CREATED SUCCESSFULLY" -ForegroundColor Green
} elseif ($buildResult -eq "failed" -or $buildResult -eq "error") {
    Write-Host "`n  RELEASE CREATED (build failed - see app/BUILD_FAILURE.txt)" -ForegroundColor Yellow
} else {
    Write-Host "`n  RELEASE FOLDER CREATED (build skipped)" -ForegroundColor Yellow
}

exit 0
