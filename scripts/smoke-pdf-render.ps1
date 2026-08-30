$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_render" `
    -ReportPath (Join-Path $root "release\smoke-results\render-smoke.md") `
    -RequiredOutputs @((Join-Path $root "demo\output\render-smoke.ppm"))
exit $LASTEXITCODE
