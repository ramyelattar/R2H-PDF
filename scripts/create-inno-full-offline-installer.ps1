# R2H PDF AI Workstation - Create Full Offline Installer (Inno Setup)
# Stages fresh v2.1.0-beta payloads and compiles the disk-spanned offline wrapper.

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$releaseVersion = "2.1.0-beta"
$appVersion = "2.1.0"
$releaseDir = Join-Path $root "release\v$releaseVersion"
$appPayloadDir = Join-Path $releaseDir "app"
$stagingDir = Join-Path $releaseDir "full-offline-inno-staging"
$offlineOutputDir = Join-Path $releaseDir "full-offline-installer"
$issFile = Join-Path $root "installer\r2h-pdf-full-offline.iss"
$outputBaseName = "R2H-PDF-v$releaseVersion-Full-Offline-Setup"
$outputExe = Join-Path $offlineOutputDir "$outputBaseName.exe"
$outputBin = Join-Path $offlineOutputDir "$outputBaseName-1.bin"

function Fail($message) {
    Write-Host "ERROR: $message" -ForegroundColor Red
    exit 1
}

function Require-File($path, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        Fail "$label not found: $path"
    }
    Get-Item -LiteralPath $path
}

function Require-Directory($path, $label) {
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        Fail "$label not found: $path"
    }
    Get-Item -LiteralPath $path
}

function Require-Text($path, $needle, $label) {
    $text = Get-Content -LiteralPath $path -Raw
    if (-not $text.Contains($needle)) {
        Fail "$label missing marker '$needle' in $path"
    }
}

function Assert-NoStaleText($path, $label) {
    $text = Get-Content -LiteralPath $path -Raw
    if ($text -match "release\\v0\.2\.0-beta|release/v0\.2\.0-beta|R2H-PDF-v0\.2\.0-beta|r2h-pdf_0\.2\.0") {
        Fail "$label still contains stale v0.2.0 references: $path"
    }
}

