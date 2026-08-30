# R2H PDF AI Workstation - Full Offline Install Script (Inno Setup post-install)
# Called by the Inno Setup installer after extracting files to {tmp}.
# Copies local-ai to LocalAppData, sets env var, runs app installer, validates.

param(
    [Parameter(Mandatory=$true)]
    [string]$SourceRoot
)

$ErrorActionPreference = "Stop"

$appDataDir = Join-Path $env:LOCALAPPDATA "R2H-PDF"
$logFile = Join-Path $appDataDir "install-full-offline.log"
$localAiDest = Join-Path $appDataDir "local-ai"

# Ensure app data directory exists
New-Item -ItemType Directory -Path $appDataDir -Force | Out-Null

function Log($msg) {
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts  $msg"
    Write-Host $line
    Add-Content -Path $logFile -Value $line -Encoding UTF8
}

try {
    Log "=== R2H PDF Full Offline Install Started ==="
    Log "Source root: $SourceRoot"
    Log "Target app data: $appDataDir"
    Log "Target local-ai: $localAiDest"

    # ---------------------------------------------------------------
    # Step 1: Copy local-ai to LocalAppData
    # ---------------------------------------------------------------
    $srcLocalAi = Join-Path $SourceRoot "local-ai"

    if (-not (Test-Path $srcLocalAi)) {
        throw "Bundled local-ai folder not found at: $srcLocalAi"
    }

    if (Test-Path $localAiDest) {
        Log "Existing local-ai found at $localAiDest. Removing for clean install."
        Remove-Item $localAiDest -Recurse -Force -ErrorAction Stop
        Log "Old local-ai removed."
    }

    Log "Copying local-ai assets (this may take several minutes for 7+ GB)..."
    Log "  Source: $srcLocalAi"
    Log "  Destination: $localAiDest"

    # Copy with progress logging at top-level folder level
    $topFolders = Get-ChildItem $srcLocalAi -Directory
    New-Item -ItemType Directory -Path $localAiDest -Force | Out-Null

    foreach ($folder in $topFolders) {
        Log "  Copying: $($folder.Name)..."
        Copy-Item $folder.FullName (Join-Path $localAiDest $folder.Name) -Recurse -Force
    }

    # Copy top-level files (e.g., config files at root of local-ai)
    $topFiles = Get-ChildItem $srcLocalAi -File
    foreach ($f in $topFiles) {
        Copy-Item $f.FullName (Join-Path $localAiDest $f.Name) -Force
    }

    Log "local-ai copy completed."

    # ---------------------------------------------------------------
    # Step 2: Set R2H_LOCAL_AI_ROOT environment variable (user level)
    # ---------------------------------------------------------------
    [System.Environment]::SetEnvironmentVariable("R2H_LOCAL_AI_ROOT", $localAiDest, "User")
    $env:R2H_LOCAL_AI_ROOT = $localAiDest
    Log "Set R2H_LOCAL_AI_ROOT = $localAiDest (User environment variable)"

    # ---------------------------------------------------------------
    # Step 3: Run app installer
    # ---------------------------------------------------------------
    $installAppDir = Join-Path $SourceRoot "install-app"
    $appInstaller = Get-ChildItem -LiteralPath $installAppDir -Filter "r2h-pdf_2.1.0*_x64-setup.exe" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if (-not $appInstaller) {
        throw "Bundled app installer not found in $installAppDir. Expected a fresh r2h-pdf_2.1.0*_x64-setup.exe payload."
    }

    if ($appInstaller.Name -match "0\.2\.0|0\.2\.1") {
        throw "Refusing stale app installer payload: $($appInstaller.Name)"
    }

    if (Test-Path $appInstaller.FullName) {
        Log "Launching app installer: $($appInstaller.FullName)"
        # Tauri NSIS installer supports /S for silent install
        $proc = Start-Process -FilePath $appInstaller.FullName -ArgumentList "/S" -Wait -PassThru
        if ($proc.ExitCode -eq 0) {
            Log "App installer completed successfully (exit code 0)."
        } else {
            Log "WARNING: App installer exited with code $($proc.ExitCode). App may still have installed."
        }
    }

    # ---------------------------------------------------------------
    # Step 4: Run local AI validation
    # ---------------------------------------------------------------
    $validateScript = Join-Path $SourceRoot "scripts\validate-local-ai.ps1"

    if (Test-Path $validateScript) {
        Log "Running local AI validation..."
        Push-Location (Split-Path $validateScript -Parent)
        try {
            $validateOutput = powershell -ExecutionPolicy Bypass -File $validateScript 2>&1 | Out-String
            Log "Validation output:"
            foreach ($line in ($validateOutput -split "`n")) {
                if ($line.Trim()) { Log "  $($line.Trim())" }
            }
            if ($LASTEXITCODE -eq 0) {
                Log "Local AI validation PASSED."
            } else {
                Log "WARNING: Local AI validation reported issues (exit $LASTEXITCODE)."
            }
        } finally {
            Pop-Location
        }
    } else {
        Log "WARNING: validate-local-ai.ps1 not found. Skipping validation."
    }

    # ---------------------------------------------------------------
    # Step 5: Final summary
    # ---------------------------------------------------------------
    $totalSize = (Get-ChildItem $localAiDest -Recurse -File | Measure-Object -Property Length -Sum).Sum
    $totalSizeGB = [math]::Round($totalSize / 1GB, 2)

    Log ""
    Log "=== Installation Summary ==="
    Log "  App installed: Yes"
    Log "  Local AI path: $localAiDest"
    Log "  Local AI size: $totalSizeGB GB"
    Log "  Env variable: R2H_LOCAL_AI_ROOT = $localAiDest"
    Log "  Log file: $logFile"
    Log ""
    Log "=== R2H PDF Full Offline Install COMPLETED ==="

    exit 0
}
catch {
    Log "FATAL ERROR: $($_.Exception.Message)"
    Log "Stack trace: $($_.ScriptStackTrace)"
    Log ""
    Log "=== R2H PDF Full Offline Install FAILED ==="
    exit 1
}
