[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$buildScript = Join-Path $PSScriptRoot "build-bentopdf-windows.ps1"
if (-not (Test-Path -LiteralPath $buildScript -PathType Leaf)) {
    throw "Missing BentoPDF build script: $buildScript"
}

& powershell.exe -NoProfile -ExecutionPolicy Bypass -File $buildScript
if ($LASTEXITCODE -ne 0) {
    throw "BentoPDF bootstrap/build failed with exit code $LASTEXITCODE."
}

Write-Host "BENTOPDF SOURCE AND BUILD PHASE PASSED" -ForegroundColor Green
