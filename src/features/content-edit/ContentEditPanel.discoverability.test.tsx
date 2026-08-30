/**
 * Phase 32 — UI discoverability tests for the Edit Content tab.
 *
 * The original UI showed text objects as plain bullets and buried the edit
 * workflow inside a "Selected: ..." details panel. These tests pin the
 * minimum discoverability contract:
 *   1. The panel surfaces a clear "Click a text item" instruction.
 *   2. Each text object is rendered as a selectable, clickable row.
 *   3. Selecting a row reveals the Edit Inline / Edit in Panel / Apply /
 *      Cancel controls and a method badge.
 *   4. Native-editable rows show the "Native editable" hint.
 *   5. Visual-fallback rows show the "Safe visual replacement" warning.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

// Mock all IPC entry points used by ContentEditPanel so we can drive the
// component synchronously with a fixed set of text objects.
vi.mock("../../lib/ipc", async () => {
  const actual = await vi.importActual<typeof import("../../lib/ipc")>("../../lib/ipc");
  return {
    ...actual,
    pdfGetPageContentObjects: vi.fn(),
    pdfPrepareTextBlockEdit: vi.fn(),
    pdfListContentEdits: vi.fn(),
    pdfGetPageFontRegistry: vi.fn(),
    pdfApplyNativeTextEdit: vi.fn(),
    pdfReplaceNativeImage: vi.fn(),
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
  pdfPreviewFindReplace,
  pdfApplyFindReplace,
  pdfReplaceNativeImage,
  type ContentObject,
} from "../../lib/ipc";
import { ContentEditPanel } from "./ContentEditPanel";

const makeTextObject = (overrides: Partial<ContentObject> & { id: string; decoded: string; level: ContentObject["editable_level"] }): ContentObject => ({
  id: overrides.id,
  session_id: "doc-session-test",
  page_index: 0,
  object_type: "text_span",
  bbox: [10, 700, 200, 720],
  z_index: 0,
  editable_level: overrides.level,
  text_info: {
    raw_text: overrides.decoded,
    decoded_text: overrides.decoded,
    glyph_count: overrides.decoded.length,
    font_name: "Helvetica",
    font_size: 12,
    fill_color: null,
    stroke_color: null,
    writing_mode: "horizontal",
    is_subset_font: false,
    encoding_safe: true,
    operator_offset: null,
    operator_length: null,
    occurrence_index: 0,
    operator_type: "tj",
    editable_strategy: overrides.level === "native_editable" ? "native_in_place" : "safe_visual_replacement",
    font_encoding: "WinAnsiEncoding",
  },
  image_info: null,
  style_info: null,
  diagnostics: [],
  ...overrides,
});

const makeImageObject = (id = "img-1"): ContentObject => ({
  id,
  session_id: "doc-session-test",
  page_index: 0,
  object_type: "image_xobject",
  bbox: [10, 100, 210, 200],
  z_index: 1,
  editable_level: "visual_patch_only",
  text_info: null,
  image_info: {
    xobject_name: "Im1",
    width: 400,
    height: 200,
    color_space: "DeviceRGB",
    bits_per_component: 8,
    transform_matrix: [200, 0, 0, 100, 10, 100],
  },
  style_info: null,
  diagnostics: [],
});

const setupIpcMocks = (objects: ContentObject[]) => {
  vi.mocked(pdfGetPageContentObjects).mockResolvedValue({ ok: true, data: objects });
  vi.mocked(pdfPrepareTextBlockEdit).mockResolvedValue({ ok: true, data: [] });
  vi.mocked(pdfListContentEdits).mockResolvedValue({ ok: true, data: [] });
  vi.mocked(pdfGetPageFontRegistry).mockResolvedValue({
    ok: true,
    data: {
      session_id: "doc-session-test",
      page_index: 0,
      fonts: [],
    },
  });
  vi.mocked(pdfPreviewFindReplace).mockResolvedValue({ ok: true, data: { session_id: "doc-session-test", matches: [], unsafe_count: 0, warnings: [] } });
  vi.mocked(pdfApplyFindReplace).mockResolvedValue({ ok: true, data: { session_id: "doc-session-test", applied_count: 0, skipped_count: 0, failed_count: 0, results: [], warnings: [] } });
};

const flushAsync = async () => {
  // Two microtask flushes cover the two Promise.all() chains in loadObjects.
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
};

describe("Phase 32 — Edit Content panel discoverability", () => {
  it("renders a clear instruction for editing text", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Hello world", level: "native_editable" }),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const instructions = await screen.findByTestId("content-edit-instructions");
    // The callout must instruct the user how to start editing — naming
    // both the page interaction (double-click) and the row interaction.
    expect(instructions.textContent?.toLowerCase()).toContain("double-click");
    expect(instructions.textContent?.toLowerCase()).toContain("row");
    // It must also call out the native-vs-visual tradeoff so users aren't
    // surprised by the fallback method.
    expect(instructions.textContent?.toLowerCase()).toContain("native");
    expect(instructions.textContent?.toLowerCase()).toContain("visual");
  });

  it("renders each text object as a clickable, selectable row", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Native row", level: "native_editable" }),
      makeTextObject({ id: "t2", decoded: "Visual row", level: "visual_patch_only" }),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row1 = await screen.findByTestId("text-obj-row-t1");
    const row2 = await screen.findByTestId("text-obj-row-t2");
    // Rows expose ARIA option semantics so the list is announced correctly.
    expect(row1.getAttribute("role")).toBe("option");
    expect(row2.getAttribute("role")).toBe("option");
    // Visual-patch rows surface the "Safe visual replacement" warning so the
    // user knows the fallback method will be used.
    const visualHints = screen.getAllByTestId("text-row-visual-hint");
    expect(visualHints.length).toBeGreaterThan(0);
    // Native-editable rows surface the "Native editable" hint.
    const nativeHints = screen.getAllByTestId("text-row-native-hint");
    expect(nativeHints.length).toBeGreaterThan(0);
  });

  it("reveals Edit Inline + Edit in Panel + Apply + Cancel controls when a native-editable row is selected", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Editable native text", level: "native_editable" }),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);

    const row = await screen.findByTestId("text-obj-row-t1");
    expect(row.getAttribute("aria-selected")).toBe("false");
    fireEvent.click(row);
    await flushAsync();

    expect(screen.getByTestId("text-obj-row-t1").getAttribute("aria-selected")).toBe("true");
    // Edit Inline and Edit in Panel actions must both be visible.
    expect(screen.getByTestId("open-inline-editor")).toBeTruthy();
    expect(screen.getByTestId("open-panel-editor")).toBeTruthy();
    // The original text is surfaced explicitly so the user can compare.
    const original = screen.getByTestId("text-edit-original");
    expect(original.textContent).toContain("Editable native text");
    // Apply / Cancel are both rendered (Apply is initially disabled because
    // the textarea matches the original text — that's the correct state).
    const apply = screen.getByTestId("text-edit-apply");
    const cancel = screen.getByTestId("text-edit-cancel");
    expect(apply.textContent?.trim()).toBe("Apply");
    expect(cancel.textContent?.trim()).toBe("Cancel");
    // With no font-registry entry in this test fixture, the panel must not
    // claim a native edit. Phase 34 requires visual fallback unless native
    // font/encoding/operator identity can be verified.
    expect(screen.getByTestId("method-badge-safe_visual_replacement")).toBeTruthy();
  });

  it("activates the Apply button after the replacement text changes", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Hello", level: "native_editable" }),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-t1");
    fireEvent.click(row);
    await flushAsync();

    const apply = screen.getByTestId("text-edit-apply") as HTMLButtonElement;
    expect(apply.disabled).toBe(true);
    const textarea = screen.getByTestId("text-edit-replacement-input") as HTMLTextAreaElement;
    fireEvent.change(textarea, { target: { value: "Hello world" } });
    expect(apply.disabled).toBe(false);
  });

  it("shows the Safe Visual Replacement warning + action for visual-patch-only text", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Subset font glyphs", level: "visual_patch_only" }),
    ]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    const row = await screen.findByTestId("text-obj-row-t1");
    fireEvent.click(row);
    await flushAsync();
    expect(screen.getByTestId("content-edit-text-action-visual")).toBeTruthy();
    expect(screen.getByTestId("text-edit-apply-visual").textContent).toContain("Safe Visual Replacement");
  });

  it("previews and applies find/replace safe matches with review", async () => {
    setupIpcMocks([
      makeTextObject({ id: "t1", decoded: "Panel A", level: "native_editable" }),
    ]);
    vi.mocked(pdfPreviewFindReplace).mockResolvedValueOnce({
      ok: true,
      data: {
        session_id: "doc-session-test",
        unsafe_count: 1,
        warnings: [],
        matches: [
          {
            id: "fr-1",
            page_index: 0,
            content_object_id: "t1",
            text_preview: "Panel A",
            bbox: [10, 700, 200, 720],
            editable_status: "safe",
            replacement_method: "Native text edit",
            safe: true,
            reason: null,
            checked: true,
          },
          {
            id: "fr-2",
            page_index: 0,
            content_object_id: "bad",
            text_preview: "����",
            bbox: [10, 600, 200, 620],
            editable_status: "not_safely_editable",
            replacement_method: "Not safely editable",
            safe: false,
            reason: "Text encoding could not be decoded safely.",
            checked: false,
          },
        ],
      },
    });
    vi.mocked(pdfApplyFindReplace).mockResolvedValueOnce({
      ok: true,
      data: {
        session_id: "doc-session-test",
        applied_count: 1,
        skipped_count: 0,
        failed_count: 0,
        results: [],
        warnings: [],
      },
    });
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    fireEvent.change(await screen.findByTestId("find-replace-find"), { target: { value: "Panel" } });
    fireEvent.change(screen.getByTestId("find-replace-replace"), { target: { value: "Board" } });
    fireEvent.click(screen.getByTestId("find-replace-preview"));
    expect(await screen.findByTestId("find-replace-results")).toBeTruthy();
    expect(screen.getByText(/Skipped: Text encoding could not be decoded safely/)).toBeTruthy();

    fireEvent.click(screen.getByTestId("find-replace-all-safe"));
    await flushAsync();
    expect(pdfApplyFindReplace).toHaveBeenCalledWith({
      session_id: "doc-session-test",
      replace_text: "Board",
      matches: [{ id: "fr-1", page_index: 0, content_object_id: "t1" }],
    });
  });

  it("stages image replacement without committing immediately", async () => {
    setupIpcMocks([makeImageObject()]);
    const onStageImageReplace = vi.fn();
    render(
      <ContentEditPanel
        sessionId="doc-session-test"
        currentPageIndex={0}
        onStageImageReplace={onStageImageReplace}
      />,
    );
    fireEvent.click(await screen.findByTestId("image-obj-row-img-1"));

    const file = new File(["fake-png"], "replacement.png", { type: "image/png" });
    fireEvent.change(screen.getByTestId("image-replace-input"), { target: { files: [file] } });

    expect(pdfReplaceNativeImage).not.toHaveBeenCalled();
    await waitFor(() => expect(onStageImageReplace).toHaveBeenCalled());
  });

  it("disables webp replacement with a clear reason", async () => {
    setupIpcMocks([makeImageObject()]);
    render(<ContentEditPanel sessionId="doc-session-test" currentPageIndex={0} />);
    fireEvent.click(await screen.findByTestId("image-obj-row-img-1"));

    const file = new File(["fake-webp"], "replacement.webp", { type: "image/webp" });
    fireEvent.change(screen.getByTestId("image-replace-input"), { target: { files: [file] } });
    expect(await screen.findByText(/WebP replacement is disabled/)).toBeTruthy();
    expect(pdfReplaceNativeImage).not.toHaveBeenCalled();
  });
});
