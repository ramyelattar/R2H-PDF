# Duplicate Ownership Matrix and Theme Baseline Implementation Plan

> **Superseded product decision (2026-08-06):** This retained historical plan
> records the former BentoPDF-canonical ownership contract. The active contract
> now makes R2H.AI-PDF canonical for OCR, Compare, Forms, Page Organizer,
> Sign & Stamp, and Local Generation; BentoPDF is a separate optional surface.
> Its historical examples and retirement steps must not be used to remove
> those R2H production routes.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce an authoritative, tested ownership matrix for every overlapping R2H/BentoPDF capability and establish the exact R2H visual-token baseline before any duplicate feature is deleted or BentoPDF styling is changed.

**Architecture:** BentoPDF remains the existing separate offline Tauri WebView. This plan performs only evidence generation, classification, and theme-baseline capture. It does not remove source code or change the BentoPDF interface.

**Tech Stack:** PowerShell 5.1, Node.js, TypeScript, React, Vitest, Rust, Cargo, Tauri 2, Astro/BentoPDF source inventory.

## Global Constraints

- Repository root: `E:\Projects\R2H-PDF`.
- Required branch: `feature/bentopdf-integration`.
- Required parent baseline: `542a03aae7b0ef9aa1a8cb7b8f8156533b8cc432`.
- Required BentoPDF revision: `21c924a3e6a7ce28740535a5bc6b74f872fcdcb5`.
- Keep BentoPDF as a separate reusable window.
- No source deletion in this plan.
- No user-facing behavior changes in this plan.
- No network access.
- Do not use prior reports as the sole authority; verify current source.
- No TODOs, placeholders, guessed ownership, or unverified deletion decisions.
- The final matrix must distinguish UI ownership from backend dependency ownership.

---

## Planned File Structure

### Create

- `docs/superpowers/specs/2026-07-18-bentopdf-separate-window-consolidation-design.md`  
  Approved architecture and constraints.

- `docs/superpowers/plans/2026-07-18-bentopdf-duplicate-ownership-and-theme-baseline.md`  
  This execution plan.

- `scripts/bentopdf/build-duplicate-ownership-matrix.mjs`  
  Reads live R2H and BentoPDF source and emits deterministic ownership evidence.

- `scripts/bentopdf/verify-duplicate-ownership-matrix.mjs`  
  Fails on ambiguous ownership, duplicate final ownership, missing evidence, or unknown deletion safety.

- `scripts/bentopdf/capture-r2h-theme-baseline.mjs`  
  Extracts current R2H CSS values and component-state evidence.

- `scripts/bentopdf/verify-r2h-theme-baseline.mjs`  
  Validates required token groups and source references.

- `scripts/bentopdf/duplicate-ownership-overrides.json`  
  Explicit human-reviewed classification overrides where source-name matching is insufficient.

- `src/features/bentopdf/ownership/ownershipTypes.ts`  
  Shared TypeScript types for matrix validation tests.

- `src/features/bentopdf/ownership/ownershipValidation.ts`  
  Pure validation functions.

- `src/features/bentopdf/ownership/ownershipValidation.test.ts`  
  Tests ownership invariants.

### Generated, ignored audit artifacts

- `audit-output/bentopdf-duplicate-ownership-<timestamp>/R2H_FEATURE_SURFACE.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/R2H_BACKEND_COMMANDS.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/BENTOPDF_TOOL_CATALOG.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/DUPLICATE_OWNERSHIP_MATRIX.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/DEPENDENCY_EDGES.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/DELETION_CANDIDATES.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/RETAINED_R2H_CAPABILITIES.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/R2H_THEME_BASELINE.json`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/R2H_THEME_SOURCE_REFERENCES.csv`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>/README_FIRST.md`
- `audit-output/bentopdf-duplicate-ownership-<timestamp>.zip`

---

### Task 1: Commit the Approved Design and Plan

**Files:**
- Create: `docs/superpowers/specs/2026-07-18-bentopdf-separate-window-consolidation-design.md`
- Create: `docs/superpowers/plans/2026-07-18-bentopdf-duplicate-ownership-and-theme-baseline.md`

**Interfaces:**
- Consumes: approved architecture from the discovery review.
- Produces: immutable design constraints used by all later tasks.

- [ ] **Step 1: Verify repository identity**

Run:

