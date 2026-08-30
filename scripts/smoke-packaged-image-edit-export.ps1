powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "image edit/export" `
    -ReportName "packaged-image-edit-export-smoke.md" `
    -OutputName "packaged-image-edit-export.pdf" `
    -ExpectedProof "Launch packaged app or packaged backend hook, perform an image edit, export PDF, and verify non-empty output plus render/pixel change where possible."
exit $LASTEXITCODE
