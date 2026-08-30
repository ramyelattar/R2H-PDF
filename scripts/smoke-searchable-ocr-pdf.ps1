$root = Split-Path -Parent $PSScriptRoot
$reportPath = Join-Path $root "release\smoke-results\searchable-ocr-pdf-smoke.md"
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"
New-Item -ItemType Directory -Force -Path (Split-Path $reportPath) | Out-Null

@"
# Searchable OCR PDF Smoke

- Timestamp: $timestamp
- Project root: $root
- Status: PASS_WITH_LIMITATION
- Claim status: NOT CLAIMED
- Input: `demo\input\scanned-ocr-sample.pdf`
- Output PDF: not generated
- Extraction method: not run because true searchable OCR PDF export is not implemented
- Extracted text: none
- Reason: `pdf_create_ocr_text_layer` intentionally reports not_implemented; faking invisible text or sidecar OCR would be dishonest.
- Product action: searchable OCR PDF export must not be claimed until exported PDF text extraction proves the OCR text exists in the PDF.
"@ | Out-File -LiteralPath $reportPath -Encoding utf8

Write-Host "Searchable OCR PDF smoke: PASS_WITH_LIMITATION - not claimed ($reportPath)"
exit 4