```powershell
Set-Location "E:\Projects\R2H-PDF"

$Branch = git branch --show-current
$Head = git rev-parse HEAD
$BentoHead = git -C ".\thirdparty\bentopdf" rev-parse HEAD
$Status = git status --short

if ($Branch -ne "feature/bentopdf-integration") {
    throw "Unexpected branch: $Branch"
}

if ($Head -ne "542a03aae7b0ef9aa1a8cb7b8f8156533b8cc432") {
    throw "Unexpected parent HEAD: $Head"
}

if ($BentoHead -ne "21c924a3e6a7ce28740535a5bc6b74f872fcdcb5") {
    throw "Unexpected BentoPDF HEAD: $BentoHead"
}

if ($Status) {
    throw "Working tree is not clean:`n$Status"
}
```

Expected: no output and no exception.

- [ ] **Step 2: Add the approved design and plan documents**

Copy the supplied documents to the exact paths listed above.

- [ ] **Step 3: Scan for incomplete language**

Run:

```powershell
$Docs = @(
    "docs/superpowers/specs/2026-07-18-bentopdf-separate-window-consolidation-design.md",
    "docs/superpowers/plans/2026-07-18-bentopdf-duplicate-ownership-and-theme-baseline.md"
)

$ForbiddenPattern = "\b" + "TODO" + "\b|\b" + "TBD" + "\b|implement later|fill in details"

$Forbidden = Select-String `
    -Path $Docs `
    -Pattern $ForbiddenPattern `
    -CaseSensitive:$false |
    Where-Object {
        $_.Line -notmatch "ForbiddenPattern"
    }

if ($Forbidden) {
    $Forbidden | Format-Table Path, LineNumber, Line -AutoSize
    throw "Incomplete plan language detected."
}
```

Expected: no matches.

- [ ] **Step 4: Commit the documentation gate**

```powershell
git add `
    "docs/superpowers/specs/2026-07-18-bentopdf-separate-window-consolidation-design.md" `
    "docs/superpowers/plans/2026-07-18-bentopdf-duplicate-ownership-and-theme-baseline.md"

git commit -m "docs(pdf): define BentoPDF ownership and theming architecture"
```

Expected: one commit containing only the two documents.

---

### Task 2: Define Ownership Contracts and Failing Tests

**Files:**
- Create: `src/features/bentopdf/ownership/ownershipTypes.ts`
- Create: `src/features/bentopdf/ownership/ownershipValidation.ts`
- Create: `src/features/bentopdf/ownership/ownershipValidation.test.ts`

**Interfaces:**
- Produces:
  - `type CapabilityOwner = "bentopdf" | "r2h"`
  - `type DeletionSafety = "ui-only-safe" | "backend-shared" | "backend-safe" | "blocked"`
  - `interface CapabilityOwnershipRecord`
  - `validateOwnershipMatrix(records): OwnershipValidationIssue[]`

- [ ] **Step 1: Write the failing tests**

The tests must cover:

```typescript
import { describe, expect, it } from "vitest";
import {
  type CapabilityOwnershipRecord,
  validateOwnershipMatrix,
} from "./ownershipValidation";

const validRecord: CapabilityOwnershipRecord = {
  capabilityId: "merge-pdf",
  displayName: "Merge PDF",
  finalOwner: "bentopdf",
  r2hUiPaths: ["src/components/shell/WelcomeScreen.tsx"],
  r2hBackendSymbols: ["doc_merge"],
  bentoToolPaths: ["src/pages/merge-pdf.astro"],
  retainedR2hConsumers: [],
  deletionSafety: "backend-safe",
  evidence: ["live-source"],
};

describe("validateOwnershipMatrix", () => {
  it("accepts a complete single-owner record", () => {
    expect(validateOwnershipMatrix([validRecord])).toEqual([]);
  });

  it("rejects missing Bento evidence for a Bento-owned capability", () => {
    const issues = validateOwnershipMatrix([
      { ...validRecord, bentoToolPaths: [] },
    ]);
    expect(issues.map((issue) => issue.code)).toContain(
      "BENTO_OWNER_WITHOUT_TOOL",
    );
  });

  it("rejects backend-safe when retained R2H consumers remain", () => {
    const issues = validateOwnershipMatrix([
      {
        ...validRecord,
        retainedR2hConsumers: ["src/features/export/usePdfExport.ts"],
        deletionSafety: "backend-safe",
      },
    ]);
    expect(issues.map((issue) => issue.code)).toContain(
      "BACKEND_SAFE_WITH_CONSUMERS",
    );
  });

  it("rejects duplicate capability identifiers", () => {
    const issues = validateOwnershipMatrix([validRecord, validRecord]);
    expect(issues.map((issue) => issue.code)).toContain(
      "DUPLICATE_CAPABILITY_ID",
    );
  });

  it("rejects records without source evidence", () => {
    const issues = validateOwnershipMatrix([
      { ...validRecord, evidence: [] },
    ]);
    expect(issues.map((issue) => issue.code)).toContain(
      "MISSING_EVIDENCE",
    );
  });
});
```

