$ErrorActionPreference = "Stop"

param(
    [string]$OutputPath = "",
    [int]$ValidDays = 365
)

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $root "release\smoke-results\generated-test-license.json"
}

New-Item -ItemType Directory -Force -Path (Split-Path $OutputPath) | Out-Null
Push-Location (Join-Path $root "src-tauri")
cargo build --release -q
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    throw "cargo build failed while preparing test-license generator"
}
Pop-Location

$licenseDir = Split-Path $OutputPath
$env:R2H_LICENSE_DIR = $licenseDir
$env:R2H_LICENSE_SMOKE_OUTPUT = (Join-Path $licenseDir "license-generation-smoke.json")
$exe = Join-Path $root "src-tauri\target\release\r2h-pdf.exe"
& $exe --license-smoke offline | Out-Null
if ($LASTEXITCODE -ne 0) {
    Remove-Item Env:\R2H_LICENSE_DIR -ErrorAction SilentlyContinue
    Remove-Item Env:\R2H_LICENSE_SMOKE_OUTPUT -ErrorAction SilentlyContinue
    throw "license smoke generation failed"
}
Remove-Item Env:\R2H_LICENSE_DIR -ErrorAction SilentlyContinue
Remove-Item Env:\R2H_LICENSE_SMOKE_OUTPUT -ErrorAction SilentlyContinue

$generated = Join-Path $licenseDir "license.json"
if (-not (Test-Path -LiteralPath $generated -PathType Leaf)) {
    throw "expected generated license not found: $generated"
}
Copy-Item -LiteralPath $generated -Destination $OutputPath -Force
Write-Host "Generated local test license: $OutputPath"
