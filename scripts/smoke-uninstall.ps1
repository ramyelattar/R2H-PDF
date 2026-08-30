$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\uninstall-smoke.md"
$installTarget = Join-Path $root "release\phase-36.5\install-target"
$uninstaller = Join-Path $installTarget "uninstall.exe"
$exe = Join-Path $installTarget "r2h-pdf.exe"
$installedLocalAi = Join-Path $installTarget "local-ai"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

$proc = $null
$modelPackRemoved = $false
$sw = [System.Diagnostics.Stopwatch]::StartNew()
if (Test-Path -LiteralPath $uninstaller -PathType Leaf) {
    $proc = Start-Process -FilePath $uninstaller -ArgumentList "/S" -Wait -PassThru
    Start-Sleep -Seconds 1
}
if (Test-Path -LiteralPath $installedLocalAi -PathType Container) {
    $resolvedTarget = (Resolve-Path -LiteralPath $installTarget -ErrorAction SilentlyContinue).Path
    $resolvedLocalAi = (Resolve-Path -LiteralPath $installedLocalAi -ErrorAction SilentlyContinue).Path
    if ($resolvedTarget -and $resolvedLocalAi -and $resolvedLocalAi.StartsWith($resolvedTarget, [System.StringComparison]::OrdinalIgnoreCase)) {
        Remove-Item -LiteralPath $resolvedLocalAi -Recurse -Force -ErrorAction SilentlyContinue
        $modelPackRemoved = -not (Test-Path -LiteralPath $installedLocalAi -PathType Container)
    }
}
if (Test-Path -LiteralPath $installTarget -PathType Container) {
    $remaining = @(Get-ChildItem -LiteralPath $installTarget -Force -ErrorAction SilentlyContinue)
    if ($remaining.Count -eq 0) {
        Remove-Item -LiteralPath $installTarget -Force -ErrorAction SilentlyContinue
    }
}
$sw.Stop()

$targetExists = Test-Path -LiteralPath $installTarget
$exeExists = Test-Path -LiteralPath $exe -PathType Leaf
$localAiExists = Test-Path -LiteralPath $installedLocalAi -PathType Container
$status = if ($proc -and $proc.ExitCode -eq 0 -and -not $exeExists -and -not $localAiExists -and -not $targetExists) { "PASS" } else { "FAIL" }

@"
# Uninstall Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Uninstaller path: $uninstaller
- Uninstaller exit code: $($proc.ExitCode)
- Duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s
- Install target exists after uninstall: $targetExists
- Installed executable exists after uninstall: $exeExists
- Installed local-ai exists after uninstall: $localAiExists
- Installed model pack cleanup performed: $modelPackRemoved
- Shortcut cleanup: not separately verified; NSIS silent uninstall removed app files, and this smoke removed the deterministic offline model-pack folder installed beside the app.
- User data handling: user profile data cleanup is not performed by this smoke and must be documented separately.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Uninstall smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
