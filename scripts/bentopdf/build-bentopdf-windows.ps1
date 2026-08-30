[CmdletBinding()]
param(
    [string]$ProjectRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$submodule = Join-Path $ProjectRoot "thirdparty\bentopdf"
$node = (Get-Command node.exe -ErrorAction Stop).Source
$npm = (Get-Command npm.cmd -ErrorAction Stop).Source
$tsc = Join-Path $submodule "node_modules\.bin\tsc.cmd"
$vite = Join-Path $submodule "node_modules\.bin\vite.cmd"

function Run {
    param([string]$File, [string[]]$Args = @())
    & $File @Args
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code $LASTEXITCODE`: $File $($Args -join ' ')"
    }
}

$env:SIMPLE_MODE = "true"
$env:BASE_URL = "/bentopdf/"
$env:VITE_USE_CDN = "false"
$env:VITE_DEFAULT_LANGUAGE = "ar"
$env:VITE_APP_NAME = "R2H PDF"
$env:VITE_BRAND_NAME = "R2H PDF"
$env:NODE_OPTIONS = "--max-old-space-size=3072"

Push-Location $submodule
try {
    Run $npm @("ci")

    $env:SITE_URL = "https://www.bentopdf.com"
    Run $tsc
    Run $vite @("build")
    Run $node @("scripts/generate-i18n-pages.mjs")

    $env:SITE_URL = "https://www.bentopdf.com/bentopdf"
    Run $node @("scripts/generate-sitemap.mjs")
    Run $node @("scripts/generate-security-headers.mjs")

    git restore --source=HEAD --staged --worktree -- security-headers-docs.conf
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to restore generated security-headers-docs.conf."
    }

    Run $node @("scripts/seo-audit.mjs")
}
finally {
    Pop-Location
}

Write-Host "BENTOPDF WINDOWS BUILD PASSED" -ForegroundColor Green
