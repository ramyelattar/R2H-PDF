$root = Split-Path -Parent $PSScriptRoot
powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "run-release-smoke-test.ps1") `
    -TestName "release_smoke_rag_ask_pdf" `
    -ReportPath (Join-Path $root "release\smoke-results\rag-ask-pdf-smoke.md") `
    -RequiredOutputs @((Join-Path $root "demo\output\ask-pdf-evidence-report.md"))
exit $LASTEXITCODE
