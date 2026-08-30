import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePdfEditorState } from "./usePdfEditorState";
import type { TextBoxObject, CommentObject, HighlightObject, StampObject, RedactionObject } from "./types";

function makeTextBox(overrides: Partial<TextBoxObject> = {}): TextBoxObject {
  return {
    id: `obj-${Date.now()}-${Math.random().toString(36).slice(2)}`,
    sessionId: "session-1", pageIndex: 0, type: "textBox",
    rect: { x: 100, y: 200, width: 120, height: 40 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: Date.now(), updatedAt: Date.now(), metadata: {},
    text: "Hello", fontSize: 12, fontFamily: "Helvetica",
    color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
    ...overrides,
  };
}

function makeComment(overrides: Partial<CommentObject> = {}): CommentObject {
  return {
    id: `comment-${Date.now()}`, sessionId: "session-1", pageIndex: 0, type: "comment",
    rect: { x: 50, y: 700, width: 24, height: 24 },
    rotation: 0, zIndex: 2, locked: false, hidden: false,
    createdAt: Date.now(), updatedAt: Date.now(), metadata: {},
    contents: "Review this", author: "User", color: "#ffba6b", status: "open",
    ...overrides,
  };
}

function makeHighlight(overrides: Partial<HighlightObject> = {}): HighlightObject {
  return {
    id: `hl-${Date.now()}`, sessionId: "session-1", pageIndex: 0, type: "highlight",
    rect: { x: 72, y: 500, width: 200, height: 14 },
    rotation: 0, zIndex: 3, locked: false, hidden: false,
    createdAt: Date.now(), updatedAt: Date.now(), metadata: {},
    color: "rgba(255,255,0,0.4)", opacity: 0.4, contents: "",
    ...overrides,
  };
}

function makeStamp(overrides: Partial<StampObject> = {}): StampObject {
  return {
    id: `stamp-${Date.now()}`, sessionId: "session-1", pageIndex: 0, type: "stamp",
    rect: { x: 200, y: 600, width: 150, height: 50 },
    rotation: 0, zIndex: 4, locked: false, hidden: false,
    createdAt: Date.now(), updatedAt: Date.now(), metadata: {},
    stampText: "APPROVED", stampType: "APPROVED", color: "#0a0",
    ...overrides,
  };
}

function makeRedaction(overrides: Partial<RedactionObject> = {}): RedactionObject {
  return {
    id: `redact-${Date.now()}`, sessionId: "session-1", pageIndex: 0, type: "redaction",
    rect: { x: 100, y: 300, width: 200, height: 20 },
    rotation: 0, zIndex: 5, locked: false, hidden: false,
    createdAt: Date.now(), updatedAt: Date.now(), metadata: {},
    fillColor: "#000", replacementText: "", reason: "PII", status: "draft",
    ...overrides,
  };
}

describe("usePdfEditorState", () => {
  it("starts with empty state", () => {
    const { result } = renderHook(() => usePdfEditorState());
    expect(result.current.objects.size).toBe(0);
    expect(result.current.selectedIds).toEqual([]);
    expect(result.current.activeTool).toBe("select");
  });

  it("adds a text box and selects it", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeTextBox({ id: "obj-1" });
    act(() => { result.current.addObject(obj); });
    expect(result.current.objects.size).toBe(1);
    expect(result.current.selectedIds).toEqual(["obj-1"]);
  });

  it("creates a comment object", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeComment({ id: "c-1" });
    act(() => { result.current.addObject(obj); });
    const stored = result.current.objects.get("c-1") as CommentObject;
    expect(stored.type).toBe("comment");
    expect(stored.contents).toBe("Review this");
    expect(stored.status).toBe("open");
  });

  it("creates a highlight object", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeHighlight({ id: "hl-1" });
    act(() => { result.current.addObject(obj); });
    const stored = result.current.objects.get("hl-1") as HighlightObject;
    expect(stored.type).toBe("highlight");
    expect(stored.opacity).toBe(0.4);
  });

  it("creates a stamp object", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeStamp({ id: "st-1" });
    act(() => { result.current.addObject(obj); });
    const stored = result.current.objects.get("st-1") as StampObject;
    expect(stored.type).toBe("stamp");
    expect(stored.stampType).toBe("APPROVED");
  });

  it("edits text box text via patch with undo", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeTextBox({ id: "tb-1", text: "Original" });
    act(() => { result.current.addObject(obj); });
    act(() => { result.current.patchObject("tb-1", { text: "Edited" }); });

    const edited = result.current.objects.get("tb-1") as TextBoxObject;
    expect(edited.text).toBe("Edited");

    act(() => { result.current.undo(); });
    const restored = result.current.objects.get("tb-1") as TextBoxObject;
    expect(restored.text).toBe("Original");
  });

  it("resolves a comment via patch", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeComment({ id: "c-1", status: "open" });
    act(() => { result.current.addObject(obj); });
    act(() => { result.current.patchObject("c-1", { status: "resolved" }); });

    const resolved = result.current.objects.get("c-1") as CommentObject;
    expect(resolved.status).toBe("resolved");
  });

  it("updates redaction status", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeRedaction({ id: "r-1", status: "draft" });
    act(() => { result.current.addObject(obj); });
    act(() => { result.current.patchObject("r-1", { status: "applied" }); });

    const applied = result.current.objects.get("r-1") as RedactionObject;
    expect(applied.status).toBe("applied");
  });

  it("undo/redo comment edit", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeComment({ id: "c-1", contents: "First" });
    act(() => { result.current.addObject(obj); });
    act(() => { result.current.patchObject("c-1", { contents: "Second" }); });

    act(() => { result.current.undo(); });
    expect((result.current.objects.get("c-1") as CommentObject).contents).toBe("First");

    act(() => { result.current.redo(); });
    expect((result.current.objects.get("c-1") as CommentObject).contents).toBe("Second");
  });

  it("undo/redo delete", () => {
    const { result } = renderHook(() => usePdfEditorState());
    const obj = makeTextBox({ id: "obj-1" });
    act(() => { result.current.addObject(obj); });
    act(() => { result.current.deleteObject("obj-1"); });
    expect(result.current.objects.size).toBe(0);

    act(() => { result.current.undo(); });
    expect(result.current.objects.size).toBe(1);

    act(() => { result.current.redo(); });
    expect(result.current.objects.size).toBe(0);
  });

  it("selects in replace mode", () => {
    const { result } = renderHook(() => usePdfEditorState());
    act(() => { result.current.addObject(makeTextBox({ id: "a" })); });
    act(() => { result.current.addObject(makeTextBox({ id: "b" })); });
    act(() => { result.current.selectObject("a", "replace"); });
    expect(result.current.selectedIds).toEqual(["a"]);
  });

  it("updates object rect", () => {
    const { result } = renderHook(() => usePdfEditorState());
    act(() => { result.current.addObject(makeTextBox({ id: "obj-1" })); });
    act(() => { result.current.updateObjectRect("obj-1", { x: 50, y: 50, width: 200, height: 80 }); });
    expect(result.current.objects.get("obj-1")!.rect).toEqual({ x: 50, y: 50, width: 200, height: 80 });
  });

  it("does not move locked objects", () => {
    const { result } = renderHook(() => usePdfEditorState());
    act(() => { result.current.addObject(makeTextBox({ id: "obj-1", locked: true, rect: { x: 10, y: 20, width: 100, height: 50 } })); });
    act(() => { result.current.updateObjectRect("obj-1", { x: 999, y: 999, width: 100, height: 50 }); });
    expect(result.current.objects.get("obj-1")!.rect.x).toBe(10);
  });

  it("objectsForPage filters correctly", () => {
    const { result } = renderHook(() => usePdfEditorState());
    act(() => { result.current.addObject(makeTextBox({ id: "a", sessionId: "s1", pageIndex: 0 })); });
    act(() => { result.current.addObject(makeTextBox({ id: "b", sessionId: "s1", pageIndex: 1 })); });
    act(() => { result.current.addObject(makeComment({ id: "c", sessionId: "s1", pageIndex: 0 })); });
    const page0 = result.current.objectsForPage("s1", 0);
    expect(page0.length).toBe(2);
  });
});
