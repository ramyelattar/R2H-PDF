# Creates deterministic, synthetic PDF fixtures for release smoke tests.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$script = Join-Path $PSScriptRoot "create-demo-pdfs.cjs"

New-Item -ItemType Directory -Force -Path `
    (Join-Path $root "demo\input"), `
    (Join-Path $root "demo\output"), `
    (Join-Path $root "release\smoke-results"), `
    (Join-Path $root "release\logs") | Out-Null

node $script
if ($LASTEXITCODE -ne 0) {
    throw "Demo PDF generation failed with exit code $LASTEXITCODE"
}
