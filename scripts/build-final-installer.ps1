$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\phase-36.5-installer-proof.md"
$bundleRoot = Join-Path $root "src-tauri\target\release\bundle"
$nsisPath = Join-Path $bundleRoot "nsis\r2h-pdf_2.1.0_x64-setup.exe"
$msiPath = Join-Path $bundleRoot "msi\r2h-pdf_2.1.0_x64_en-US.msi"
$releaseAppDir = Join-Path $root "release\v2.1.0-beta\app"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

$sw = [System.Diagnostics.Stopwatch]::StartNew()
Push-Location $root
$output = pnpm tauri build 2>&1
$exitCode = $LASTEXITCODE
Pop-Location
$sw.Stop()

$installer = $null
if (Test-Path -LiteralPath $nsisPath -PathType Leaf) {
    $installer = Get-Item -LiteralPath $nsisPath
} elseif (Test-Path -LiteralPath $msiPath -PathType Leaf) {
    $installer = Get-Item -LiteralPath $msiPath
}

$status = "FAIL"
$notes = @()
if ($exitCode -eq 0 -and $installer -and $installer.Length -gt 0) {
    New-Item -ItemType Directory -Force -Path $releaseAppDir | Out-Null
    if (Test-Path -LiteralPath $nsisPath -PathType Leaf) {
        Copy-Item -LiteralPath $nsisPath -Destination $releaseAppDir -Force
    }
    if (Test-Path -LiteralPath $msiPath -PathType Leaf) {
        Copy-Item -LiteralPath $msiPath -Destination $releaseAppDir -Force
    }
    $status = "PASS"
    $notes += "Final installer artifact exists and is non-empty."
    $notes += "Fresh Tauri MSI/NSIS app payloads were copied to release\v2.1.0-beta\app for freshness validation."
} else {
    $notes += "Installer build command failed or no non-empty installer artifact was found."
}

@"
# Phase 36.5 Installer Proof

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Build command: pnpm tauri build
- Build exit code: $exitCode
- Build duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s
- Installer path: $($installer.FullName)
- Installer type: $(if ($installer -and $installer.Extension -eq ".msi") { "MSI" } elseif ($installer) { "NSIS setup EXE" } else { "not found" })
- Installer size: $(if ($installer) { $installer.Length } else { 0 })
- Installer timestamp: $(if ($installer) { $installer.LastWriteTime } else { "" })
- MSI path: $msiPath
- NSIS path: $nsisPath
- Release app payload directory: $releaseAppDir
- Expected product name: r2h-pdf
- Expected version: 2.1.0 / 2.1.0-beta package metadata
- Notes: $($notes -join " ")

## Build Output

``````
$($output -join "`n")
``````
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Installer proof: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
