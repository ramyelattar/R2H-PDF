import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import { useObjectsPanel } from "./useObjectsPanel";
import type {
  CommentObject,
  HighlightObject,
  StampObject,
  StrikethroughObject,
  TextBoxObject,
} from "../pdf-editor/types";

function tbox(id: string, page = 0, text = "Hello"): TextBoxObject {
  return {
    id, sessionId: "s1", pageIndex: page, type: "textBox",
    rect: { x: 10, y: 20, width: 100, height: 30 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: {},
    text, fontSize: 12, fontFamily: "Helvetica",
    color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
  };
}

function compareComment(id: string, page = 1): CommentObject {
  return {
    id, sessionId: "s1", pageIndex: page, type: "comment",
    rect: { x: 50, y: 700, width: 24, height: 24 },
    rotation: 0, zIndex: 2, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: { source: "compare" },
    contents: "from compare", author: "Compare", color: "#c66", status: "open",
  };
}

function signatureStamp(id: string, page = 2): StampObject {
  return {
    id, sessionId: "s1", pageIndex: page, type: "stamp",
    rect: { x: 100, y: 100, width: 200, height: 60 },
    rotation: 0, zIndex: 3, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: { source: "signature" },
    stampText: "J. Doe", stampType: "CUSTOM", color: "#0a4",
  };
}

function lockedHighlight(id: string): HighlightObject {
  return {
    id, sessionId: "s1", pageIndex: 0, type: "highlight",
    rect: { x: 0, y: 0, width: 50, height: 12 },
    rotation: 0, zIndex: 4, locked: true, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: {},
    color: "rgba(255,255,0,0.3)", opacity: 0.3, contents: "",
  };
}

function redlineStrike(id: string): StrikethroughObject {
  return {
    id, sessionId: "s1", pageIndex: 0, type: "strikethrough",
    rect: { x: 12, y: 700, width: 80, height: 14 },
    rotation: 0, zIndex: 5, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: { source: "compare" },
    color: "#c33", contents: "removed line",
  };
}

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  // Default responses: empty lists for all backend calls.
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "doc_list_form_fields") return [];
    if (cmd === "pdf_list_content_edits") return [];
    if (cmd === "pdf_get_page_content_objects") return [];
    return [];
  });
});

describe("useObjectsPanel — aggregation", () => {
  it("includes overlay objects as rows with full action availability", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tbox("tb-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const rows = result.current.allRows;
    const row = rows.find((r) => r.sourceId === "tb-1");
    expect(row).toBeDefined();
    expect(row!.kind).toBe("textBox");
    expect(row!.source).toBe("manual");
    const a = result.current.actionsFor(row!);
    expect(a.canDelete).toBe(true);
    expect(a.canHide).toBe(true);
    expect(a.canLock).toBe(true);
  });

  it("identifies compare-source overlays via metadata.source", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [compareComment("cmp-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.sourceId === "cmp-1")!;
    expect(row.source).toBe("compare");
  });

  it("identifies signature overlays via metadata.source", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [signatureStamp("sig-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.sourceId === "sig-1")!;
    expect(row.source).toBe("signature");
  });

  it("includes strikethrough/underline overlays as their own kind", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [redlineStrike("rl-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.sourceId === "rl-1")!;
    expect(row.kind).toBe("strikethrough");
    expect(row.source).toBe("compare");
  });

  it("delete is disabled with a reason when overlay is locked", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [lockedHighlight("lh-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.sourceId === "lh-1")!;
    const a = result.current.actionsFor(row);
    expect(a.canDelete).toBe(false);
    expect(a.disabledReason ?? "").toMatch(/locked/i);
  });

  it("includes form fields from the backend and disables delete with a reason", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "doc_list_form_fields") {
        return [{ name: "Customer", field_type: "text", value: "Alice", page_index: 0, rect: [0, 0, 50, 20] }];
      }
      return [];
    });
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.id.startsWith("form:"))!;
    expect(row.kind).toBe("form_field");
    const a = result.current.actionsFor(row);
    expect(a.canDelete).toBe(false);
    expect(a.disabledReason).toContain("Forms tab");
  });

  it("includes native content objects with read-only action availability", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "pdf_get_page_content_objects") {
        return [{
          id: "nat-1", session_id: "s1", page_index: 0,
          object_type: "image_xobject",
          bbox: [0, 0, 200, 100], z_index: 0, editable_level: "native_replaceable",
          text_info: null,
          image_info: { xobject_name: "Im1", width: 200, height: 100, color_space: "DeviceRGB", bits_per_component: 8, transform_matrix: [1,0,0,1,0,0] },
          style_info: null,
          diagnostics: [],
        }];
      }
      return [];
    });
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const row = result.current.allRows.find((r) => r.id.startsWith("native:"))!;
    expect(row.kind).toBe("native_image");
    const a = result.current.actionsFor(row);
    expect(a.canDelete).toBe(false);
    expect(a.disabledReason ?? "").toMatch(/Edit Content/i);
  });

  it("filter.search narrows the rows", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tbox("a", 0, "hello world"), tbox("b", 0, "totally different")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.setFilter({ ...result.current.filter, search: "hello" }));
    const rows = result.current.filteredRows;
    expect(rows.find((r) => r.sourceId === "a")).toBeDefined();
    expect(rows.find((r) => r.sourceId === "b")).toBeUndefined();
  });

  it("filter.source narrows to a specific source", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tbox("a"), compareComment("c")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.setFilter({ ...result.current.filter, source: "compare" }));
    expect(result.current.filteredRows).toHaveLength(1);
    expect(result.current.filteredRows[0].sourceId).toBe("c");
  });

  it("filter.page narrows to a single page", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tbox("a", 0), tbox("b", 3)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    act(() => result.current.setFilter({ ...result.current.filter, page: 3 }));
    expect(result.current.filteredRows).toHaveLength(1);
    expect(result.current.filteredRows[0].pageIndex).toBe(3);
  });
});

