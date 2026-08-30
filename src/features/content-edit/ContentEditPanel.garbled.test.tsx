/**
 * Acrobat-UX fix — garbled text never appears as a primary row label.
 *
 * The complaint: opening a real PDF showed a sidebar full of
 * `������������` rows. The user could not tell what they were
 * editing, and double-clicking a row would silently try to "edit"
 * non-existent text. These tests pin the corrected behaviour:
 *
 *   1. A row whose backend `decoding_quality === "garbled"` shows
 *      "Text object" as the primary label, never the raw garbage.
 *   2. A garbled row surfaces a warning chip that explains *why*
 *      it can't be edited.
 *   3. Double-clicking a garbled row does not open the inline
 *      editor.
 *   4. A row whose backend lacks `decoding_quality` but whose
 *      decoded string is mostly U+FFFD is treated the same.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("../../lib/ipc", async () => {
  const actual = await vi.importActual<typeof import("../../lib/ipc")>("../../lib/ipc");
  return {
    ...actual,
    pdfGetPageContentObjects: vi.fn(),
    pdfPrepareTextBlockEdit: vi.fn(),
    pdfListContentEdits: vi.fn(),
    pdfGetPageFontRegistry: vi.fn(),
    pdfApplyNativeTextEdit: vi.fn(),
    pdfApplyTextBlockEdit: vi.fn(),
    pdfRevertContentEdit: vi.fn(),
    pdfDeleteNativeImage: vi.fn(),
    pdfMoveNativeImage: vi.fn(),
    pdfCropNativeImage: vi.fn(),
    pdfRotateNativeImage: vi.fn(),
    pdfPreviewFindReplace: vi.fn(),
    pdfApplyFindReplace: vi.fn(),
    pdfCoverPathObject: vi.fn(),
  };
});

import {
  pdfGetPageContentObjects,
  pdfPrepareTextBlockEdit,
  pdfListContentEdits,
  pdfGetPageFontRegistry,
  type ContentObject,
} from "../../lib/ipc";
import { ContentEditPanel } from "./ContentEditPanel";

const makeRow = (
  id: string,
  decoded: string,
  quality: "ok" | "garbled" | "partial" | undefined,
  level: ContentObject["editable_level"] = "native_editable",
): ContentObject => ({
  id,
  session_id: "doc-session-test",
  page_index: 0,
  object_type: "text_span",
  bbox: [10, 700, 200, 720],
  z_index: 0,
  editable_level: level,
  text_info: {
    raw_text: decoded,
    decoded_text: decoded,
    glyph_count: decoded.length,
    font_name: "Helvetica",
    font_size: 12,
    fill_color: null,
    stroke_color: null,
    writing_mode: "horizontal",
    is_subset_font: false,
    encoding_safe: quality !== "garbled",
    operator_offset: null,
    operator_length: null,
    occurrence_index: 0,
    operator_type: "tj",
    editable_strategy: level === "read_only" ? "read_only" : "native_in_place",
    font_encoding: "WinAnsiEncoding",
    decoding_quality: quality,
  },
  image_info: null,
  style_info: null,
  diagnostics: [],
});

const setup = (rows: ContentObject[]) => {
  vi.mocked(pdfGetPageContentObjects).mockResolvedValue({ ok: true, data: rows });
  vi.mocked(pdfPrepareTextBlockEdit).mockResolvedValue({ ok: true, data: [] });
  vi.mocked(pdfListContentEdits).mockResolvedValue({ ok: true, data: [] });
  vi.mocked(pdfGetPageFontRegistry).mockResolvedValue({
    ok: true,
    data: { session_id: "doc-session-test", page_index: 0, fonts: [] },
  });
};

const flushAsync = async () => {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
};

describe("Garbled text never appears as primary label", () => {
  it("backend-flagged garbled row shows 'Text object', not the decoded garbage", async () => {
    setup([
      makeRow("g1", "Doesn't matter — encoding broken", "garbled", "read_only"),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-g1");
    expect(row.textContent).toContain("Text object");
    // The raw garbage must not leak through as the visible label.
    expect(row.textContent).not.toContain("Doesn't matter");
    // Warning chip is shown.
    expect(screen.getByTestId("text-row-garbled-hint")).toBeTruthy();
  });

  it("heuristic catches U+FFFD-heavy strings even without the backend flag", async () => {
    setup([
      makeRow("g2", "����������������", undefined, "native_editable"),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-g2");
    expect(row.textContent).toContain("Text object");
    expect(screen.getByTestId("text-row-garbled-hint")).toBeTruthy();
  });

  it("double-clicking a garbled row selects it but does not open the inline editor", async () => {
    setup([
      makeRow("g3", "����������������", "garbled", "read_only"),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-g3");
    fireEvent.doubleClick(row);
    await flushAsync();
    // No inline editor mounted.
    expect(screen.queryByTestId("inline-text-editor")).toBeNull();
  });

  it("clean rows still show their text and the native badge", async () => {
    setup([
      makeRow("c1", "Hello world", "ok", "native_editable"),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-c1");
    expect(row.textContent).toContain("Hello world");
    expect(screen.queryByTestId("text-row-garbled-hint")).toBeNull();
  });
});

describe("Right inspector primary message", () => {
  it("tells the user to edit on the page (advanced list is fallback)", async () => {
    setup([makeRow("c1", "Hello", "ok", "native_editable")]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const instructions = await screen.findByTestId("content-edit-instructions");
    const body = instructions.textContent?.toLowerCase() ?? "";
    expect(body).toContain("edit directly on the page");
    expect(body).toContain("double-click");
    // The text object list is presented as a fallback, not the main path.
    expect(body).toContain("fallback");
  });

  it("renders the text-object list under an Advanced disclosure", async () => {
    setup([makeRow("c1", "Hello", "ok", "native_editable")]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const details = await screen.findByTestId("content-edit-text-objects-details");
    expect(details.tagName.toLowerCase()).toBe("details");
    expect(details.textContent?.toLowerCase()).toContain("advanced");
  });
});
