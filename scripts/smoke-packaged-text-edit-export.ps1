powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "text edit/export" `
    -ReportName "packaged-text-edit-export-smoke.md" `
    -OutputName "packaged-text-edit-export.pdf" `
    -ExpectedProof "Launch packaged app or packaged backend hook, replace OLD TEXT VALUE with NEW TEXT VALUE, export PDF, and verify non-empty output."
exit $LASTEXITCODE
