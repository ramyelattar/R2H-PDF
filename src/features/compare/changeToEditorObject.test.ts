import { describe, it, expect } from "vitest";
import type { CompareTextChange, CompareVisualChange } from "../../lib/ipc";
import {
  bulkTextChangesToEditorObjects,
  textChangeToEditorObject,
  visualChangeToEditorObject,
} from "./changeToEditorObject";

const SID = "doc-session-7";

function txt(partial: Partial<CompareTextChange> = {}): CompareTextChange {
  return {
    page_index: 2,
    change_type: "added",
    old_text: null,
    new_text: "the new sentence",
    bbox: null,
    confidence: 0.9,
    citation: "page 3",
    ...partial,
  };
}

describe("textChangeToEditorObject (Phase 24A)", () => {
  it("maps a text change to a comment with compare metadata", () => {
    const obj = textChangeToEditorObject(SID, txt(), "comment");
    expect(obj.type).toBe("comment");
    expect(obj.sessionId).toBe(SID);
    expect(obj.pageIndex).toBe(2);
    expect(obj.metadata.source).toBe("compare");
    expect(obj.metadata.change_type).toBe("added");
    // Comment always has author "Compare"
    if (obj.type === "comment") {
      expect(obj.author).toBe("Compare");
      expect(obj.contents).toContain("ADDED");
      expect(obj.contents).toContain("the new sentence");
    }
  });

  it("falls back to a page-level comment when bbox is missing (highlight requested)", () => {
    // 24A rule: missing bbox creates a page-level comment with a clear message.
    const obj = textChangeToEditorObject(SID, txt({ bbox: null }), "highlight");
    expect(obj.type).toBe("comment");
    if (obj.type === "comment") {
      expect(obj.contents.toLowerCase()).toContain("no bounding box");
    }
  });

  it("produces a highlight when bbox is present and kind=highlight", () => {
    const obj = textChangeToEditorObject(
      SID,
      txt({ bbox: [10, 20, 100, 40] }),
      "highlight",
    );
    expect(obj.type).toBe("highlight");
    expect(obj.rect).toEqual({ x: 10, y: 20, width: 90, height: 20 });
    expect(obj.metadata.source).toBe("compare");
  });

  it("rejects degenerate bbox and falls back to default rect", () => {
    const obj = textChangeToEditorObject(SID, txt({ bbox: [50, 50, 50, 50] }), "highlight");
    // Degenerate bbox is treated as "no bbox" → comment fallback.
    expect(obj.type).toBe("comment");
  });
});

describe("visualChangeToEditorObject (Phase 24B)", () => {
  function vis(partial: Partial<CompareVisualChange> = {}): CompareVisualChange {
    return {
      page_index: 1,
      change_type: "visual_modified",
      bbox: [100, 100, 300, 200],
      confidence: 0.5,
      ...partial,
    };
  }

  it("maps visual change to a rectangle with compare_visual provenance", () => {
    const obj = visualChangeToEditorObject(SID, vis());
    expect(obj.type).toBe("rectangle");
    expect(obj.pageIndex).toBe(1);
    expect(obj.metadata.source).toBe("compare_visual");
    expect(obj.metadata.confidence).toBe(0.5);
    expect(obj.rect).toEqual({ x: 100, y: 100, width: 200, height: 100 });
  });
});

describe("textChangeToEditorObject — Phase 27C redline line_bboxes", () => {
  it("threads bbox into metadata.line_bboxes for redline strikeout (removed)", () => {
    const obj = textChangeToEditorObject(
      SID,
      txt({ bbox: [10, 20, 100, 40], change_type: "removed", old_text: "drop me" }),
      "redline",
    );
    expect(obj.type).toBe("strikethrough");
    const meta = obj.metadata as { line_bboxes?: number[][] };
    expect(Array.isArray(meta.line_bboxes)).toBe(true);
    expect(meta.line_bboxes![0]).toEqual([10, 20, 100, 40]);
  });

  it("threads bbox into metadata.line_bboxes for redline underline (added)", () => {
    const obj = textChangeToEditorObject(
      SID,
      txt({ bbox: [15, 30, 200, 60], change_type: "added", new_text: "added line" }),
      "redline",
    );
    expect(obj.type).toBe("underline");
    const meta = obj.metadata as { line_bboxes?: number[][] };
    expect(meta.line_bboxes![0]).toEqual([15, 30, 200, 60]);
  });

  it("omits metadata.line_bboxes when no bbox is provided (heuristic fallback path)", () => {
    const obj = textChangeToEditorObject(SID, txt({ bbox: null }), "redline");
    // Without a bbox we fall back to comment, but if a future caller bypasses
    // the fallback we'd still have no line_bboxes — assert that here.
    expect(obj.metadata).toBeDefined();
    const meta = obj.metadata as { line_bboxes?: number[][] };
    expect(meta.line_bboxes).toBeUndefined();
  });
});

describe("bulkTextChangesToEditorObjects (Phase 24A)", () => {
  it("converts every change to an editor object", () => {
    const changes: CompareTextChange[] = [
      txt({ change_type: "added", new_text: "one" }),
      txt({ change_type: "removed", old_text: "two", new_text: null }),
      txt({ change_type: "modified", old_text: "three-old", new_text: "three-new" }),
    ];
    const objs = bulkTextChangesToEditorObjects(SID, changes, "comment");
    expect(objs).toHaveLength(3);
    expect(objs.every((o) => o.metadata.source === "compare")).toBe(true);
    expect(objs.every((o) => o.type === "comment")).toBe(true);
  });
});
