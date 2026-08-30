# R2H PDF AI Workstation - Create Full Offline Installer
# Creates a single self-extracting EXE containing app + all AI models/runtimes.
# Requires: 7-Zip installed at C:\Program Files\7-Zip\7z.exe
#
# IMPORTANT:
# This script supports archives larger than 2GB.
# It uses disk-level binary concatenation via cmd.exe copy /b.
# Do NOT replace it with [System.IO.File]::ReadAllBytes($archivePath).

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$version = "0.2.0-beta"
$releaseDir = Join-Path $root "release\v$version"
$stagingDir = Join-Path $releaseDir "full-offline-staging"
$outputExe = Join-Path $releaseDir "R2H-PDF-v$version-Full-Offline-Setup.exe"
$archivePath = Join-Path $releaseDir "r2h-pdf-full-offline.7z"
$sevenZip = "C:\Program Files\7-Zip\7z.exe"

function Remove-UnsafeLocalAiAuxiliaryContent($localAiRoot) {
    # The DocLayout-YOLO source repository includes training/demo/download/hub
    # helpers with external URLs, telemetry, and cloud/API-key paths. Runtime OCR
    # uses the local PaddleOCR-VL worker and assets, so this auxiliary tree must
    # not be included in any offline package staging.
    $unsafePath = Join-Path $localAiRoot "models\layout\DocLayout-YOLO"
    if (Test-Path -LiteralPath $unsafePath) {
        $resolvedRoot = (Resolve-Path -LiteralPath $localAiRoot).Path
        $resolvedUnsafe = (Resolve-Path -LiteralPath $unsafePath).Path
        if (-not $resolvedUnsafe.StartsWith($resolvedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "Refusing to remove path outside staged local-ai: $resolvedUnsafe"
        }
        Write-Host "  Removing unsafe auxiliary local-ai content: models\layout\DocLayout-YOLO" -ForegroundColor Yellow
        Remove-Item -LiteralPath $resolvedUnsafe -Recurse -Force
    }
}

Write-Host "=== R2H PDF - Full Offline Installer Builder ===" -ForegroundColor Cyan
Write-Host "Version: $version"
Write-Host "Staging: $stagingDir"
Write-Host "Output: $outputExe"

# Verify 7-Zip
if (-not (Test-Path $sevenZip)) {
    Write-Host "ERROR: 7-Zip not found at $sevenZip" -ForegroundColor Red
    Write-Host "Install 7-Zip from https://www.7-zip.org/ and retry." -ForegroundColor Yellow
    exit 1
}

# Verify app installers
$nsisSetup = Join-Path $releaseDir "app\r2h-pdf_0.2.0_x64-setup.exe"
$msiSetup = Join-Path $releaseDir "app\r2h-pdf_0.2.0_x64_en-US.msi"

if (-not (Test-Path $nsisSetup)) {
    Write-Host "ERROR: NSIS setup not found: $nsisSetup" -ForegroundColor Red
    exit 1
}

# Verify local-ai
$localAi = Join-Path $root "local-ai"
if (-not (Test-Path $localAi)) {
    Write-Host "ERROR: local-ai folder not found: $localAi" -ForegroundColor Red
    exit 1
}

# Clean and create staging
Write-Host "`n--- Creating Staging Directory ---"
if (Test-Path $stagingDir) {
    Remove-Item $stagingDir -Recurse -Force
}
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

# 1. Copy app installers
Write-Host "  Copying app installers..."
$appDir = Join-Path $stagingDir "install-app"
New-Item -ItemType Directory -Path $appDir -Force | Out-Null
Copy-Item $nsisSetup $appDir -Force
if (Test-Path $msiSetup) {
    Copy-Item $msiSetup $appDir -Force
}

# 2. Copy local-ai
Write-Host "  Copying local-ai folder (this may take several minutes)..."
$destLocalAi = Join-Path $stagingDir "local-ai"
Copy-Item $localAi $destLocalAi -Recurse -Force
Remove-UnsafeLocalAiAuxiliaryContent $destLocalAi
Write-Host "  local-ai copied."

# 3. Copy scripts
Write-Host "  Copying scripts..."
$scriptsDir = Join-Path $stagingDir "scripts"
New-Item -ItemType Directory -Path $scriptsDir -Force | Out-Null

$validateLocalAi = Join-Path $root "scripts\validate-local-ai.ps1"
if (Test-Path $validateLocalAi) {
    Copy-Item $validateLocalAi $scriptsDir -Force
} else {
    Write-Host "  WARNING: validate-local-ai.ps1 not found. Validation will be skipped in installer." -ForegroundColor Yellow
}

# 4. Copy docs
Write-Host "  Copying docs..."
$docsDir = Join-Path $stagingDir "docs"
New-Item -ItemType Directory -Path $docsDir -Force | Out-Null

$docFiles = @(
    "RELEASE_NOTES_v0.2.0-beta.md",
    "KNOWN_LIMITATIONS.md",
    "CLEAN_MACHINE_VALIDATION.md",
    "PACKAGING_STRATEGY.md",
    "OFFLINE_ENFORCEMENT.md",
    "RELEASE_READINESS_REPORT.md"
)

foreach ($d in $docFiles) {
    $src = Join-Path $root "docs\release\$d"
    if (Test-Path $src) {
        Copy-Item $src $docsDir -Force
    }
}

# 5. Generate install script
Write-Host "  Generating install-full-offline.ps1..."
$installScript = @'
# R2H PDF AI Workstation - Full Offline Install Script
# This script is launched by the full offline installer.

$ErrorActionPreference = "Stop"

$packageRoot = $PSScriptRoot
$appData = Join-Path $env:LOCALAPPDATA "R2H-PDF"
$logFile = Join-Path $appData "install-full-offline.log"

New-Item -ItemType Directory -Path $appData -Force | Out-Null

function Log($msg) {
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts $msg"
    Write-Host $line
    Add-Content -Path $logFile -Value $line
}

try {
    Log "Install started."
    Log "Package root: $packageRoot"
    Log "Target app data: $appData"

    # Copy local-ai
    $srcLocalAi = Join-Path $packageRoot "local-ai"
    $destLocalAi = Join-Path $appData "local-ai"

    if (-not (Test-Path $srcLocalAi)) {
        throw "Bundled local-ai folder not found: $srcLocalAi"
    }

    if (Test-Path $destLocalAi) {
        Log "Existing local-ai found at $destLocalAi. Removing old folder."
        Remove-Item $destLocalAi -Recurse -Force
    }

    Log "Copying local-ai assets. This may take several minutes."
    Copy-Item $srcLocalAi $destLocalAi -Recurse -Force
    Log "local-ai copied successfully."

    # Set environment variable
    [System.Environment]::SetEnvironmentVariable("R2H_LOCAL_AI_ROOT", $destLocalAi, "User")
    $env:R2H_LOCAL_AI_ROOT = $destLocalAi
    Log "Set R2H_LOCAL_AI_ROOT = $destLocalAi"

    # Install app
    $setup = Join-Path $packageRoot "install-app\r2h-pdf_0.2.0_x64-setup.exe"
    if (Test-Path $setup) {
        Log "Launching app installer: $setup"
        Start-Process -FilePath $setup -Wait
        Log "App installer completed."
    } else {
        Log "WARNING: App installer not found at $setup"
    }

    # Validate local AI
    $validateScript = Join-Path $packageRoot "scripts\validate-local-ai.ps1"
    if (Test-Path $validateScript) {
        Log "Running local AI validation."
        Push-Location $packageRoot
        try {
            powershell -ExecutionPolicy Bypass -File $validateScript
            if ($LASTEXITCODE -eq 0) {
                Log "Validation PASSED."
            } else {
                Log "Validation completed with issues. Exit code: $LASTEXITCODE"
            }
        }
        finally {
            Pop-Location
        }
    } else {
        Log "Validation script not found. Skipping."
    }

    Log "Installation completed successfully."
    Write-Host ""
    Write-Host "Installation complete. You can now launch R2H PDF AI Workstation." -ForegroundColor Green
    Write-Host "Local AI assets: $destLocalAi" -ForegroundColor Cyan
    Write-Host "Log: $logFile" -ForegroundColor DarkGray
    Read-Host "Press Enter to close"
    exit 0
}
catch {
    Log "FAILED: $($_.Exception.Message)"
    Write-Host ""
    Write-Host "Installation failed. See log:" -ForegroundColor Red
    Write-Host $logFile -ForegroundColor Yellow
    Read-Host "Press Enter to close"
    exit 1
}
'@

Set-Content -Path (Join-Path $stagingDir "install-full-offline.ps1") -Value $installScript -Encoding UTF8

# 6. Generate README
Write-Host "  Generating README_INSTALL.txt..."
$readme = @"
R2H PDF AI Workstation - Full Offline Installer
================================================
Version: $version

This package contains the complete application and all AI models/runtimes.
No internet connection is required.

INSTALLATION:
Run:
  R2H-PDF-v$version-Full-Offline-Setup.exe

The installer will:
1. Extract the full package.
2. Copy local-ai to:
   %LOCALAPPDATA%\R2H-PDF\local-ai
3. Set:
   R2H_LOCAL_AI_ROOT=%LOCALAPPDATA%\R2H-PDF\local-ai
4. Run the app installer.
5. Run local AI validation.

CONTENTS:
- install-app\     App installer (NSIS + MSI)
- local-ai\        AI models, runtimes, workers
- scripts\         Validation scripts
- docs\            Release documentation

SYSTEM REQUIREMENTS:
- Windows 10/11 x64
- 16 GB RAM minimum, 32 GB recommended
- 10 GB+ free disk space
- Python 3.10+ for OCR worker
- No internet required after install

NOTE:
This beta installer is not code-signed. Windows may show an Unknown Publisher warning.
"@

Set-Content -Path (Join-Path $stagingDir "README_INSTALL.txt") -Value $readme -Encoding UTF8

# 7. Generate SFX config
Write-Host "  Generating SFX config..."
$sfxConfig = @"
;!@Install@!UTF-8!
Title="R2H PDF AI Workstation v$version - Full Offline Setup"
BeginPrompt="Install R2H PDF AI Workstation with all offline AI models?"
RunProgram="powershell.exe -ExecutionPolicy Bypass -File \"%%T\\install-full-offline.ps1\""
;!@InstallEnd@!
"@

$sfxConfigPath = Join-Path $stagingDir "sfx-config.txt"

# 7-Zip SFX config should be UTF-8, preferably without BOM.
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($sfxConfigPath, $sfxConfig, $utf8NoBom)

# 8. Create 7z archive
Write-Host "`n--- Creating 7z Archive ---"
if (Test-Path $archivePath) {
    Remove-Item $archivePath -Force
}

Write-Host "  Compressing (this will take several minutes for 7+ GB)..."
& $sevenZip a -t7z -mx=3 -mmt=on $archivePath "$stagingDir\*" | Out-Null

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: 7z compression failed" -ForegroundColor Red
    exit 1
}

