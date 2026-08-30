# Text-Editing Fixtures (Phase 29F)

This directory is intentionally **empty in version control**. Real-world
PDFs from Microsoft Word, LibreOffice, and other office suites cannot be
committed here for size and licensing reasons.

Phase 29F's round-trip suite is implemented in
`src-tauri/src/content_editing/round_trip_tests.rs` and uses **synthetic
minimal content streams** generated in Rust:

- `WORD_LIKE_STREAM` — simple `Tj` operators on consecutive lines, the
  shape Microsoft Word produces for ASCII paragraphs.
- `LIBRE_LIKE_STREAM` — `TJ` array with kerning glue, the shape
  LibreOffice produces.
- `REPEATED_STREAM` — three identical `(Total) Tj` operators to verify
  occurrence-based disambiguation.
- Subset font, Identity-H Type0, and Latin-1 round-trips are driven
  through `classify_text_edit_strategy` with hand-built
  `FontResourceInfo` records — those font kinds cannot be synthesised
  through mupdf-rs without committing real font binaries.

## Adding real fixture PDFs

If you want to extend the suite with real PDFs:

1. Place a PDF under `text-editing/your-fixture.pdf`.
2. Add a test in `round_trip_tests.rs` that loads it via
   `mupdf::pdf::PdfDocument::from_bytes(include_bytes!("../../test-fixtures/text-editing/your-fixture.pdf"))`,
   then runs the same assertions:
   - parse operators
   - apply edit
   - assert new text appears and old text removed
   - assert no corruption
3. Do **not** check in PDFs containing proprietary fonts unless
   licensed for redistribution.
