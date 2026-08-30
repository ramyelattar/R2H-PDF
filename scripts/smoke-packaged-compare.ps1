powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "compare" `
    -ReportName "packaged-compare-smoke.md" `
    -OutputName "packaged-compare-report.md" `
    -ExpectedProof "Run packaged compare workflow or packaged hook and detect 185 mm2 to 240 mm2 change."
exit $LASTEXITCODE
