$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_text_edit_export" `
    -ReportPath (Join-Path $root "release\smoke-results\text-edit-export-smoke.md") `
    -RequiredOutputs @((Join-Path $root "demo\output\text-edit-export.pdf"))
exit $LASTEXITCODE