- [ ] **Step 2: Run the test and verify RED**

```powershell
pnpm vitest run `
    "src/features/bentopdf/ownership/ownershipValidation.test.ts" `
    --fileParallelism=false
```

Expected: FAIL because the module and exports do not exist.

- [ ] **Step 3: Implement the contracts and validator**

Implement the exact types and codes used by the tests. Validation must be pure, deterministic, and must not read files.

- [ ] **Step 4: Run the focused test**

```powershell
pnpm vitest run `
    "src/features/bentopdf/ownership/ownershipValidation.test.ts" `
    --fileParallelism=false
```

Expected: `5 passed`.

- [ ] **Step 5: Commit**

```powershell
git add "src/features/bentopdf/ownership"
git commit -m "test(pdf): define duplicate capability ownership contracts"
```

---

### Task 3: Build the Live R2H Feature and Backend Inventory

**Files:**
- Create: `scripts/bentopdf/build-duplicate-ownership-matrix.mjs`
- Create: `scripts/bentopdf/duplicate-ownership-overrides.json`

**Interfaces:**
- Consumes:
  - R2H `src/**/*.ts`, `src/**/*.tsx`, `src-tauri/src/**/*.rs`;
  - BentoPDF tracked source;
  - existing Tauri command registration in `src-tauri/src/lib.rs`.
- Produces:
  - `R2H_FEATURE_SURFACE.csv`
  - `R2H_BACKEND_COMMANDS.csv`
  - `DEPENDENCY_EDGES.csv`

- [ ] **Step 1: Add a script self-test mode**

The command:

```powershell
node ".\scripts\bentopdf\build-duplicate-ownership-matrix.mjs" --self-test
```

must exit `0` and print:

```text
OWNERSHIP_MATRIX_SELF_TEST_PASS
```

It must test CSV quoting, import normalization, Rust command extraction, and duplicate path handling.

- [ ] **Step 2: Run self-test before implementation**

Expected: non-zero because the script does not exist.

- [ ] **Step 3: Implement deterministic R2H inventory**

The script must inventory at minimum:

```text
src/components/shell
src/features/compare
src/features/export
src/features/forms
src/features/ocr
src/features/page-organizer
src/features/pdf-editor
src/features/sign-stamp
src/lib
src/state
src-tauri/src
```

For every discovered surface, emit:

```text
capabilityCandidate
path
symbol
surfaceType
tauriCommand
imports
importedBy
testPaths
sourceSha256
```

- [ ] **Step 4: Execute the inventory**

```powershell
$AuditRoot = Join-Path `
    "E:\Projects\R2H-PDF\audit-output" `
    ("bentopdf-duplicate-ownership-" + (Get-Date -Format "yyyyMMdd-HHmmss"))

node ".\scripts\bentopdf\build-duplicate-ownership-matrix.mjs" `
    --project-root "E:\Projects\R2H-PDF" `
    --bento-root "E:\Projects\R2H-PDF\thirdparty\bentopdf" `
    --audit-root "$AuditRoot" `
    --inventory-only

if ($LASTEXITCODE -ne 0) {
    throw "R2H inventory failed."
}
```

Expected: the three R2H inventory files exist and are non-empty.

- [ ] **Step 5: Verify known live surfaces are detected**

The script must fail unless it finds the current known candidates:

```text
compare
forms
ocr
page-organizer
sign-stamp
bentopdf window integration
```

- [ ] **Step 6: Commit**

```powershell
git add `
    "scripts/bentopdf/build-duplicate-ownership-matrix.mjs" `
    "scripts/bentopdf/duplicate-ownership-overrides.json"

git commit -m "feat(pdf): inventory R2H duplicate capability surfaces"
```

---

### Task 4: Build the BentoPDF Tool Catalog and Match Candidates

**Files:**
- Modify: `scripts/bentopdf/build-duplicate-ownership-matrix.mjs`
- Modify: `scripts/bentopdf/duplicate-ownership-overrides.json`

