powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "render" `
    -ReportName "packaged-render-smoke.md" `
    -OutputName "packaged-render-output.ppm" `
    -ExpectedProof "Launch packaged app or packaged backend hook, open demo\input\render-sample.pdf, and prove a rendered page."
exit $LASTEXITCODE
