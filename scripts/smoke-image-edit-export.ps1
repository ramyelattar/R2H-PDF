$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_image_edit_export" `
    -ReportPath (Join-Path $root "release\smoke-results\image-edit-export-smoke.md") `
    -RequiredOutputs @((Join-Path $root "demo\output\image-edit-export.pdf"))
exit $LASTEXITCODE
