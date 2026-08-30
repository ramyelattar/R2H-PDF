# R2H-PDF + BentoPDF Separate-Window Consolidation Design

> **Superseded product decision (2026-08-06):** This retained historical design
> previously made BentoPDF canonical for overlapping PDF utilities. The current
> contract is `R2H.AI-PDF` as the canonical owner of OCR, Compare, Forms, Page
> Organizer, Sign & Stamp, and Local Generation. BentoPDF remains a separate
> optional surface and must not receive R2H feature navigation. See
> `audit-output/remediation-phase-2a-r2h-feature-ownership/ownership_contract.md`.

**Date:** 2026-07-18  
**Repository:** `E:\Projects\R2H-PDF`  
**Branch:** `feature/bentopdf-integration`  
**Baseline commit:** `542a03aae7b0ef9aa1a8cb7b8f8156533b8cc432`  
**Pinned BentoPDF commit:** `21c924a3e6a7ce28740535a5bc6b74f872fcdcb5`

## Historical Goal (Superseded)

Keep BentoPDF as the existing separate offline Tauri WebView window, remove duplicate PDF services from the R2H-PDF main application, and restyle the BentoPDF window to use the same visual identity as R2H-PDF.

## Historical Product Architecture (Superseded)

```text
R2H-PDF main window
├── R2H-only capabilities
├── AI/RAG/engineering/review capabilities
└── Open PDF Tools
    └── BentoPDF secondary window
        ├── complete BentoPDF tool catalog
        ├── pinned offline bundle
        ├── existing local HTTP runtime
        ├── WASM/workers/OCR assets
        └── R2H visual theme
```

The secondary-window ownership statement in this historical design is superseded. Current R2H feature navigation stays in the R2H production workflow; the BentoPDF window remains optional and independent.

## Non-Negotiable Constraints

- Keep one BentoPDF secondary window and reuse it when already open.
- Keep the current offline runtime and pinned BentoPDF source revision.
- No iframe.
- No external network access.
- Do not rewrite BentoPDF tools into R2H React/Rust during this project.
- Do not delete backend code until every consumer is proven absent.
- Do not remove R2H-only capabilities.
- No TODOs, placeholders, fake tools, disabled validation, or silent fallbacks.
- Preserve a rollback point before every deletion phase.
- Do not alter licensing notices until a valid commercial-license decision permits it.

## Ownership Rules

### Historical BentoPDF-Owned User-Facing Capabilities (Superseded)

The rule below was part of the superseded contract: it must not be used to remove the current R2H feature routes.

Likely overlap candidates include:

- merge and split;
- compression and repair;
- OCR;
- PDF comparison;
- forms;
- signing and stamping;
- watermark and header/footer;
- page organization and page transformations;
- PDF/image/document conversion;
- metadata, security, attachment, and sanitization tools.

The exact list must come from a generated ownership matrix, not assumptions.

### Historical R2H-Owned Capabilities (Superseded)

R2H retains capabilities not represented by BentoPDF, including application-specific:

- local AI runtime and model management;
- RAG and citations;
- engineering analysis;
- document review workflows;
- audit logging;
- R2H project/session persistence;
- content-edit workflows that are materially different from BentoPDF utilities;
- any R2H capability whose removal would break a retained R2H feature.

## Historical Duplicate Removal Policy (Superseded)

Removal occurs in four independent levels:

1. **Discovery:** map R2H UI, state, IPC, Rust commands, tests, and dependent features.
2. **UI retirement:** remove duplicate cards, panels, menu items, and navigation.
3. **Consumer verification:** prove no retained capability calls the old implementation.
4. **Backend deletion:** remove unused hooks, IPC contracts, commands, Rust modules, tests, and dependencies.

A backend implementation may remain temporarily after its UI is removed when another retained R2H capability still consumes it.

## BentoPDF Theme Strategy

BentoPDF remains a separate application shell internally, but its visible appearance is changed through a centralized R2H theme bridge.

### Theme Source of Truth

R2H values must be extracted from the current R2H source, primarily:

- `src/App.css`
- shared shell/component styles;
- typography and icon usage;
- focus, hover, disabled, success, warning, and error states.

### BentoPDF Theme Bridge

Create one central token layer in the BentoPDF source and route existing BentoPDF variables and utility styles through it.

Required token groups:

```css
--r2h-bg-app
--r2h-bg-surface
--r2h-bg-elevated
--r2h-border
--r2h-border-strong
--r2h-text-primary
--r2h-text-secondary
--r2h-text-muted
--r2h-accent
--r2h-accent-hover
--r2h-focus
--r2h-success
--r2h-warning
--r2h-danger
--r2h-shadow
--r2h-radius-sm
--r2h-radius-md
--r2h-radius-lg
```

The theme must cover:

- page background;
- cards and panels;
- headers and navigation;
- buttons and icon buttons;
- file drop zones;
- inputs, selects, checkboxes, and radios;
- tables;
- modals;
- toasts and alerts;
- progress bars;
- tabs;
- hover/focus/active/disabled states;
- scrollbars;
- light/dark behavior if BentoPDF still exposes both modes.

## Desktop-Shell Cleanup

The BentoPDF window must not show website-only elements that conflict with the desktop product:

- external promotional links;
- donation links;
- GitHub links;
- blog and marketing navigation;
- PWA installation prompts;
- external update prompts;
- unrelated footer material.

Functional tool navigation, legal notices required by the license, accessibility controls, and required attribution remain.

## Runtime Requirements

The current runtime remains:

- `src-tauri/src/bentopdf_runtime.rs`
- loopback-only HTTP server;
- isolation headers;
- local offline bundle;
- one reusable `bentopdf-tools` WebView window.

It must continue to pass:

- loopback-only binding;
- COOP/COEP and required security headers;
- no external requests;
- existing-window reuse;
- clean close/reopen;
- large-output socket handling;
- release packaging checks.

## Testing Strategy

### Ownership Matrix Tests

- every R2H user-facing PDF utility has one owner;
- no duplicate R2H navigation remains for tools that are actually BentoPDF-owned under the current decision;
- every deletion candidate has a recorded dependency result.

### R2H Regression Gates

- `pnpm typecheck`;
- complete Vitest suite;
- `cargo check --lib`;
- retained feature tests;
- release build after deletion batches.

### BentoPDF Visual Gates

- home/tool catalog;
- representative simple tool;
- representative worker/WASM tool;
- OCR tool;
- modal, toast, form, table, progress, disabled, and error states;
- RTL/Arabic where currently supported;
- desktop resolutions and minimum supported window size.

### Runtime Gates

- open, reuse, close, reopen;
- offline network isolation;
- merge smoke;
- large payload handling;
- packaged release runtime lookup.

## Delivery Decomposition

This design is implemented through separate plans:

1. Duplicate ownership matrix and safe-retirement map.
2. R2H duplicate UI retirement.
3. BentoPDF R2H theme bridge.
4. R2H duplicate backend deletion.
5. final runtime, release, and regression certification.

Each plan must produce a separately reviewable commit and preserve rollback.
