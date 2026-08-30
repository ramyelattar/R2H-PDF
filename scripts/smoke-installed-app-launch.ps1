$ErrorActionPreference = "Continue"

$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\installed-app-launch-smoke.md"
$installTarget = Join-Path $root "release\phase-36.5\install-target"
$exe = Join-Path $installTarget "r2h-pdf.exe"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

$proc = $null
$windowDetected = $false
$crashed = $false
$sw = [System.Diagnostics.Stopwatch]::StartNew()
if (Test-Path -LiteralPath $exe -PathType Leaf) {
    $proc = Start-Process -FilePath $exe -PassThru
    Start-Sleep -Seconds 5
    $live = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue
    if ($live) {
        $live.Refresh()
        $windowDetected = ($live.MainWindowHandle -ne 0)
        $crashed = $live.HasExited
        $live.CloseMainWindow() | Out-Null
        Start-Sleep -Seconds 1
        if (-not $live.HasExited) {
            Stop-Process -Id $live.Id -Force -ErrorAction SilentlyContinue
        }
    } else {
        $crashed = $true
    }
}
$sw.Stop()

$status = if ($proc -and $windowDetected -and -not $crashed) { "PASS" } else { "FAIL" }

@"
# Installed App Launch Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- Installed executable: $exe
- Executable exists: $(Test-Path -LiteralPath $exe -PathType Leaf)
- Process id: $($proc.Id)
- Startup duration: $([math]::Round($sw.Elapsed.TotalSeconds, 2))s
- Main window detected: $windowDetected
- Immediate crash detected: $crashed
- Local resource check: release-smoke executable path exists; runtime resources are validated separately by local AI and packaged workflow gates.
- Screenshot status: NOT IMPLEMENTED - PowerShell screenshot capture is not wired in this smoke.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Installed app launch smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
