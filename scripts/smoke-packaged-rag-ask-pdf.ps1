powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "write-packaged-workflow-report.ps1") `
    -Workflow "Ask PDF/RAG" `
    -ReportName "packaged-rag-ask-pdf-smoke.md" `
    -OutputName "packaged-ask-pdf-evidence-report.md" `
    -ExpectedProof "Run packaged Ask PDF/RAG workflow or packaged hook and verify answer plus evidence for MDB-01 rating and feeder cable."
exit $LASTEXITCODE
