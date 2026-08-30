$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_ocr" `
    -ReportPath (Join-Path $root "release\smoke-results\ocr-smoke.md") `
    -Ignored
exit $LASTEXITCODE
