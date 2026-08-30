$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_compare" `
    -ReportPath (Join-Path $root "release\smoke-results\compare-smoke.md") `
    -RequiredOutputs @((Join-Path $root "demo\output\compare-report.md"))
exit $LASTEXITCODE
