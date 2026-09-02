# R2H-PDF

**R2H-PDF** is a desktop PDF workstation for professional document review, editing, OCR, engineering workflows, document comparison, local AI, and evidence-oriented document analysis.

The application combines a **React/TypeScript workstation UI**, a **Rust/Tauri native backend**, and a **MuPDF-backed document engine**.

> Current application version: **2.1.0-beta**

## Product Focus

R2H-PDF is designed for document-heavy professional workflows where users need more than a basic PDF viewer.

Primary areas include:

- Legal and engineering document review
- PDF rendering and navigation
- Content editing
- Page organization
- OCR
- Document comparison
- Forms
- Export
- Comments, highlights, stamps, and redactions
- Engineering-oriented document tools
- Local AI actions
- Retrieval-Augmented Generation (RAG)
- Local model management
- Audit logging
- Session and project persistence

## Core PDF Engine

The native backend uses **MuPDF** for core PDF operations.

The application maintains a native document-session boundary and supports workflows such as:

- Opening PDF documents
- Page rendering
- Navigation and zoom
- Text/content inspection
- Search
- Editing
- Save / Save As
- Session cleanup
- Project-state restoration

## PDF Editor

The editor architecture supports multiple object types, including:

- Text boxes
- Shapes
- Redactions
- Comments
- Highlights
- Stamps
- Image/content replacement overlays

The workstation also contains inline content-editing infrastructure and project-sidecar persistence for durable editing sessions.

## OCR

R2H-PDF includes a local OCR workflow with:

- Page processing
- Runtime validation
- Progress reporting
- OCR result preview
- Structured OCR result contracts
- Searchable/editable overlay generation
- Integration with the PDF editor

The OCR subsystem is designed so recognized text can become part of the document-review and editing workflow rather than remaining a disconnected text export.

## Local AI

The repository contains dedicated feature areas for:

- Local AI runtime
- AI document actions
- Model management
- RAG

The local-AI architecture is intended to keep document intelligence close to the workstation and provide explicit runtime/model management rather than hiding the AI layer behind the UI.

Large model binaries are excluded from the Git repository and are expected to be managed locally.

## RAG & Evidence Workflows

R2H-PDF contains a Retrieval-Augmented Generation feature boundary for document-grounded AI workflows.

The application tracks when document edits can invalidate a previously built retrieval index, preventing stale document state from being silently treated as current evidence.

## Document Comparison

A dedicated comparison feature allows document-review workflows to analyze differences between PDF documents.

This sits alongside the standard rendering/editor engine instead of being implemented as an external web workflow.

## Page Organization

R2H-PDF includes a dedicated page-organizer subsystem for operations involving PDF page structure and ordering.

## Engineering Workflows

The source tree includes a dedicated engineering feature area in addition to general PDF review features.

The product is therefore structured for professional technical-document workflows rather than only consumer PDF viewing.

## Audit & Diagnostics

The workstation includes:

- Audit logging
- Diagnostics
- Session persistence
- Autosave
- Recent-document state
- Document properties
- Preferences
- Command palette

These features support longer-lived professional review sessions and make document operations easier to inspect.

## BentoPDF Utility Surface

R2H-PDF can expose BentoPDF as an additional local utility surface.

Core R2H-owned workflows such as OCR, Compare, Forms, Page Organizer, signing/stamping, and local generation are maintained as application-owned production workflows rather than delegated entirely to BentoPDF.

## Tech Stack

| Layer | Technology |
| --- | --- |
| Desktop Runtime | Tauri 2 |
| Native Backend | Rust |
| PDF Engine | MuPDF |
| Frontend | React 19 |
| Language | TypeScript |
| Bundler | Vite |
| Testing | Vitest, Testing Library, property-based testing |
| Native Async | Tokio |
| Networking | Reqwest |
| Integrity / Crypto | SHA-2, Ed25519 |
| Installer | Windows / Inno Setup workflow |

## Application Architecture

```text
src/
├── components/
├── features/
│   ├── ai-actions/
│   ├── ai-local/
│   ├── audit-log/
│   ├── bentopdf/
│   ├── compare/
│   ├── content-edit/
│   ├── engineering/
│   ├── export/
│   ├── forms/
│   ├── model-manager/
│   ├── objects/
│   ├── ocr/
│   ├── page-organizer/
│   ├── pdf-editor/
│   └── rag/
├── hooks/
├── lib/
├── state/
└── types/

src-tauri/
├── src/
└── Cargo.toml
```

## Development

### Requirements

- Node.js
- pnpm
- Rust toolchain
- Tauri prerequisites
- Native dependencies required by the bundled document engine

### Install

```bash
pnpm install
```

### Development UI

```bash
pnpm dev
```

### Type Check

```bash
pnpm typecheck
```

### Lint

```bash
pnpm lint
```

### Tests

```bash
pnpm test
```

### Frontend Build

```bash
pnpm build
```

### Desktop / Tauri Build

```bash
pnpm tauri build
```

## Quality Gates

The project exposes independent commands for:

```text
TypeScript type checking
ESLint
Vitest
Frontend production build
Rust / Cargo validation
```

These should be run before producing a release build.

## Local Models

AI/OCR model files can be large and are not intended to live in normal Git history.

Keep runtime models in the configured local model directories and manage them separately from the application source repository.

## Status

R2H-PDF is currently a **beta** product and remains under active development.

Professional users should validate generated, edited, compared, OCR-processed, or AI-assisted document output before relying on it for contractual, legal, engineering, compliance, or other high-consequence decisions.

## Repository

[R2H-PDF](https://github.com/ramyelattar/R2H-PDF)

---

**R2H — A professional PDF workstation built around local document control.**