// ─── Phase 27B — z-order reordering math ──────────────────────────────

describe("useObjectsPanel — Phase 27B z-order math", () => {
  function tboxZ(id: string, z: number, page = 0): TextBoxObject {
    return {
      id, sessionId: "s1", pageIndex: page, type: "textBox",
      rect: { x: 0, y: 0, width: 50, height: 20 },
      rotation: 0, zIndex: z, locked: false, hidden: false,
      createdAt: 0, updatedAt: 0, metadata: {},
      text: id, fontSize: 12, fontFamily: "Helvetica",
      color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
    };
  }

  it("reorder availability mirrors locked-ness", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 1), lockedHighlight("b")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    const rowA = result.current.allRows.find((r) => r.sourceId === "a")!;
    const rowB = result.current.allRows.find((r) => r.sourceId === "b")!;
    expect(result.current.actionsFor(rowA).canReorder).toBe(true);
    expect(result.current.actionsFor(rowB).canReorder).toBe(false);
    expect(result.current.actionsFor(rowB).reorderDisabledReason ?? "").toMatch(/locked/i);
  });

  it("forward increments zIndex by 1 when not already top", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 1), tboxZ("b", 3)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "forward")).toBe(2);
  });

  it("forward at top returns null (no-op)", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 5)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "forward")).toBeNull();
  });

  it("backward decrements zIndex without going negative", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 2), tboxZ("b", 5)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "backward")).toBe(1);
  });

  it("backward at 0 returns null (no-op when already at min)", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 0)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "backward")).toBeNull();
  });

  it("bring to front sets max+1", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 1), tboxZ("b", 7)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "front")).toBe(8);
  });

  it("send to back goes to min-1 clamped to 0", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 5), tboxZ("b", 2)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "back")).toBe(1);
  });

  it("locked overlay reorder returns null", async () => {
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [lockedHighlight("lh-1")],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("lh-1", "forward")).toBeNull();
    expect(result.current.computeReorderZIndex("lh-1", "front")).toBeNull();
  });

  it("peers are scoped to the same page", async () => {
    // 'a' on page 0 with z=2; 'b' on page 3 with z=10. Front for 'a' should
    // return 3 (max on its own page + 1), not 11.
    const { result } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 2, 0), tboxZ("b", 10, 3)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.computeReorderZIndex("a", "front")).toBeNull();
    // It's the only object on page 0 so peers.length === 1 → null per spec.
    // Add a second peer to make the case interesting.
    const { result: r2 } = renderHook(() => useObjectsPanel({
      sessionId: "s1",
      overlayObjects: [tboxZ("a", 2, 0), tboxZ("c", 5, 0), tboxZ("b", 10, 3)],
      activePageIndex: 0,
    }));
    await waitFor(() => expect(r2.current.loading).toBe(false));
    expect(r2.current.computeReorderZIndex("a", "front")).toBe(6);
  });
});