function Remove-UnsafeLocalAiAuxiliaryContent($localAiRoot) {
    # The DocLayout-YOLO repository contains training, demo, export, hub, download,
    # telemetry, and cloud/API helper code. R2H runtime OCR validation uses
    # local-ai/workers/paddleocr_vl_worker.py with local PaddleOCR-VL assets only,
    # so this auxiliary source tree must not be shipped in release staging.
    $unsafeRelativePaths = @(
        "models\layout\DocLayout-YOLO"
    )

    foreach ($relativePath in $unsafeRelativePaths) {
        $candidate = Join-Path $localAiRoot $relativePath
        if (Test-Path -LiteralPath $candidate) {
            $resolvedRoot = (Resolve-Path -LiteralPath $localAiRoot).Path
            $resolvedCandidate = (Resolve-Path -LiteralPath $candidate).Path
            if (-not $resolvedCandidate.StartsWith($resolvedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
                Fail "Refusing to remove path outside staged local-ai: $resolvedCandidate"
            }
            Write-Host "  Removing unsafe auxiliary local-ai content: $relativePath" -ForegroundColor Yellow
            Remove-Item -LiteralPath $resolvedCandidate -Recurse -Force
        }
    }

    Get-ChildItem -LiteralPath $localAiRoot -Directory -Filter ".git" -Recurse -Force -ErrorAction SilentlyContinue |
        ForEach-Object {
            Write-Host "  Removing VCS metadata from staged local-ai: $($_.FullName)" -ForegroundColor Yellow
            Remove-Item -LiteralPath $_.FullName -Recurse -Force
        }
}

Write-Host "=== R2H PDF - Inno Setup Full Offline Installer Builder ===" -ForegroundColor Cyan
Write-Host "Release version: $releaseVersion"
Write-Host "App version: $appVersion"
Write-Host "Root: $root"
Write-Host "Release: $releaseDir"
Write-Host "Staging: $stagingDir"
Write-Host "Output folder: $offlineOutputDir"
Write-Host ""

Write-Host "--- Finding Inno Setup Compiler ---"
$isccCandidates = @(
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)

$iscc = $isccCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
if (-not $iscc) {
    Write-Host "Searched:" -ForegroundColor Yellow
    foreach ($candidate in $isccCandidates) { Write-Host "  $candidate" -ForegroundColor Yellow }
    Fail "Inno Setup 6 compiler (ISCC.exe) not found."
}
Write-Host "  Found: $iscc" -ForegroundColor Green

Write-Host ""
Write-Host "--- Verifying Fresh Source Markers ---"
Require-Text (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "decodeBase64ToBytes" "Render fix"
Require-Text (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "onRenderError" "Render fix"
Require-Text (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "Render exception" "Render fix"
Require-Text (Join-Path $root "src\components\viewer\PdfCanvasViewer.tsx") "finally" "Render fix"
Require-Text (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "R2H PDF AI Workstation" "Welcome screen"
Require-Text (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Edit PDF" "Welcome screen"
Require-Text (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Review with AI" "Welcome screen"
Require-Text (Join-Path $root "src\components\shell\WelcomeScreen.tsx") "Compare PDFs" "Welcome screen"
Require-Text (Join-Path $root "src\App.css") "btn--primary" "UI polish"
Require-Text (Join-Path $root "src\App.css") "card--elevated" "UI polish"
Require-Text (Join-Path $root "src\App.css") "callout" "UI polish"
Require-Text (Join-Path $root "src\App.css") "welcome-screen" "UI polish"
Require-File (Join-Path $root "src\features\export\ExportPanel.tsx") "ExportPanel polish" | Out-Null
Require-File (Join-Path $root "src\features\export\ExportPanel.test.tsx") "ExportPanel tests" | Out-Null
Require-File (Join-Path $root "src\features\ocr\OcrPanel.tsx") "OcrPanel polish" | Out-Null
Require-File (Join-Path $root "src\features\ocr\OcrPanel.polish.test.tsx") "OcrPanel tests" | Out-Null
Require-File (Join-Path $root "src\features\compare\ComparePanel.tsx") "ComparePanel polish" | Out-Null
Require-File (Join-Path $root "src\features\compare\ComparePanel.polish.test.tsx") "ComparePanel tests" | Out-Null
Require-File (Join-Path $root "src\features\forms\FormsPanel.tsx") "FormsPanel polish" | Out-Null
Require-File (Join-Path $root "src\features\forms\FormsPanel.polish.test.tsx") "FormsPanel tests" | Out-Null
Require-File (Join-Path $root "src\features\sign-stamp\SignStampPanel.tsx") "SignStampPanel polish" | Out-Null
Require-File (Join-Path $root "src\features\sign-stamp\SignStampPanel.polish.test.tsx") "SignStampPanel tests" | Out-Null

$iconSource = Require-File (Join-Path $root "R2H-PDF-app-icon.ico") "Source icon"
$iconDest = Require-File (Join-Path $root "src-tauri\icons\icon.ico") "Tauri icon"
$iconSourceHash = (Get-FileHash -LiteralPath $iconSource.FullName -Algorithm SHA256).Hash
$iconDestHash = (Get-FileHash -LiteralPath $iconDest.FullName -Algorithm SHA256).Hash
if ($iconSourceHash -ne $iconDestHash) {
    Fail "src-tauri\icons\icon.ico does not match R2H-PDF-app-icon.ico"
}
Write-Host "  Source markers and icon match." -ForegroundColor Green

Write-Host ""
Write-Host "--- Verifying Fresh App Payload ---"
Require-Directory $appPayloadDir "Release app payload folder" | Out-Null
$nsisSetup = Get-ChildItem -LiteralPath $appPayloadDir -Filter "r2h-pdf_$appVersion*_x64-setup.exe" -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
$msiSetup = Get-ChildItem -LiteralPath $appPayloadDir -Filter "r2h-pdf_$appVersion*_x64*.msi" -File -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1

if (-not $nsisSetup) { Fail "Fresh NSIS payload not found in $appPayloadDir" }
if (-not $msiSetup) { Fail "Fresh MSI payload not found in $appPayloadDir" }
if ($nsisSetup.Name -match "0\.2\.0|0\.2\.1" -or $msiSetup.Name -match "0\.2\.0|0\.2\.1") {
    Fail "Release app payload contains stale installer names."
}

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
$latestSourceWrite = ($sourceFiles | ForEach-Object { Get-Item -LiteralPath $_ } | Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime
if ($nsisSetup.LastWriteTime -lt $latestSourceWrite -or $msiSetup.LastWriteTime -lt $latestSourceWrite) {
    Fail "Release app payload is older than source markers. Re-run pnpm tauri build and copy fresh target payloads."
}

Write-Host "  NSIS: $($nsisSetup.FullName) ($([math]::Round($nsisSetup.Length / 1MB, 1)) MB, $($nsisSetup.LastWriteTime))" -ForegroundColor Green
Write-Host "  MSI:  $($msiSetup.FullName) ($([math]::Round($msiSetup.Length / 1MB, 1)) MB, $($msiSetup.LastWriteTime))" -ForegroundColor Green

Write-Host ""
Write-Host "--- Verifying Installer Script Contract ---"
Assert-NoStaleText $issFile "Inno script"
Assert-NoStaleText $PSCommandPath "Inno build script"
Require-Text $issFile "OutputDir=..\release\v$releaseVersion\full-offline-installer" "Inno output"
Require-Text $issFile "OutputBaseFilename=$outputBaseName" "Inno output"
Require-Text $issFile "DiskSpanning=yes" "Inno disk spanning"
Require-Text $issFile "DiskSliceSize=2000000000" "Inno disk spanning"
Require-Text $issFile "SlicesPerDisk=1" "Inno disk spanning"
Require-Text $issFile "CreateAppDir=no" "Inno bootstrapper"
Require-Text $issFile "Uninstallable=no" "Inno bootstrapper"
Require-Text $issFile "CreateUninstallRegKey=no" "Inno bootstrapper"
Write-Host "  Inno script points only at v$releaseVersion and is disk-spanned." -ForegroundColor Green

Write-Host ""
Write-Host "--- Creating Staging Directory ---"
if (Test-Path -LiteralPath $stagingDir) {
    Remove-Item -LiteralPath $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

$installAppStaging = Join-Path $stagingDir "install-app"
New-Item -ItemType Directory -Path $installAppStaging -Force | Out-Null
Copy-Item -LiteralPath $nsisSetup.FullName -Destination $installAppStaging -Force
Copy-Item -LiteralPath $msiSetup.FullName -Destination $installAppStaging -Force

$localAiSrc = Join-Path $root "local-ai"
Require-Directory $localAiSrc "local-ai folder" | Out-Null
$destLocalAi = Join-Path $stagingDir "local-ai"
Write-Host "  Copying local-ai folder (this may take several minutes)..."
Copy-Item -LiteralPath $localAiSrc -Destination $destLocalAi -Recurse -Force
Remove-UnsafeLocalAiAuxiliaryContent $destLocalAi

$scriptsDir = Join-Path $stagingDir "scripts"
New-Item -ItemType Directory -Path $scriptsDir -Force | Out-Null
$scriptFiles = @(
    "install-full-offline-inno.ps1",
    "validate-local-ai.ps1",
    "validate-full-offline-install.ps1"
)
foreach ($scriptFile in $scriptFiles) {
    $src = Join-Path $root "scripts\$scriptFile"
    if (Test-Path -LiteralPath $src) {
        Copy-Item -LiteralPath $src -Destination $scriptsDir -Force
    } else {
        Write-Host "  WARNING: Script not found: $scriptFile" -ForegroundColor Yellow
    }
}

$docsDir = Join-Path $stagingDir "docs"
New-Item -ItemType Directory -Path $docsDir -Force | Out-Null
$docFiles = @(
    "KNOWN_LIMITATIONS.md",
    "CLEAN_MACHINE_VALIDATION.md",
    "OFFLINE_ENFORCEMENT.md"
)
foreach ($docFile in $docFiles) {
    $src = Join-Path $root "docs\release\$docFile"
    if (Test-Path -LiteralPath $src) {
        Copy-Item -LiteralPath $src -Destination $docsDir -Force
    }
}

$packageNotes = @"
# R2H PDF AI Workstation v$releaseVersion Full Offline Package

This folder was staged from the fresh v$releaseVersion build.

- App installer: $($nsisSetup.Name)
- MSI payload: $($msiSetup.Name)
- Local AI assets: included
- Disk spanning: enabled
- Wrapper uninstall entry: disabled

Run the setup executable from the full-offline-installer folder and keep every data slice beside it.
"@
Set-Content -Path (Join-Path $docsDir "RELEASE_PACKAGE_NOTES_v$releaseVersion.md") -Value $packageNotes -Encoding UTF8

$stagingSize = (Get-ChildItem -LiteralPath $stagingDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$stagingSizeGB = [math]::Round($stagingSize / 1GB, 2)
Write-Host "  Staging size: $stagingSizeGB GB" -ForegroundColor Cyan

Write-Host ""
Write-Host "--- Running Inno Setup Compiler ---"
Write-Host "  Disk spanning: EXE + .bin"
Write-Host "  Compression: LZMA2/fast"
$startTime = Get-Date
$proc = Start-Process -FilePath $iscc -ArgumentList "`"$issFile`"" -Wait -PassThru -NoNewWindow
$elapsed = (Get-Date) - $startTime
$elapsedMin = [math]::Round($elapsed.TotalMinutes, 1)

if ($proc.ExitCode -ne 0) {
    Fail "Inno Setup compilation failed (exit code $($proc.ExitCode))."
}

Write-Host "  Compilation completed in $elapsedMin minutes." -ForegroundColor Green

Write-Host ""
Write-Host "--- Verifying Disk-Spanned Output ---"
$finalExe = Require-File $outputExe "Full offline setup EXE"
$finalBin = Require-File $outputBin "Full offline setup BIN"
$finalBins = Get-ChildItem -LiteralPath $offlineOutputDir -Filter "$outputBaseName-*.bin" -File | Sort-Object Name
if ($finalBins.Count -lt 1) {
    Fail "No disk-spanned .bin files were produced in $offlineOutputDir"
}
if ($finalExe.Name -match "0\.2\.0|0\.2\.1" -or ($finalBins | Where-Object { $_.Name -match "0\.2\.0|0\.2\.1" })) {
    Fail "Final output filename contains stale version."
}

$finalExeSizeMB = [math]::Round($finalExe.Length / 1MB, 1)
$finalBinSizeGB = [math]::Round(($finalBins | Measure-Object -Property Length -Sum).Sum / 1GB, 2)
Write-Host "  EXE: $($finalExe.FullName) ($finalExeSizeMB MB)" -ForegroundColor Green
foreach ($bin in $finalBins) {
    Write-Host "  BIN: $($bin.FullName) ($([math]::Round($bin.Length / 1GB, 2)) GB)" -ForegroundColor Green
}

Write-Host ""
Write-Host "--- Generating SHA256 Checksums ---"
$checksumDir = Join-Path $releaseDir "checksums"
New-Item -ItemType Directory -Path $checksumDir -Force | Out-Null
$exeHash = (Get-FileHash -LiteralPath $finalExe.FullName -Algorithm SHA256).Hash
$binHashes = foreach ($bin in $finalBins) {
    [PSCustomObject]@{
        Name = $bin.Name
        FullName = $bin.FullName
        Length = $bin.Length
        Hash = (Get-FileHash -LiteralPath $bin.FullName -Algorithm SHA256).Hash
    }
}
$checksumFile = Join-Path $checksumDir "$outputBaseName.sha256.txt"
$checksumLines = @("$exeHash  $($finalExe.Name)") + ($binHashes | ForEach-Object { "$($_.Hash)  $($_.Name)" })
Set-Content -Path $checksumFile -Value $checksumLines -Encoding UTF8
Write-Host "  Written to: $checksumFile" -ForegroundColor Green

Write-Host ""
Write-Host "--- Writing Freshness Proof ---"
$freshnessFile = Join-Path $releaseDir "BUILD_FRESHNESS.txt"
$binFreshnessLines = ($binHashes | ForEach-Object {
    "- Data BIN: $($_.FullName)`n  size: $($_.Length) bytes`n  SHA256: $($_.Hash)"
}) -join "`n"
$freshness = @"
R2H PDF Fresh Build Proof
=========================
Build timestamp: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss zzz")
Package version: $releaseVersion
App/Tauri version: $appVersion

Source checks:
- PdfCanvasViewer decodeBase64ToBytes present: yes
- PdfCanvasViewer onRenderError present: yes
- PdfCanvasViewer Render exception present: yes
- PdfCanvasViewer finally present: yes
- WelcomeScreen present: yes
- UI polish present: yes
- icon present: yes
- icon source SHA256: $iconSourceHash
- icon tauri SHA256: $iconDestHash

Payload:
- NSIS payload path: $($nsisSetup.FullName)
- NSIS payload size: $($nsisSetup.Length) bytes
- NSIS payload timestamp: $($nsisSetup.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"))
- MSI payload path: $($msiSetup.FullName)
- MSI payload size: $($msiSetup.Length) bytes
- MSI payload timestamp: $($msiSetup.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"))

Full offline output:
- Folder: $offlineOutputDir
- Setup EXE: $($finalExe.FullName)
- Setup EXE size: $($finalExe.Length) bytes
- Setup EXE SHA256: $exeHash
$binFreshnessLines

Freshness:
- Latest source marker timestamp: $($latestSourceWrite.ToString("yyyy-MM-dd HH:mm:ss"))
- Release payload newer than source markers: yes
- Legacy app payload used: no
- Full offline wrapper registers as installed app: no
- local-ai included in staging: yes
"@
Set-Content -Path $freshnessFile -Value $freshness -Encoding UTF8
Write-Host "  Written to: $freshnessFile" -ForegroundColor Green

$buildStatusFile = Join-Path $releaseDir "full-offline-inno-build-status.md"
$buildStatus = @"
# Full Offline Installer Build Status

## Result: SUCCESS

- **Version:** $releaseVersion
- **Setup EXE:** $($finalExe.FullName)
- **Data BIN slices:** $($finalBins.Count)
- **EXE SHA256:** $exeHash
- **Built:** $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
- **Compiler:** Inno Setup 6
- **Disk spanning:** enabled
- **Build time:** $elapsedMin minutes
- **Staging size:** $stagingSizeGB GB

## Contents

- Fresh app installer ($($nsisSetup.Name))
- Fresh MSI payload ($($msiSetup.Name))
- Local AI models and runtimes
- Offline install and validation scripts
- Release documentation

## Install Behavior

Only the bundled Tauri app installer registers in Windows Installed Apps. The Inno wrapper is a bootstrapper with no app directory and no uninstall registry entry.
"@
Set-Content -Path $buildStatusFile -Value $buildStatus -Encoding UTF8

Write-Host ""
Write-Host "=== DONE ===" -ForegroundColor Green
Write-Host "  Setup EXE: $($finalExe.FullName)"
foreach ($bin in $finalBins) {
    Write-Host "  Data BIN: $($bin.FullName)"
}
Write-Host "  SHA256: $checksumFile"
Write-Host ""
Write-Host "Run only the Setup.exe and keep every .bin slice beside it." -ForegroundColor Cyan

exit 0
