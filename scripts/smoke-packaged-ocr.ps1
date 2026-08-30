powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "OCR" `
    -ReportName "packaged-ocr-smoke.md" `
    -OutputName "packaged-ocr-output.txt" `
    -ExpectedProof "Run OCR through packaged app or packaged OCR hook and verify output contains R2H OCR TEST 123."
exit $LASTEXITCODE
