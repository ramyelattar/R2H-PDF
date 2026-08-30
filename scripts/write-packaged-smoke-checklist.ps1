param([string]$Workflow = "all")

$ErrorActionPreference = "Continue"
$root = Split-Path -Parent $PSScriptRoot
$releaseDir = Join-Path $root "release"
$smokeDir = Join-Path $releaseDir "smoke-results"
$checklistPath = Join-Path $smokeDir "PACKAGED-MANUAL-SMOKE-CHECKLIST.md"
$artifactRoot = Join-Path $releaseDir "v2.1.0-beta"
$exe = Get-ChildItem -LiteralPath $artifactRoot -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
$asar = Get-ChildItem -LiteralPath $artifactRoot -Recurse -Filter "app.asar" -ErrorAction SilentlyContinue | Select-Object -First 1
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss K"

New-Item -ItemType Directory -Force -Path $smokeDir | Out-Null

@"
# Packaged Manual Smoke Checklist

- Timestamp: $timestamp
- Project root: $root
- Workflow requested: $Workflow
- Status: MANUAL_CHECKLIST_ONLY
- Automation status: NOT IMPLEMENTED AUTOMATION
- Reason: Packaged UI automation is not implemented in Phase 36.3; this file is a manual checklist only.
- Packaged artifact root exists: $(Test-Path -LiteralPath $artifactRoot -PathType Container)
- Packaged structure check: $(if ($exe) { "PASS - executable found" } else { "NOT PROVEN - executable not found" })
- First executable found: $($exe.FullName)
- app.asar found: $($asar.FullName)

## Required Manual Checks

### 1. Launch packaged app
- Launch the packaged executable from the release artifact, not the dev server.
- Confirm the app opens without a console crash or missing-runtime dialog.

### 2. Open PDF
- Open `demo\input\render-sample.pdf`.
- Confirm the document tab/session is created.

### 3. Render page
- Confirm page 1 renders visible text: `R2H PDF Render Smoke Test`.

### 4. Text edit/export
- Open `demo\input\text-edit-sample.pdf`.
- Replace `OLD TEXT VALUE` with `NEW TEXT VALUE`.
- Export the PDF and verify the exported PDF opens.

### 5. Image edit/export
- Open `demo\input\image-edit-sample.pdf`.
- Perform one image operation: move, replace, crop, or delete.
- Export the PDF and verify the exported PDF opens.

### 6. OCR
- Open `demo\input\scanned-ocr-sample.pdf`.
- Run OCR.
- Verify recognized text includes `R2H OCR TEST 123` if OCR is available.
- Do not claim searchable OCR PDF export unless the exported PDF is actually searchable.

### 7. Ask PDF/RAG
- Open `demo\input\ask-pdf-sample.pdf`.
- Ask `What is the rating of MDB-01 and what feeder cable is mentioned?`.
- Verify answer includes `400A` and `4C x 240 mm2 Cu XLPE` with page/source evidence.

### 8. Compare
- Compare `demo\input\compare-a.pdf` and `demo\input\compare-b.pdf`.
- Verify the change from `185 mm2` to `240 mm2` is reported.

### 9. Export verification
- Confirm every exported file exists, is non-empty, opens in a PDF viewer, and reflects the operation claimed.

### 10. Close app cleanly
- Close the app and confirm no crash dialog, hung process, or corrupted settings state.

## Verdict

Packaged workflow proof: MANUAL CHECKLIST ONLY.
Automated packaged smoke: NOT IMPLEMENTED AUTOMATION.
Phase 36.3 note: packaged UI launch and individual packaged workflow smoke scripts now write separate NOT_IMPLEMENTED reports when no direct packaged app executable or packaged workflow hook is available.
"@ | Out-File -LiteralPath $checklistPath -Encoding utf8

Write-Host "Packaged smoke checklist written: $checklistPath"
exit 5
