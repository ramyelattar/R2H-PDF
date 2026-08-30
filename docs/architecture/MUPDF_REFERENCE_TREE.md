# MuPDF Reference Tree at the Repository Root

Status: **VERIFIED not used by the build** (verified 2026-08-30)

## What it is

The repository root contains a full MuPDF **1.28.0** upstream source tree
(`source/`, `include/`, `platform/`, `resources/`, `Makefile`, `Makelists`,
`Makerules`, `Makethird`, `CHANGES`, `COPYING`, `CONTRIBUTORS`, `setup.py`,
`scripts/mupdfwrap*`). Its headers were deliberately rebranded to
"R2H-PDF" (`include/mupdf/fitz/version.h`, guard `R2H-PDF_FITZ_VERSION_H`).

Most of `thirdparty/` at the root belongs to this upstream checkout as well;
those third-party library directories are **empty placeholder directories**
(submodules that were never populated), except `thirdparty/bentopdf`, which
is the separately tracked BentoPDF checkout declared in `.gitmodules`.

## Why it is retained

- Engineering reference for the DocumentEngine capabilities listed in
  `AGENTS.md` (rendering, extraction, annotation semantics).
- The rebranding work in the headers represents deliberate prior work and is
  preserved intact.

## Evidence that the build does not use it

- `src-tauri/build.rs` contains only `tauri_build::build()`.
- The Rust dependency chain is:
  `src-tauri` → `mupdf = { path = "../vendor/mupdf-0.6.0", default-features = false }`
  (pure-Rust safe wrapper, vendored, **no build.rs**) →
  `mupdf-sys 0.6.0` from crates.io (`registry+…` in `src-tauri/Cargo.lock`),
  which compiles its own bundled MuPDF C sources.
- No `MUPDF_SRC`/`MUPDF_DIR`/`MUPDF_SOURCE` environment variable or path
  reference to the root tree exists in `src-tauri/`, `.github/workflows/`,
  or `scripts/` (repo-wide search, 2026-08-30).
- No CMake/Make invocation, packaging step, test, or offline-dependency
  fallback references `source/`, `include/`, or the root `Makefile`.

## Boundary statement

This tree is **reference material, not a build input**. Changes to it have no
effect on `r2h-pdf` binaries or installers. Do not "fix" engine behavior
here and expect the application to change; the engine lives in
`src-tauri/src/document_core/` on top of `mupdf-sys`.

## License facts (technical, not legal advice)

- The reference tree is MuPDF, licensed **AGPL-3.0** (`COPYING` at the root).
  It is tracked in the repository for reference only; it is not compiled,
  linked, or distributed by the build.
- The **shipped** engine does link MuPDF (via `mupdf-sys`), which carries its
  own AGPL-3.0 obligations for distribution of the application. This applies
  regardless of the reference tree and requires an explicit licensing
  decision before any non-AGPL distribution of R2H-PDF.
- `thirdparty/bentopdf` (BentoPDF 2.8.6, optional offline tools window) is
  likewise AGPL-3.0 (`thirdparty/bentopdf/LICENSE`).

Both AGPL touchpoints are flagged for explicit licensing review in
`release/` evidence and the release readiness report.