**Interfaces:**
- Produces:
  - `BENTOPDF_TOOL_CATALOG.csv`
  - initial `DUPLICATE_OWNERSHIP_MATRIX.csv`

- [ ] **Step 1: Add failing self-test fixtures**

Add in-memory fixtures proving that:

- `merge-pdf.astro` maps to `merge-pdf`;
- aliases such as page organizer actions remain separate operations;
- website-only pages are excluded;
- localized route copies do not become duplicate tools;
- one Bento tool may map to multiple R2H UI paths.

- [ ] **Step 2: Run self-test and verify RED**

Expected: FAIL on the new catalog assertions.

- [ ] **Step 3: Implement catalog extraction**

Read current BentoPDF source and emit:

```text
capabilityId
route
sourcePath
displayName
category
usesWorker
usesWasm
usesOcr
usesFileInput
createsDownload
localizedCopy
sourceSha256
```

Exclude:

```text
home
about
privacy
terms
blog
donation
marketing
SEO-only pages
localized duplicates
```

- [ ] **Step 4: Implement deterministic matching**

Matching order:

1. explicit override;
2. exact normalized capability identifier;
3. exact Tauri command/tool alias;
4. reviewed token similarity;
5. unmatched.

No fuzzy match may become authoritative without an override.

- [ ] **Step 5: Generate the matrix**

```powershell
node ".\scripts\bentopdf\build-duplicate-ownership-matrix.mjs" `
    --project-root "E:\Projects\R2H-PDF" `
    --bento-root "E:\Projects\R2H-PDF\thirdparty\bentopdf" `
    --audit-root "$AuditRoot"
```

Expected: every row contains one `finalOwner`; ambiguous rows are marked `blocked`, not guessed.

- [ ] **Step 6: Commit**

```powershell
git add `
    "scripts/bentopdf/build-duplicate-ownership-matrix.mjs" `
    "scripts/bentopdf/duplicate-ownership-overrides.json"

git commit -m "feat(pdf): map BentoPDF tools to R2H capability ownership"
```

---

### Task 5: Classify Safe UI and Backend Retirement

**Files:**
- Modify: `scripts/bentopdf/build-duplicate-ownership-matrix.mjs`
- Modify: `scripts/bentopdf/duplicate-ownership-overrides.json`

**Interfaces:**
- Produces:
  - `DELETION_CANDIDATES.csv`
  - `RETAINED_R2H_CAPABILITIES.csv`

- [ ] **Step 1: Add failing dependency-classification tests**

Fixtures must prove:

- a duplicate UI with a retained backend consumer becomes `ui-only-safe`;
- a backend with retained consumers becomes `backend-shared`;
- a backend with no consumers and a Bento replacement becomes `backend-safe`;
- unresolved dynamic invocation becomes `blocked`;
- R2H-only AI/engineering/RAG features remain `r2h`.

- [ ] **Step 2: Run self-test and verify RED**

Expected: FAIL on dependency classification.

- [ ] **Step 3: Implement dependency tracing**

Trace:

- TS/TSX static imports;
- barrel exports;
- Tauri `invoke()` command strings;
- Rust `invoke_handler` registration;
- Rust module references;
- tests;
- dynamic command names found in string constants.

A dynamic or unresolved edge must block backend deletion.

- [ ] **Step 4: Generate deletion classifications**

Required columns:

```text
capabilityId
finalOwner
r2hUiDeletion
r2hBackendDeletion
retainedConsumers
blockingReason
requiredTests
reviewStatus
```

`reviewStatus` must initially be `pending-human-review`.

- [ ] **Step 5: Verify no destructive action occurred**

```powershell
$AfterStatus = git status --short

$Unexpected = $AfterStatus |
    Where-Object {
        $_ -notmatch "^\?\? audit-output/"
    }

if ($Unexpected) {
    throw "Inventory changed tracked source unexpectedly:`n$Unexpected"
}
```

- [ ] **Step 6: Commit**

```powershell
git add `
    "scripts/bentopdf/build-duplicate-ownership-matrix.mjs" `
    "scripts/bentopdf/duplicate-ownership-overrides.json"

