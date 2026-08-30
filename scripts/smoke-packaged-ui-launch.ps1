$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "release"
$smokeDir = Join-Path $root "release\smoke-results"
$reportPath = Join-Path $smokeDir "packaged-ui-launch-smoke.md"
$rootReportPath = Join-Path $releaseDir "packaged-ui-launch-smoke.md"
$screenshotPath = Join-Path $smokeDir "packaged-ui-launch.png"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path $smokeDir | Out-Null

function Find-PackagedAppExe {
    $candidates = New-Object System.Collections.Generic.List[object]
    $searchRoots = @(
        (Join-Path $root "src-tauri\target\release"),
        (Join-Path $root "release\v2.1.0-beta"),
        (Join-Path $env:LOCALAPPDATA "Programs"),
        $env:ProgramFiles,
        ${env:ProgramFiles(x86)}
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) }

    foreach ($searchRoot in $searchRoots) {
        Get-ChildItem -LiteralPath $searchRoot -Recurse -File -Filter "*.exe" -ErrorAction SilentlyContinue |
            Where-Object {
                $_.FullName -match 'r2h|R2H' -and
                $_.Name -notmatch 'setup|installer|install' -and
                $_.FullName -notmatch 'python-runtime|llama-cpp|runtimes'
            } |
            ForEach-Object { $candidates.Add($_) | Out-Null }
    }

    return $candidates | Select-Object -First 1
}

function Write-Report {
    param(
        [string]$Status,
        [string]$Reason,
        [string]$ExePath = "",
        [bool]$WindowDetected = $false,
        [string]$ScreenshotStatus = "NOT IMPLEMENTED"
    )

@"
# Packaged UI Launch Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $Status
- Packaged/installed app executable: $ExePath
- Executable classification: $(if ($ExePath) { "app executable" } else { "not found" })
- Window detected: $WindowDetected
- Screenshot path: $screenshotPath
- Screenshot status: $ScreenshotStatus
- Reason: $Reason
- Limitation: Setup installers are not treated as packaged app UI launch proof.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8
    Copy-Item -LiteralPath $reportPath -Destination $rootReportPath -Force
}

$exe = Find-PackagedAppExe
if (-not $exe) {
    Write-Report -Status "NOT_IMPLEMENTED" -Reason "No installed or unpacked R2H-PDF application executable was found; release artifacts currently expose setup installers, not a directly launchable app exe."
    Write-Host "Packaged UI launch smoke NOT_IMPLEMENTED: no app executable found."
    exit 2
}

$proc = $null
try {
    $proc = Start-Process -FilePath $exe.FullName -PassThru -WindowStyle Normal
    $deadline = (Get-Date).AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 500
        $proc.Refresh()
    } while ($proc.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline -and -not $proc.HasExited)

    if ($proc.HasExited -or $proc.MainWindowHandle -eq 0) {
        Write-Report -Status "FAIL" -Reason "Process launched but no main window was detected within 30 seconds." -ExePath $exe.FullName
        exit 1
    }

    Write-Report -Status "PASS" -Reason "Packaged/installed app executable launched and a main window handle was detected. Process id: $($proc.Id)." -ExePath $exe.FullName -WindowDetected $true -ScreenshotStatus "NOT IMPLEMENTED - PowerShell window screenshot capture is not wired in this smoke."
    exit 0
} finally {
    if ($proc -and -not $proc.HasExited) {
        $proc.CloseMainWindow() | Out-Null
        Start-Sleep -Seconds 2
        if (-not $proc.HasExited) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        }
    }
}
