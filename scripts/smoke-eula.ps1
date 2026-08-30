$root = Split-Path -Parent $PSScriptRoot
$docsEula = Join-Path $root "docs\EULA.md"
$releaseEula = Join-Path $root "release\EULA.txt"
$reportPath = Join-Path $root "release\smoke-results\eula-smoke.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $docsEula), (Split-Path $releaseEula), (Split-Path $reportPath) | Out-Null

$required = @("license", "offline", "engineering", "professional", "warranty", "liability", "trial")
$docsText = if (Test-Path -LiteralPath $docsEula) { Get-Content -LiteralPath $docsEula -Raw } else { "" }
$releaseText = if (Test-Path -LiteralPath $releaseEula) { Get-Content -LiteralPath $releaseEula -Raw } else { "" }
$missing = @()
foreach ($term in $required) {
    if ($docsText -notmatch $term -or $releaseText -notmatch $term) {
        $missing += $term
    }
}
$integrated = (Get-Content -LiteralPath (Join-Path $root "src-tauri\tauri.conf.json") -Raw) -match "EULA|license"
$status = if ((Test-Path -LiteralPath $docsEula) -and (Test-Path -LiteralPath $releaseEula) -and $missing.Count -eq 0 -and $integrated) { "PASS" } else { "FAIL" }

@"
# EULA Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: $status
- EULA file: $docsEula
- Release EULA file: $releaseEula
- Docs EULA exists: $(Test-Path -LiteralPath $docsEula -PathType Leaf)
- Release EULA exists: $(Test-Path -LiteralPath $releaseEula -PathType Leaf)
- Required terms missing: $($missing -join ", ")
- Installer integration detected in Tauri config: $integrated
- In-app access: NOT PROVEN
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "EULA smoke: $status ($reportPath)"
if ($status -eq "PASS") { exit 0 }
exit 1
