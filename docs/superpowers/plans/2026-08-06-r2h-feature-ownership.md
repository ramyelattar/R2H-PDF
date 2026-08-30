# R2H Feature Ownership Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make R2H.AI-PDF the sole authoritative production owner and entry path for OCR, Compare, Forms, Page Organizer, Sign & Stamp, and Local Generation while preserving the existing feature internals and Phase 1/2 persistence paths.

**Architecture:** Add one production ownership registry containing the canonical R2H tab, command, panel, controller, typed IPC, and backend-command metadata for the six features. Use that registry to generate the active inspector tab definitions and command-palette registrations, then prove the App → RightInspector → panel/controller → IPC/backend graph with positive tests. BentoPDF remains an independent optional utility surface and is excluded from the six R2H feature routes.

**Tech Stack:** React 19, TypeScript, Tauri typed IPC, Vitest, Testing Library, PowerShell, JSON/Markdown/CSV evidence.

## Global Constraints

- R2H.AI-PDF is the canonical owner of OCR, Compare, Forms, Page Organizer, Sign & Stamp, and Local Generation.
- Do not modify OCR workers/models, compare algorithms, form behavior, page mutation algorithms, signature/stamp rendering, local AI runtime, persistence, save/export architecture, licensing, identity, paths, models, or MuPDF.
- Preserve all Phase 1 and Phase 2 changes and the existing canonical `project_save`, `doc_export`, and native transaction paths.
- Do not delete or refactor BentoPDF broadly; keep its optional independent window and shared backend code.
- Do not add dependencies, fixture data, synthetic feature routes, duplicate screens, or success fallbacks.
- Do not use Git or run broad native/MuPDF builds.

---

### Task 1: Capture the contract seam and pre-edit evidence

**Files:**
- Read: `audit-output/forensic-regression-root-cause/FORENSIC_REGRESSION_ROOT_CAUSE_REPORT.md`
- Read: `audit-output/forensic-regression-root-cause/shell_migration_analysis.md`
- Read: `audit-output/forensic-regression-root-cause/feature_root_cause_matrix.csv`
- Read: `audit-output/remediation-phase-2-reachability-real-data-truthful-state/PHASE_2_REMEDIATION_REPORT.md`
- Read: `src/App.tsx`, `src/components/shell/RightInspector.tsx`, `src/hooks/useMenuActions.ts`, `src/state/shellCatalog.ts`, `src/components/overlays/CommandPalette.tsx`

- [x] Record SHA-256 hashes for the source, ownership metadata, and documentation files that may change, without invoking Git.
- [x] Confirm the active graph already imports the six panels from `App.tsx`/`RightInspector.tsx` and that document-dependent commands use the shared `isBackendPdfSession` guard.
- [x] Keep runtime/native behavior classified as ownership/reachability evidence only.

### Task 2: Replace the stale retirement contract with failing positive tests

**Files:**
- Modify: `src/components/shell/duplicateUiRetirement.test.ts`
- Create: `src/hooks/useMenuActions.test.ts`

- [x] Replace absence assertions with tests that require six canonical R2H records, unique tabs/commands/panels/controllers, active App/Inspector imports, typed IPC/backend registration tokens, no synthetic feature route, no duplicate production panel, and the optional BentoPDF boundary.
- [x] Test that document-dependent inspector buttons are native-disabled and that the shared tab guard rejects an unguarded request without a PDF.
- [x] Test that the command palette renders the six commands and disables them without a document; test direct command execution is rejected before `requestInspectorTab` when no PDF is active.
- [x] Run the new focused tests before the production registry exists and confirm failures are caused by the missing current ownership contract, not test syntax.

### Task 3: Add the shared ownership registry and wire the production graph

**Files:**
- Create: `src/state/r2hFeatureOwnership.ts`
- Modify: `src/state/shellCatalog.ts`
- Modify: `src/components/shell/RightInspector.tsx`

- [x] Define one record for each feature with canonical owner `R2H.AI-PDF`, one inspector tab, one command ID, one panel, one controller/hook, typed IPC wrapper names, backend command names, `requiresDocument: true`, and `syntheticRoute: false`.
- [x] Generate the six command-palette records from the registry without changing command behavior or titles except for the current visual Sign & Stamp wording.
- [x] Generate the six inspector tab definitions from the registry while preserving the existing wrapping layout and `canAccessTab` guard.
- [x] Keep panel rendering in the existing `RightInspector` branches so the registry centralizes ownership metadata rather than introducing a second UI implementation.
- [x] Run the ownership, inspector, command-palette, and menu tests and inspect the actual diff for unrelated feature changes.

### Task 4: Correct the BentoPDF boundary and stale documentation

**Files:**
- Modify: `scripts/bentopdf/duplicate-ownership-overrides.json`
- Modify: `docs/superpowers/specs/2026-07-18-bentopdf-separate-window-consolidation-design.md`
- Modify: `docs/superpowers/plans/2026-07-18-bentopdf-duplicate-ownership-and-theme-baseline.md`
- Modify: `src/components/shell/WelcomeScreen.tsx`

- [x] Change only the six overlapping override records so their `finalOwner` is `r2h`, their retained R2H surface is explicit, and their rationale says BentoPDF is a separate optional surface rather than the canonical owner.
- [x] Mark the historical BentoPDF consolidation design and plan as superseded by the current R2H ownership decision, without deleting the retained historical record.
- [x] Update the WelcomeScreen ownership comment so the optional BentoPDF CTA cannot be mistaken for a feature route.
- [x] Run static checks proving no active R2H feature command/tab redirects to BentoPDF and no feature source uses `session://` as its route.

### Task 5: Verify the requested gates

**Files:**
- Read: the actual changed files and focused test output.

- [x] Run the exact focused suite requested by the product decision.
- [x] Run all directly affected menu, command-palette, inspector, ownership, and BentoPDF-boundary tests discovered in `src/`.
- [x] Run `pnpm typecheck` and `pnpm lint`; classify any failure as product, test, environment, or pre-existing with exact output.
- [x] Do not run Cargo/MuPDF/native builds or claim runtime feature behavior.

### Task 6: Produce the ownership evidence bundle

**Files:**
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/R2H_FEATURE_OWNERSHIP_REPORT.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/ownership_contract.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/production_entry_matrix.csv`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/obsolete_contract_changes.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/modified_files.csv`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/test_results.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/command_execution_log.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/remaining_runtime_requirements.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/evidence/production_graph.md`
- Create: `audit-output/remediation-phase-2a-r2h-feature-ownership/evidence/source_hashes.csv`

- [x] Record the exact six graph traces from R2H entry through panel/controller, typed IPC wrapper, and registered backend command.
- [x] Record duplicate-path count, synthetic-route status, no-document behavior, tests, and status for every feature.
- [x] Record changed-file SHA-256 values and command exit statuses; do not claim native/runtime success.
- [x] Use `R2H_OWNERSHIP_PASS` only if the requested focused tests, typecheck, and lint all exit successfully and the static ownership contract is satisfied; otherwise use the appropriate partial, fail, or blocked verdict.