git commit -m "feat(pdf): classify safe duplicate retirement boundaries"
```

---

### Task 6: Capture the Exact R2H Theme Baseline

**Files:**
- Create: `scripts/bentopdf/capture-r2h-theme-baseline.mjs`
- Create: `scripts/bentopdf/verify-r2h-theme-baseline.mjs`

**Interfaces:**
- Produces:
  - `R2H_THEME_BASELINE.json`
  - `R2H_THEME_SOURCE_REFERENCES.csv`

- [ ] **Step 1: Add script self-test fixtures**

The self-test must validate extraction of:

- CSS custom properties;
- literal colors used when no custom property exists;
- gradients;
- border radius;
- shadows;
- font families;
- focus-visible states;
- hover, active, disabled, success, warning, and error states.

- [ ] **Step 2: Run self-test and verify RED**

Expected: non-zero because the scripts do not yet exist.

- [ ] **Step 3: Implement source extraction**

Read at minimum:

```text
src/App.css
src/components/shell
src/components/common
src/components/overlays
```

Emit each token with:

```text
tokenId
semanticRole
value
sourcePath
sourceLine
state
usageCount
```

Do not invent missing values. Mark missing semantic roles explicitly.

- [ ] **Step 4: Validate required semantic roles**

The verifier must require:

```text
appBackground
surfaceBackground
elevatedBackground
border
primaryText
secondaryText
mutedText
accent
accentHover
focus
success
warning
danger
shadow
smallRadius
mediumRadius
largeRadius
```

Failure output must identify the missing role and candidate source references.

- [ ] **Step 5: Run capture and verification**

```powershell
node ".\scripts\bentopdf\capture-r2h-theme-baseline.mjs" `
    --project-root "E:\Projects\R2H-PDF" `
    --audit-root "$AuditRoot"

node ".\scripts\bentopdf\verify-r2h-theme-baseline.mjs" `
    --baseline "$AuditRoot\R2H_THEME_BASELINE.json" `
    --references "$AuditRoot\R2H_THEME_SOURCE_REFERENCES.csv"
```

Expected:

```text
R2H_THEME_BASELINE_PASS
```

- [ ] **Step 6: Commit**

```powershell
git add `
    "scripts/bentopdf/capture-r2h-theme-baseline.mjs" `
    "scripts/bentopdf/verify-r2h-theme-baseline.mjs"

git commit -m "feat(ui): capture authoritative R2H visual theme baseline"
```

---

### Task 7: Add Matrix Verification and TypeScript Contract Tests

**Files:**
- Create: `scripts/bentopdf/verify-duplicate-ownership-matrix.mjs`
- Modify: `src/features/bentopdf/ownership/ownershipValidation.ts`
- Modify: `src/features/bentopdf/ownership/ownershipValidation.test.ts`

**Interfaces:**
- Consumes: generated matrix and deletion classifications.
- Produces: machine gate `DUPLICATE_OWNERSHIP_MATRIX_PASS`.

- [ ] **Step 1: Extend failing tests**

Add tests for:

- Bento-owned record retaining R2H UI deletion as false;
- backend-safe record with unknown dynamic edges;
- R2H-owned record with no retained R2H surface;
- duplicate final owners;
- missing human review state.

- [ ] **Step 2: Run focused tests and verify RED**

```powershell
pnpm vitest run `
    "src/features/bentopdf/ownership/ownershipValidation.test.ts" `
    --fileParallelism=false
```

- [ ] **Step 3: Implement validation rules**

The verifier must fail unless:

- each capability has exactly one owner;
- every Bento-owned capability has a live Bento source path;
- every R2H-owned capability has a retained R2H surface;
- no `backend-safe` row has retained or unresolved consumers;
- every deletion candidate contains required tests;
- every row is `approved` or explicitly `blocked`;
- there are no blank capability identifiers.

- [ ] **Step 4: Run script verification**

```powershell
node ".\scripts\bentopdf\verify-duplicate-ownership-matrix.mjs" `
    --matrix "$AuditRoot\DUPLICATE_OWNERSHIP_MATRIX.csv" `
    --deletions "$AuditRoot\DELETION_CANDIDATES.csv" `
    --retained "$AuditRoot\RETAINED_R2H_CAPABILITIES.csv"
```

Expected at the automated stage: FAIL because human review remains pending.

- [ ] **Step 5: Perform explicit row review**

Update only `scripts/bentopdf/duplicate-ownership-overrides.json`. Do not edit generated CSV files manually.

For every capability, record:

```json
{
  "capabilityId": "merge-pdf",
  "finalOwner": "bentopdf",
  "reviewStatus": "approved",
  "rationale": "BentoPDF contains the complete offline tool and R2H duplicate UI will be retired.",
  "approvedDeletionLevel": "ui-only"
}
```

