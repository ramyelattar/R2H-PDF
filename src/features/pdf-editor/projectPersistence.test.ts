import { describe, it, expect } from "vitest";
import {
  createProjectSaveRequest,
  deserializeProject,
  restoreProjectObjects,
  serializeProject,
  SCHEMA_VERSION,
} from "./projectPersistence";
import type { TextBoxObject, CommentObject, HighlightObject, StampObject, RedactionObject, EditorObject } from "./types";
import type { DocumentTab } from "../../types/shell";
import { createOcrEditorObjects } from "../ocr/ocrContract";
import type { OcrPageResult } from "../ocr/types";

function makeTextBox(id = "obj-1"): TextBoxObject {
  return {
    id, sessionId: "s1", pageIndex: 0, type: "textBox",
    rect: { x: 100, y: 200, width: 120, height: 40 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 1000, updatedAt: 1000, metadata: {},
    text: "Hello", fontSize: 12, fontFamily: "Helvetica",
    color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
  };
}

function makeComment(id = "obj-2"): CommentObject {
  return {
    id, sessionId: "s1", pageIndex: 1, type: "comment",
    rect: { x: 50, y: 700, width: 24, height: 24 },
    rotation: 0, zIndex: 2, locked: false, hidden: false,
    createdAt: 2000, updatedAt: 2000, metadata: {},
    contents: "Check this section", author: "Reviewer", color: "#ffba6b", status: "open",
  };
}

function makeHighlight(id = "obj-3"): HighlightObject {
  return {
    id, sessionId: "s1", pageIndex: 0, type: "highlight",
    rect: { x: 72, y: 500, width: 200, height: 14 },
    rotation: 0, zIndex: 3, locked: false, hidden: false,
    createdAt: 3000, updatedAt: 3000, metadata: {},
    color: "rgba(255,255,0,0.4)", opacity: 0.4, contents: "",
  };
}

function makeStamp(id = "obj-4"): StampObject {
  return {
    id, sessionId: "s1", pageIndex: 0, type: "stamp",
    rect: { x: 200, y: 600, width: 150, height: 50 },
    rotation: 0, zIndex: 4, locked: false, hidden: false,
    createdAt: 4000, updatedAt: 4000, metadata: {},
    stampText: "APPROVED", stampType: "APPROVED", color: "#0a0",
  };
}

function makeRedaction(id = "obj-5"): RedactionObject {
  return {
    id, sessionId: "s1", pageIndex: 2, type: "redaction",
    rect: { x: 100, y: 300, width: 200, height: 20 },
    rotation: 0, zIndex: 5, locked: false, hidden: false,
    createdAt: 5000, updatedAt: 5000, metadata: {},
    fillColor: "#000", replacementText: "", reason: "PII", status: "draft",
  };
}

describe("serializeProject", () => {
  it("produces valid JSON with schema version", () => {
    const json = serializeProject(
      { path: "/test.pdf", fingerprint: "abc123", pageCount: 10 },
      [makeTextBox()],
    );
    const parsed = JSON.parse(json);
    expect(parsed.schemaVersion).toBe(SCHEMA_VERSION);
    expect(parsed.app).toBe("R2H PDF AI Workstation");
    expect(parsed.editor.objects.length).toBe(1);
  });

  it("preserves all object fields", () => {
    const obj = makeTextBox();
    const json = serializeProject({ path: "", fingerprint: "", pageCount: 0 }, [obj]);
    const parsed = JSON.parse(json);
    const restored = parsed.editor.objects[0];
    expect(restored.id).toBe(obj.id);
    expect(restored.rect).toEqual(obj.rect);
    expect(restored.text).toBe("Hello");
    expect(restored.fontSize).toBe(12);
  });
});

describe("deserializeProject", () => {
  it("round-trips all object types", () => {
    const objects: EditorObject[] = [makeTextBox(), makeComment(), makeHighlight(), makeStamp(), makeRedaction()];
    const json = serializeProject({ path: "/doc.pdf", fingerprint: "xyz", pageCount: 5 }, objects);
    const result = deserializeProject(json);
    expect(result).not.toBeNull();
    expect(result!.editor.objects.length).toBe(5);
    expect(result!.editor.objects.map((o) => o.type)).toEqual(["textBox", "comment", "highlight", "stamp", "redaction"]);
  });

  it("rejects unsupported schema version", () => {
    const json = JSON.stringify({ schemaVersion: 999, editor: { objects: [] } });
    expect(deserializeProject(json)).toBeNull();
  });

  it("filters out unknown object types safely", () => {
    const json = JSON.stringify({
      schemaVersion: 1,
      app: "test",
      document: { path: "", fingerprint: "", pageCount: 0 },
      editor: { objects: [makeTextBox(), { id: "x", type: "unknownFuture", rect: {} }] },
      audit: [],
    });
    const result = deserializeProject(json);
    expect(result).not.toBeNull();
    expect(result!.editor.objects.length).toBe(1);
    expect(result!.editor.objects[0].type).toBe("textBox");
  });

  it("preserves coordinates through round-trip", () => {
    const obj = makeTextBox();
    obj.rect = { x: 123.456, y: 789.012, width: 55.5, height: 33.3 };
    const json = serializeProject({ path: "", fingerprint: "", pageCount: 0 }, [obj]);
    const result = deserializeProject(json);
    expect(result!.editor.objects[0].rect).toEqual(obj.rect);
  });

  it("returns null for invalid JSON", () => {
    expect(deserializeProject("not json")).toBeNull();
  });
});

describe("production project persistence bridge", () => {
  const tab: DocumentTab = {
    id: "tab-1",
    title: "contract.pdf",
    kind: "pdf",
    workspaceId: "workspace-1",
    sourcePath: "C:/docs/contract.pdf",
    pinned: false,
    dirty: true,
    page: 2,
    totalPages: 5,
    zoom: 1.25,
    rotation: 90,
    loadState: "ready",
  };

  it("builds a canonical save request without persisting the volatile session id", () => {
    const request = createProjectSaveRequest(tab, [makeTextBox()]);
    expect(request.source_path).toBe(tab.sourcePath);
    expect(request.workspace_id).toBe("workspace-1");
    expect(request.page_count).toBe(5);
    expect(request.overlay_objects[0]).toMatchObject({ id: "obj-1", sessionId: "" });
    expect(request.view_state).toEqual({ page: 2, zoom: 1.25, rotation: 90 });
  });

  it("restores persisted overlays into the newly opened backend session", () => {
    const restored = restoreProjectObjects(
      { documents: [{ document_id: "doc-1", overlay_objects: [makeTextBox(), { id: "bad" }] }] },
      "doc-1",
      "session-new",
    );
    expect(restored).toHaveLength(1);
    expect(restored[0].sessionId).toBe("session-new");
    expect(restored[0].id).toBe("obj-1");
  });

  it("round-trips an accepted OCR overlay through the canonical project sidecar shape", () => {
    const result: OcrPageResult = {
      schema_version: 1,
      operation_id: "ocr-op-persist-1",
      document_id: "doc-1",
      session_id: "doc-session-live",
      page_index: 1,
      page_width_points: 612,
      page_height_points: 792,
      rotation_degrees: 90,
      text: "Invoice total",
      blocks: [{
        id: "block-total",
        text: "Invoice total",
        bbox: { x: 200, y: 300, width: 200, height: 40 },
        confidence: null,
        block_type: "text",
        reading_order: 0,
      }],
      confidence: null,
      language: "auto",
      language_source: "requested",
      engine: "PaddleOCR-VL",
      worker_version: "1.2.0",
      model_version: null,
      duration_ms: 10,
      warnings: [],
      status: "completed",
      created_at: 1,
      bbox_coordinate_space: "image_px",
      image_width_px: 1700,
      image_height_px: 2200,
    };
    const [accepted] = createOcrEditorObjects(result, [{
      id: "overlay-total",
      page_index: 1,
      text: "Invoice total",
      bbox: [72, 600, 144, 636],
      confidence: null,
      block_type: "text",
      block_id: "block-total",
      original_bbox: [200, 300, 400, 340],
      coordinate_space: "image_px",
      conversion_scale: [0.36, 0.36],
    }], 1);

    const saveRequest = createProjectSaveRequest(tab, [accepted]);
    expect(saveRequest.overlay_objects[0]).toMatchObject({
      id: "ocr-doc-1-p1-block-total",
      sessionId: "",
      pageIndex: 1,
      rect: { x: 72, y: 600, width: 72, height: 36 },
      text: "Invoice total",
      metadata: {
        source: "ocr",
        ocr_block_id: "block-total",
        ocr_operation_id: "ocr-op-persist-1",
      },
    });
    const restored = restoreProjectObjects(
      { documents: [{ document_id: "doc-1", overlay_objects: saveRequest.overlay_objects }] },
      "doc-1",
      "doc-session-reopened",
    );
    expect(restored[0]).toMatchObject({
      id: "ocr-doc-1-p1-block-total",
      sessionId: "doc-session-reopened",
      rect: { x: 72, y: 600, width: 72, height: 36 },
      text: "Invoice total",
    });
  });
});
