# R2H PDF - Fresh Build Verification
# Fails if the v2.1.0-beta offline package could contain stale app payloads.

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$releaseVersion = "2.1.0-beta"
$appVersion = "2.1.0"
$releaseDir = Join-Path $root "release\v$releaseVersion"
$oldReleaseDir = Join-Path $root "release\v0.2.0-beta"
$appPayloadDir = Join-Path $releaseDir "app"
$offlineOutputDir = Join-Path $releaseDir "full-offline-installer"
$outputBaseName = "R2H-PDF-v$releaseVersion-Full-Offline-Setup"
$setupExe = Join-Path $offlineOutputDir "$outputBaseName.exe"
$setupBin = Join-Path $offlineOutputDir "$outputBaseName-1.bin"
$checksumFile = Join-Path $releaseDir "checksums\$outputBaseName.sha256.txt"
$freshnessFile = Join-Path $releaseDir "BUILD_FRESHNESS.txt"

$failures = New-Object System.Collections.Generic.List[string]

function Add-Failure($message) {
    $script:failures.Add($message) | Out-Null
}

function Test-RequiredFile($path, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        Add-Failure "$label missing: $path"
        return $null
    }
    Get-Item -LiteralPath $path
}

function Test-RequiredDirectory($path, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        Add-Failure "$label missing: $path"
        return $null
    }
    Get-Item -LiteralPath $path
}

function Test-TextMarker($path, $needle, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        Add-Failure "$label file missing: $path"
        return
    }

    $text = Get-Content -LiteralPath $path -Raw
    if (-not $text.Contains($needle)) {
        Add-Failure "$label missing marker '$needle' in $path"
    }
}

function Test-NoStaleText($path, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        Add-Failure "$label file missing: $path"
        return
    }

    $text = Get-Content -LiteralPath $path -Raw
    if ($text -match "release\\v0\.2\.0-beta|release/v0\.2\.0-beta|R2H-PDF-v0\.2\.0-beta|r2h-pdf_0\.2\.0") {
        Add-Failure "$label contains stale v0.2.0 package references: $path"
    }
}

Write-Host "=== R2H PDF - Fresh Build Verification ===" -ForegroundColor Cyan
Write-Host "Root: $root"
Write-Host "Release: $releaseDir"
Write-Host ""

if (Test-Path -LiteralPath $oldReleaseDir) {
    Add-Failure "Old release output still exists: $oldReleaseDir"
}

Test-RequiredDirectory $releaseDir "v$releaseVersion release folder" | Out-Null
Test-RequiredDirectory $appPayloadDir "Fresh app payload folder" | Out-Null
Test-RequiredDirectory $offlineOutputDir "Full offline output folder" | Out-Null

