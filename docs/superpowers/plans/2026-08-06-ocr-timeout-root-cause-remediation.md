# OCR Timeout Root-Cause and Remediation Plan

> **For agentic workers:** This plan is executed inline in the current task. Keep the OCR boundary isolated; do not begin Compare, Forms, Page Organizer, Sign & Stamp, Local Generation, or another feature phase.

**Goal:** Identify the primary cause of the real local OCR worker timeout, repair only that cause, and obtain one bounded real structured OCR result or report a measured blocker.

**Architecture:** Preserve the existing R2H.AI-PDF `OcrPanel`/`useOcr` -> typed IPC -> Rust `OcrEngine` -> local Python worker path. Add bounded stderr stage diagnostics and resource/process observations around the existing one-shot worker, then change only the component proven responsible. Keep final OCR JSON on stdout and all diagnostics on stderr.

**Tech Stack:** Python standard library/Pillow/Transformers/Torch worker, Rust/Tauri process supervision, React/TypeScript OCR controller, PowerShell, focused Vitest/unittest/cargo checks.

## Global Constraints

- Work only in `E:\Projects\R2H-PDF`; do not initialize Git, install dependencies, download models, or use cloud OCR.
- Reconcile the prior 24-file report against the 49-file review inventory before source edits; preserve all existing user changes.
- Do not increase the timeout as the primary repair or fabricate text, geometry, confidence, progress, or success.
- Preserve local model identity, Arabic/mixed-language support, canonical project persistence, visual export truthfulness, and the searchable-PDF `not_implemented` boundary.
- Every real probe records the exact command, arguments, working directory, environment policy, input dimensions/size, model path, stage timings, resource peak, exit state, stdout status, stderr summary, and process-tree cleanup.

---

### Task 1: Reconcile the baseline change surface

**Files:**
- Create: `audit-output/remediation-phase-3a-ocr-timeout/previous_file_change_reconciliation.csv`
- Create: `docs/superpowers/plans/2026-08-06-ocr-timeout-root-cause-remediation.md`

- [x] Inventory the 24 paths in `audit-output/remediation-phase-3-ocr-runtime/modified_files.csv`.
- [x] Inventory the 15 Phase-3 audit documents, 9 Phase-3 evidence files, and the external temporary OCR reviewer contract.
- [x] Record repository membership, classification, prior-CSV membership, purpose, OCR relevance, and unintended scope-expansion status.
- [ ] Recount after the timeout phase and report only newly modified production/test files separately from deliverables.

### Task 2: Trace and instrument the existing worker boundary

**Files:**
- Inspect first: `local-ai/workers/paddleocr_vl_worker.py`, `local-ai/workers/test_paddleocr_vl_worker.py`
- Inspect first: `src-tauri/src/document_core/ocr.rs`, `src-tauri/src/document_core/ipc.rs`, `src-tauri/src/lib.rs`
- Inspect first: `src/features/ocr/useOcr.ts`, `src/features/ocr/OcrPanel.tsx`, `src/lib/ipc.ts`, `src/App.tsx`, `src/components/shell/RightInspector.tsx`

- [ ] Extract the exact owner-probe request, executable, arguments, working directory, environment, image, model, timestamps, last output, exit state, child state, and partial-result state from existing evidence and source.
- [ ] Add one bounded stderr-only stage event at each required worker stage without logging page text.
- [ ] Add bounded parent/worker RSS, CPU, process-state, page-fault, elapsed, and stage samples during the real reproduction.
- [ ] Add a deterministic temporary worker fixture only in test scope for large stderr and final JSON protocol checks.

### Task 3: Red tests for the proven gap

**Files:**
- Modify only the focused worker/Rust tests needed by the measured failure.
- Keep production source unchanged until the new regression test fails for the intended reason.

- [ ] Run the minimal worker stage/protocol test and confirm the expected red result.
- [ ] Run the minimal Rust concurrent-drainage/timeout-cleanup test and confirm the expected red result if supervision is implicated.
- [ ] Add offline-only and local-path assertions only where the current implementation lacks the measured guard.

### Task 4: Reproduce and classify one variable at a time

- [ ] Run the exact real worker command outside the UI with the original page image.
- [ ] Determine the last completed stage and classify exactly one primary timeout type from the required enum.
- [ ] Run controlled input-size probes only after recording the current dimensions, file size, DPI/scale, rotation, and page type.
- [ ] Measure cold model load separately from warm inference when the worker can reach those stages.
- [ ] Verify no network/remote-code/cache lookup occurred and record any attempted lookup as a defect.
- [ ] Check child and descendant cleanup and capture partial stdout/stderr after timeout.

### Task 5: Implement one evidence-backed repair

- [ ] Write the smallest source change that addresses the confirmed primary cause; do not bundle speculative performance changes.
- [ ] Keep process cancellation/timeout tree cleanup complete and preserve bounded diagnostics.
- [ ] Keep the result protocol strict: one structured JSON value on stdout, no mixed logs, non-zero exit is failure, and no empty-text bypass.
- [ ] Re-run the red test green, then run the affected focused tests before broader targeted checks.

### Task 6: Verify and deliver the Phase 3A evidence package

**Files:**
- Create: `audit-output/remediation-phase-3a-ocr-timeout/` required report, CSV, evidence, and verification files from the user specification.

- [ ] Run worker unit/contract tests, Rust OCR supervision/parsing/geometry tests, frontend OCR tests, ownership regressions, TypeScript, lint, and Vite build.
- [ ] Run the real local model-load probe and one real single-page OCR probe with fresh resource/process evidence.
- [ ] Mark English/Arabic/mixed-language and application persistence/export levels independently; do not infer them from source or mocks.
- [ ] Set the verdict only from measured evidence: `OCR_TIMEOUT_FIXED`, `OCR_TIMEOUT_CODE_FIXED_RUNTIME_PENDING`, `OCR_TIMEOUT_PARTIAL`, `OCR_TIMEOUT_FAIL`, or `OCR_TIMEOUT_BLOCKED`.
- [ ] Print the required final console fields and end exactly with `OCR_TIMEOUT_ROOT_CAUSE_COMPLETE`.
