# R2H PDF AI Workstation - Version Check
$root = Split-Path -Parent $PSScriptRoot

Write-Host "=== R2H PDF - Version Check ===" -ForegroundColor Cyan

$pkg = Get-Content (Join-Path $root "package.json") | ConvertFrom-Json
Write-Host "  package.json: $($pkg.version)"

$tauri = Get-Content (Join-Path $root "src-tauri/tauri.conf.json") | ConvertFrom-Json
Write-Host "  tauri.conf.json: $($tauri.version)"
Write-Host "  productName: $($tauri.productName)"

$cargo = Get-Content (Join-Path $root "src-tauri/Cargo.toml") -Raw
if ($cargo -match 'version\s*=\s*"([^"]+)"') { Write-Host "  Cargo.toml: $($Matches[1])" }

# Tauri/MSI requires semver without pre-release suffix.
# package.json may have "2.1.0-beta" while tauri.conf.json has "2.1.0".
$pkgBase = $pkg.version -replace '-.*$', ''
if ($pkgBase -eq $tauri.version) {
    Write-Host ""
    Write-Host "  Versions MATCH (base: $pkgBase)" -ForegroundColor Green
    if ($pkg.version -ne $tauri.version) {
        Write-Host "  Note: package.json has pre-release suffix ($($pkg.version))" -ForegroundColor DarkGray
    }
} else {
    Write-Host ""
    Write-Host "  Versions MISMATCH (package.json base=$pkgBase, tauri=$($tauri.version))" -ForegroundColor Red
    exit 1
}
exit 0
