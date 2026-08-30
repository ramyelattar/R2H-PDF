# OCR Runtime Restoration and Acceptance

## Scope

Restore and verify only the R2H.AI-PDF OCR production boundary. Preserve the
Phase 1 project sidecar/save/export authority, the Phase 2 truthful session
guards, and the Phase 2A R2H ownership decision. Do not change Compare, Forms,
Page Organizer, Sign & Stamp, Local Generation, BentoPDF, application identity,
MuPDF identity, or any OCR model implementation beyond its worker contract.

The checked-in PaddleOCR-VL transformer path currently emits text but the
worker fabricates line-wide boxes and constant confidence values. The repair
must remove that false-success path. A result without model-supplied geometry
is a truthful failed/unsupported result and cannot be accepted as a durable
overlay.

## Phase 0: Evidence and scope guard

1. Record the no-Git repository state and SHA-256 hashes for every candidate
   production, test, worker, and documentation file before editing.
2. Record the exact graph from Inspector/command palette through `OcrPanel`,
   typed IPC, Tauri commands, native render, worker/model, overlay conversion,
   `project_save`, and `doc_export`.
3. Record the current external optional local-AI pack contract for source,
   debug, packaged, and installed resolution. Do not add model assets to the
   Tauri bundle or duplicate the multi-gigabyte model.
4. Record the current independent export validation and searchable-PDF
   `not_implemented` boundary. Correct misleading searchable-text comments
   without implementing a new text-layer engine.

## Phase 1: Red contract tests first

1. Add frontend contract tests for typed OCR availability/result mapping,
   operation states, active-session/page guards, duplicate-run suppression,
   disabled command non-execution, no-text status, malformed/invalid geometry
   rejection, and acceptance only after the save callback succeeds.
2. Add Rust unit tests for availability classifications, required worker/model
   assets, Python/runtime resolution, structured request fields, worker
   non-zero/empty/malformed/unsupported responses, finite in-bounds geometry,
   no-text classification, unique temporary paths and cleanup on failure,
   stale-session/page rejection, and truthful text-layer status.
3. Add standard-library worker tests for request validation and the result
   adapter. Assert that the production PaddleOCR-VL transformer route never
   inserts constant confidence or fabricated geometry.
4. Run the new focused tests before production edits and record the expected
   failures. Do not weaken any existing ownership or Phase 1/2 tests.

## Phase 2: Typed availability and resource resolution

1. Replace the boolean OCR availability command with a structured, serializable
   status using the required codes: `available`, `missing_worker`,
   `missing_python_runtime`, `missing_model`, `missing_processor`,
   `invalid_model_layout`, `unsupported_platform`, and
   `resource_resolution_failed`.
2. Resolve the optional OCR pack through the existing Tauri-safe local-AI
   resolver. Resolve a bundled or supported Python executable without relying
   on the current working directory or a developer absolute path. Validate the
   worker, model config/weights, processor/tokenizer assets, and required
   Python imports. Keep unrelated LLM/embedding assets out of OCR readiness.
3. Return actionable, path-safe user-facing messages and retain detailed
   resource paths only in bounded diagnostics/evidence. Do not claim OCR is
   included in the base package when `tauri.conf.json` does not package it.
4. Add typed frontend wrappers for availability, status, run, cache, and
   cancellation commands. Update `useOcr` to use those wrappers.

## Phase 3: Request, render, worker, and result boundary

1. Extend the OCR request with operation ID, live session ID, page index,
   language mode, validated model identifier, timeout, output schema version,
   and optional cancellation ID. Reject stale/closed sessions, invalid pages,
   and empty ranges before running the worker.
2. Build an OCR run context from the live native session, including the stable
   document content ID, page dimensions, page rotation, and render dimensions.
   Keep renderer tab IDs and synthetic routes out of this context.
3. Use an operation-isolated temporary image path with a cleanup guard that
   runs on success, failure, timeout, cancellation, and worker-start failure.
   Do not reuse session/page names as filenames. Keep the existing native page
   render as the only input source.
4. Make the worker launcher validate process start, stdout/stderr, exit code,
   timeout, cancellation, and schema before creating a cached result. Any
   non-zero exit is failure even when stdout contains JSON. Empty text is
   `no_text_detected`, not successful empty OCR.
5. Remove the production `R2H_OCR_SMOKE_ENGINE` override. Keep release smoke
   support explicitly isolated from the production engine, with no smoke or
   template output accepted as production OCR.