Backend deletion approval must not be granted when retained consumers exist.

- [ ] **Step 6: Regenerate and re-run verification**

Expected:

```text
DUPLICATE_OWNERSHIP_MATRIX_PASS
```

- [ ] **Step 7: Run TypeScript tests**

```powershell
pnpm vitest run `
    "src/features/bentopdf/ownership/ownershipValidation.test.ts" `
    --fileParallelism=false
```

Expected: all ownership tests pass.

- [ ] **Step 8: Commit**

```powershell
git add `
    "scripts/bentopdf/verify-duplicate-ownership-matrix.mjs" `
    "scripts/bentopdf/duplicate-ownership-overrides.json" `
    "src/features/bentopdf/ownership"

git commit -m "test(pdf): enforce single-owner duplicate capability matrix"
```

---

### Task 8: Final Read-Only Certification and Evidence ZIP

**Files:**
- Modify: `scripts/bentopdf/build-duplicate-ownership-matrix.mjs`
- Modify: `scripts/bentopdf/capture-r2h-theme-baseline.mjs`

**Interfaces:**
- Produces: complete evidence ZIP for the next implementation plan.

- [ ] **Step 1: Run source gates**

```powershell
pnpm typecheck
if ($LASTEXITCODE -ne 0) { throw "Typecheck failed." }

pnpm vitest run --fileParallelism=false
if ($LASTEXITCODE -ne 0) { throw "Vitest failed." }

Push-Location ".\src-tauri"
try {
    cargo check --lib
    if ($LASTEXITCODE -ne 0) { throw "Cargo check failed." }
}
finally {
    Pop-Location
}
```

Expected:

- TypeScript PASS;
- all Vitest tests PASS;
- Cargo check PASS with no new errors.

- [ ] **Step 2: Rebuild final evidence from clean source**

Run the ownership builder and theme capture into a new final timestamped audit root.

- [ ] **Step 3: Verify source state**

```powershell
$TrackedStatus = git status --short --untracked-files=no

if ($TrackedStatus) {
    throw "Tracked source changed during final evidence generation:`n$TrackedStatus"
}
```

Expected: clean.

- [ ] **Step 4: Write `README_FIRST.md`**

It must state:

```text
Architecture: historical BentoPDF separate window; current R2H six-feature owner is R2H.AI-PDF
Duplicate owner rule: superseded for OCR, Compare, Forms, Page Organizer, Sign & Stamp, and Local Generation
R2H duplicate UI deletion: superseded; current R2H routes are retained
R2H duplicate backend deletion: not authorized by the current ownership decision
Theme modification: not started
Ownership matrix: approved
Theme baseline: complete
Source changes during audit: none
```

- [ ] **Step 5: Create the ZIP**

```powershell
$ZipPath = "$AuditRoot.zip"

Compress-Archive `
    -Path (Join-Path $AuditRoot "*") `
    -DestinationPath $ZipPath `
    -CompressionLevel Optimal `
    -Force

(Get-FileHash -LiteralPath $ZipPath -Algorithm SHA256).Hash
```

- [ ] **Step 6: Commit final planning infrastructure**

```powershell
git add `
    "scripts/bentopdf" `
    "src/features/bentopdf/ownership"

git commit -m "chore(pdf): certify duplicate ownership and theme baseline"
```

- [ ] **Step 7: Create rollback tag**

```powershell
git tag `
    -a "baseline-before-r2h-duplicate-ui-retirement" `
    -m "Approved ownership matrix and R2H theme baseline before duplicate UI retirement"

git show `
    --no-patch `
    --decorate `
    "baseline-before-r2h-duplicate-ui-retirement"
```

Expected: tag points to the final certification commit.

---

## Completion Gate

This plan is complete only when:

- every overlapping capability has one approved owner;
- all Bento-owned duplicate R2H UI surfaces are enumerated;
- all backend consumers are enumerated;
- no backend deletion is approved with retained or unresolved consumers;
- R2H-only capabilities are explicitly retained;
- the authoritative R2H visual-token baseline is complete;
- all source validation gates pass;
- the final evidence ZIP is created;
- the rollback tag exists;
- no duplicate UI or backend source has been deleted yet.

The next plan is:

```text
2026-07-18-r2h-duplicate-ui-retirement.md
```

It may start only after reviewing the generated matrix and approving every deletion row.