$archiveSize = (Get-Item $archivePath).Length
Write-Host "  Archive created: $([math]::Round($archiveSize / 1GB, 2)) GB"

# 9. Create SFX EXE
Write-Host "`n--- Creating SFX Installer ---"

$sfxModuleCandidates = @(
    "C:\Program Files\7-Zip\7zSD.sfx",
    "C:\Program Files\7-Zip\7z.sfx",
    "C:\Program Files (x86)\7-Zip\7zSD.sfx",
    "C:\Program Files (x86)\7-Zip\7z.sfx",
    (Join-Path $PSScriptRoot "7zSD.sfx"),
    (Join-Path $PSScriptRoot "7z.sfx"),
    (Join-Path $releaseDir "7zSD.sfx"),
    (Join-Path $releaseDir "7z.sfx")
)

$sfxModule = $sfxModuleCandidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $sfxModule) {
    Write-Host "WARNING: 7-Zip SFX module not found. Creating archive-only package." -ForegroundColor Yellow
    Write-Host "  Archive: $archivePath"
    Write-Host "  Users can extract with 7-Zip and run install-full-offline.ps1"
    exit 0
}

if (Test-Path $outputExe) {
    Remove-Item $outputExe -Force
}

# CRITICAL:
# Do not use [System.IO.File]::ReadAllBytes($archivePath).
# The archive is larger than 2GB. Use cmd.exe copy /b to concatenate on disk.
$concatCommand = "/c copy /b `"$sfxModule`" + `"$sfxConfigPath`" + `"$archivePath`" `"$outputExe`""

Write-Host "  SFX module: $sfxModule"
Write-Host "  Config:     $sfxConfigPath"
Write-Host "  Archive:    $archivePath"
Write-Host "  Output:     $outputExe"

$process = Start-Process `
    -FilePath "cmd.exe" `
    -ArgumentList $concatCommand `
    -Wait `
    -NoNewWindow `
    -PassThru

if ($process.ExitCode -ne 0) {
    throw "Failed to create SFX installer. cmd copy /b exit code: $($process.ExitCode)"
}

if (-not (Test-Path $outputExe)) {
    throw "SFX installer was not created: $outputExe"
}

$finalSize = (Get-Item $outputExe).Length
Write-Host "`n=== DONE ===" -ForegroundColor Green
Write-Host "  Output: $outputExe"
Write-Host "  Size: $([math]::Round($finalSize / 1GB, 2)) GB"
Write-Host "  This single file installs the app and all AI models offline."

# Cleanup archive after successful SFX creation.
Remove-Item $archivePath -Force -ErrorAction SilentlyContinue

exit 0