6. Define one typed result schema with operation/document/page identity,
   page dimensions/rotation, text blocks, optional model-supplied confidence,
   worker/model version, duration, warnings, and status. Validate finite,
   positive, in-bounds geometry and reject missing geometry for overlay
   acceptance. Preserve reading order only when supplied by the engine.
7. Treat the current PaddleOCR-VL transformer text-only output honestly. Do
   not parse lines into full-page boxes. Until an existing real geometry
   producer is connected, return a structured unsupported-geometry failure and
   keep acceptance/export disabled for that result.

## Phase 4: Truthful UI and canonical persistence

1. Expand OCR operation state to the existing truthful states plus
   `no_text_detected`, with availability code/message, page/range, operation,
   worker/model state, real block count, error code, cancellation, and
   acceptance/persistence state.
2. Render only returned text and validated geometry; show confidence as
   unavailable when the model omits it. Never use timer progress or mark
   completed on process start.
3. Make OCR controls document-dependent and keyboard/command guarded. Prevent
   rapid duplicate invocation and ensure disabled commands do not execute.
4. Route acceptance through the existing canonical editor collection and
   `usePdfSave`/`project_save` path. Add stable OCR object IDs that do not embed
   a volatile session ID. On save failure, roll back the newly added objects,
   keep dirty state, and show failure/partial failure. Do not persist rejected
   or unvalidated blocks.
5. Preserve sidecar restore semantics. Add focused round-trip tests proving
   OCR metadata/text/geometry survive project serialization without changing
   project persistence authority.

## Phase 5: Export truthfulness and regression protection

1. Keep accepted OCR overlays on the existing `doc_export` visual annotation
   path and verify the selected overlay count is reported. Do not replace the
   temporary-output, fresh-MuPDF-reopen, page-count/hash, or atomic-replace
   flow.
2. Keep searchable PDF text-layer injection explicitly unavailable while it
   remains `not_implemented`. Remove wording that calls visual annotations a
   native searchable text layer. Add a test that no searchable-PDF claim is
   surfaced for that response.
3. Replace/update the obsolete OCR retirement assertions only where they
   contradict the current R2H ownership; no BentoPDF route or ownership
   change is part of this phase.

## Phase 6: Verification and evidence

1. Run the focused OCR/ownership/frontend suites, directly affected
   persistence/export/inspector tests, worker tests, and targeted Rust OCR
   tests. Run `pnpm typecheck`, `pnpm lint`, and the Vite production build.
2. Run the OCR worker `--version`, safe local model/processor probe, and a
   bounded real-model attempt against the checked-in PDF corpus. Record exact
   output, duration, and any timeout or missing-runtime boundary; never turn a
   timeout into PASS.
3. Run targeted native/resource/package validation where feasible. Do not run
   broad native or MuPDF builds. If packaged runtime, restart/reopen, or
   independent export evidence is unavailable, cap the verdict at
   `OCR_CODE_COMPLETE_RUNTIME_PENDING` or lower according to the remaining
   false-success paths.
4. Create `audit-output/remediation-phase-3-ocr-runtime/` with all requested
   contracts, matrices, hashes, command logs, tests, native/build/runtime
   evidence, manual owner procedure, and remaining risks. Ensure the report
   explicitly states that no later feature phase was started.

## Verification commands

Use the repository's existing package scripts and targeted commands only:

```powershell
pnpm exec vitest run src/features/ocr src/components/shell/RightInspector.test.tsx --pool=threads --no-file-parallelism
pnpm exec vitest run src/components/shell/duplicateUiRetirement.test.ts src/components/shell/RightInspector.test.tsx src/components/shell/WelcomeScreen.test.tsx --pool=threads --no-file-parallelism
pnpm typecheck
pnpm lint
pnpm build
python local-ai/workers/paddleocr_vl_worker.py --version
cargo test --manifest-path src-tauri/Cargo.toml document_core::ocr --lib
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
```

If a native command exceeds its bounded allowance, record it as inconclusive
with the exact command, elapsed time, and last compilation stage.

## Completion gate

Use `OCR_RUNTIME_PASS` only with fresh real worker/model, persistence
restart/reopen, visual export, independent reopen, and packaged evidence. Use
`OCR_CODE_COMPLETE_RUNTIME_PENDING` when the code contracts and targeted gates
pass but packaged/owner runtime proof is absent. Use `OCR_PARTIAL` whenever
the checked-in model still cannot provide validated geometry, a false-success
path remains, or a displayed capability is not implemented.