Test-TextMarker (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "decodeBase64ToBytes" "Render source"
Test-TextMarker (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "onRenderError" "Render source"
Test-TextMarker (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "Render exception" "Render source"
Test-TextMarker (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "finally" "Render source"
Test-TextMarker (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "R2H PDF AI Workstation" "Welcome source"
Test-TextMarker (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Edit PDF" "Welcome source"
Test-TextMarker (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Review with AI" "Welcome source"
Test-TextMarker (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Compare PDFs" "Welcome source"
Test-TextMarker (Join-Path $root "src\App.css") "btn--primary" "UI polish source"
Test-TextMarker (Join-Path $root "src\App.css") "card--elevated" "UI polish source"
Test-TextMarker (Join-Path $root "src\App.css") "callout" "UI polish source"
Test-TextMarker (Join-Path $root "src\App.css") "welcome-screen" "UI polish source"

foreach ($required in @(
    "src\features\export\ExportPanel.tsx",
    "src\features\export\ExportPanel.test.tsx",
    "src\features\ocr\OcrPanel.tsx",
    "src\features\ocr\OcrPanel.polish.test.tsx",
    "src\features\compare\ComparePanel.tsx",
    "src\features\compare\ComparePanel.polish.test.tsx",
    "src\features\forms\FormsPanel.tsx",
    "src\features\forms\FormsPanel.polish.test.tsx",
    "src\features\sign-stamp\SignStampPanel.tsx",
    "src\features\sign-stamp\SignStampPanel.polish.test.tsx"
)) {
    Test-RequiredFile (Join-Path $root $required) "UI polish file" | Out-Null
}

$iconSource = Test-RequiredFile (Join-Path $root "R2H-PDF-app-icon.ico") "Source icon"
$iconDest = Test-RequiredFile (Join-Path $root "src-tauri\icons\icon.ico") "Tauri icon"
if ($iconSource -and $iconDest) {
    $sourceHash = (Get-FileHash -LiteralPath $iconSource.FullName -Algorithm SHA256).Hash
    $destHash = (Get-FileHash -LiteralPath $iconDest.FullName -Algorithm SHA256).Hash
    if ($sourceHash -ne $destHash) {
        Add-Failure "Tauri icon hash does not match R2H-PDF-app-icon.ico"
    }
}

Test-NoStaleText (Join-Path $root "installer\r2h-pdf-full-offline.iss") "Inno script"
Test-NoStaleText (Join-Path $root "scripts\create-inno-full-offline-installer.ps1") "Inno build script"
Test-NoStaleText (Join-Path $root "scripts\install-full-offline-inno.ps1") "Inno install script"

$issText = if (Test-Path -LiteralPath (Join-Path $root "installer\r2h-pdf-full-offline.iss")) {
    Get-Content -LiteralPath (Join-Path $root "installer\r2h-pdf-full-offline.iss") -Raw
} else {
    ""
}
foreach ($required in @(
    "CreateAppDir=no",
    "Uninstallable=no",
    "CreateUninstallRegKey=no",
    "DisableDirPage=yes",
    "DisableProgramGroupPage=yes",
    "DiskSpanning=yes",
    "DiskSliceSize=2000000000",
    "SlicesPerDisk=1",
    "OutputDir=..\release\v$releaseVersion\full-offline-installer",
    "OutputBaseFilename=$outputBaseName"
)) {
    if (-not $issText.Contains($required)) {
        Add-Failure "Inno script missing required setting: $required"
    }
}

$nsisSetup = $null
$msiSetup = $null
if (Test-Path -LiteralPath $appPayloadDir -PathType Container) {
    $nsisSetup = Get-ChildItem -LiteralPath $appPayloadDir -Filter "r2h-pdf_$appVersion*_x64-setup.exe" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    $msiSetup = Get-ChildItem -LiteralPath $appPayloadDir -Filter "r2h-pdf_$appVersion*_x64*.msi" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}

if (-not $nsisSetup) { Add-Failure "Fresh NSIS payload missing from $appPayloadDir" }
if (-not $msiSetup) { Add-Failure "Fresh MSI payload missing from $appPayloadDir" }
if ($nsisSetup -and $nsisSetup.Name -match "0\.2\.0|0\.2\.1") { Add-Failure "NSIS payload is stale: $($nsisSetup.Name)" }
if ($msiSetup -and $msiSetup.Name -match "0\.2\.0|0\.2\.1") { Add-Failure "MSI payload is stale: $($msiSetup.Name)" }

$sourceFiles = @(
    (Join-Path $root "package.json"),
    (Join-Path $root "src-tauri\Cargo.toml"),
    (Join-Path $root "src-tauri\tauri.conf.json"),
    (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx"),
    (Join-Path $root "src\components\shell\WelcomeScreen.tsx"),
    (Join-Path $root "src\App.css"),
    (Join-Path $root "src\features\export\ExportPanel.tsx"),
    (Join-Path $root "src\features\ocr\OcrPanel.tsx"),
    (Join-Path $root "src\features\compare\ComparePanel.tsx"),
    (Join-Path $root "src\features\forms\FormsPanel.tsx"),
    (Join-Path $root "src\features\sign-stamp\SignStampPanel.tsx"),
    (Join-Path $root "R2H-PDF-app-icon.ico")
)
$existingSourceFiles = $sourceFiles | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | ForEach-Object { Get-Item -LiteralPath $_ }
if ($existingSourceFiles.Count -gt 0 -and $nsisSetup -and $msiSetup) {
    $latestSource = ($existingSourceFiles | Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime
    if ($nsisSetup.LastWriteTime -lt $latestSource) {
        Add-Failure "NSIS payload is older than source markers: $($nsisSetup.FullName)"
    }
    if ($msiSetup.LastWriteTime -lt $latestSource) {
        Add-Failure "MSI payload is older than source markers: $($msiSetup.FullName)"
    }
}

$finalExe = Test-RequiredFile $setupExe "Disk-spanned setup EXE"
$finalBin = Test-RequiredFile $setupBin "Disk-spanned setup BIN"
if ($finalExe -and $finalExe.Name -match "0\.2\.0|0\.2\.1") { Add-Failure "Setup EXE filename is stale: $($finalExe.Name)" }
if ($finalBin -and $finalBin.Name -match "0\.2\.0|0\.2\.1") { Add-Failure "Setup BIN filename is stale: $($finalBin.Name)" }
$finalBins = @()
if (Test-Path -LiteralPath $offlineOutputDir -PathType Container) {
    $finalBins = @(Get-ChildItem -LiteralPath $offlineOutputDir -Filter "$outputBaseName-*.bin" -File | Sort-Object Name)
    if ($finalBins.Count -lt 1) {
        Add-Failure "No disk-spanned .bin files found in $offlineOutputDir"
    }
    foreach ($bin in $finalBins) {
        if ($bin.Name -match "0\.2\.0|0\.2\.1") {
            Add-Failure "Setup BIN filename is stale: $($bin.Name)"
        }
    }
}

Test-RequiredFile $checksumFile "SHA256 checksum file" | Out-Null
Test-RequiredFile $freshnessFile "BUILD_FRESHNESS proof" | Out-Null
if (Test-Path -LiteralPath $checksumFile -PathType Leaf) {
    $checksumText = Get-Content -LiteralPath $checksumFile -Raw
    if (-not $checksumText.Contains("$outputBaseName.exe")) {
        Add-Failure "SHA256 file does not include setup EXE: $checksumFile"
    }
    foreach ($bin in $finalBins) {
        if (-not $checksumText.Contains($bin.Name)) {
            Add-Failure "SHA256 file does not include disk-spanned slice $($bin.Name): $checksumFile"
        }
    }
    if ($finalBins.Count -gt 0 -and -not $checksumText.Contains("$outputBaseName-1.bin")) {
        Add-Failure "SHA256 file does not include first disk-spanned slice: $checksumFile"
    }
    if ($checksumText -match "0\.2\.0|0\.2\.1") {
        Add-Failure "SHA256 file contains stale version text: $checksumFile"
    }
}

if (Test-Path -LiteralPath $releaseDir -PathType Container) {
    $releaseTextFiles = @()
    $releaseTextFiles += Get-ChildItem -LiteralPath $releaseDir -File -Include *.txt,*.md,*.ps1,*.iss,*.json -ErrorAction SilentlyContinue
    foreach ($subdir in @(
        (Join-Path $releaseDir "checksums"),
        (Join-Path $releaseDir "full-offline-inno-staging\docs"),
        (Join-Path $releaseDir "full-offline-inno-staging\scripts")
    )) {
        if (Test-Path -LiteralPath $subdir -PathType Container) {
            $releaseTextFiles += Get-ChildItem -LiteralPath $subdir -Recurse -File -Include *.txt,*.md,*.ps1,*.iss,*.json -ErrorAction SilentlyContinue
        }
    }
    foreach ($textFile in $releaseTextFiles) {
        $content = Get-Content -LiteralPath $textFile.FullName -Raw
        if ($content -match "v0\.2\.0|0\.2\.0|r2h-pdf_0\.2\.0|R2H-PDF-v0\.2\.0") {
            Add-Failure "Release text artifact contains stale version text: $($textFile.FullName)"
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Host ""
    Write-Host "FRESHNESS STATUS: FAIL" -ForegroundColor Red
    foreach ($failure in $failures) {
        Write-Host "  - $failure" -ForegroundColor Red
    }
    exit 1
}

Write-Host "FRESHNESS STATUS: PASS" -ForegroundColor Green
Write-Host "  Setup EXE: $setupExe"
Write-Host "  Data BIN: $setupBin"
Write-Host "  SHA256: $checksumFile"
Write-Host "  BUILD_FRESHNESS: $freshnessFile"
exit 0
