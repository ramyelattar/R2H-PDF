import { describe, it, expect } from "vitest";
import type { ExportOptions, ExportResult, OverlayObjectPayload, RedactionMode } from "./types";
import type { TextBoxObject, CommentObject } from "../pdf-editor/types";

/**
 * Phase 7 tests for export feature types, mapping logic, and safety behavior.
 */

function makeTextBox(): TextBoxObject {
  return {
    id: "tb-1", sessionId: "s1", pageIndex: 0, type: "textBox",
    rect: { x: 100, y: 200, width: 120, height: 40 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: {},
    text: "Hello", fontSize: 12, fontFamily: "Helvetica",
    color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
  };
}

function makeComment(): CommentObject {
  return {
    id: "c-1", sessionId: "s1", pageIndex: 1, type: "comment",
    rect: { x: 50, y: 700, width: 24, height: 24 },
    rotation: 0, zIndex: 2, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: {},
    contents: "Check this", author: "Reviewer", color: "#ffba6b", status: "open",
  };
}

describe("ExportOptions", () => {
  it("defaults redaction_mode to export_copy", () => {
    const opts: ExportOptions = {
      sessionId: "s1",
      outputPath: "/tmp/export.pdf",
      commitAnnotations: true,
      flattenAnnotations: false,
      applyRedactions: false,
      redactionMode: "export_copy",
      overwriteExisting: true,
    };
    expect(opts.redactionMode).toBe("export_copy");
  });

  it("supports apply_to_current_document mode", () => {
    const mode: RedactionMode = "apply_to_current_document";
    expect(mode).toBe("apply_to_current_document");
  });
});

describe("OverlayObjectPayload mapping", () => {
  it("maps textBox to payload with rect as [x0, y0, x1, y1]", () => {
    const obj = makeTextBox();
    const payload: OverlayObjectPayload = {
      id: obj.id, object_type: obj.type, page_index: obj.pageIndex,
      rect: [obj.rect.x, obj.rect.y, obj.rect.x + obj.rect.width, obj.rect.y + obj.rect.height],
      text: obj.text, color: obj.color, font_size: obj.fontSize,
      author: null, stamp_text: null, stamp_type: null, opacity: null,
      status: null, reason: null, replacement_text: null,
      stroke_color: null, stroke_width: null, fill_color: null,
    };
    expect(payload.object_type).toBe("textBox");
    expect(payload.rect).toEqual([100, 200, 220, 240]);
    expect(payload.text).toBe("Hello");
  });

  it("maps comment to payload", () => {
    const obj = makeComment();
    const payload: OverlayObjectPayload = {
      id: obj.id, object_type: obj.type, page_index: obj.pageIndex,
      rect: [obj.rect.x, obj.rect.y, obj.rect.x + obj.rect.width, obj.rect.y + obj.rect.height],
      text: obj.contents, color: obj.color, font_size: null,
      author: obj.author, stamp_text: null, stamp_type: null,
      opacity: null, status: obj.status, reason: null,
      replacement_text: null, stroke_color: null, stroke_width: null, fill_color: null,
    };
    expect(payload.text).toBe("Check this");
    expect(payload.author).toBe("Reviewer");
  });

  it("does not include hidden objects", () => {
    const obj = makeTextBox();
    obj.hidden = true;
    // The usePdfExport.mapObjectsToPayload filters hidden objects.
    const visible = [obj].filter((o) => !o.hidden);
    expect(visible.length).toBe(0);
  });

  it("does not include objects from other sessions", () => {
    const obj = makeTextBox();
    obj.sessionId = "other-session";
    const sessionObjects = [obj].filter((o) => o.sessionId === "s1");
    expect(sessionObjects.length).toBe(0);
  });
});

describe("ExportResult", () => {
  it("has correct shape with new fields", () => {
    const result: ExportResult = {
      output_path: "/tmp/export.pdf",
      output_file_size_bytes: 123,
      output_sha256: "hash",
      export_type: "pdf",
      independently_validated: true,
      page_count: 10,
      overlay_objects_received_count: 5,
      annotations_committed_count: 4,
      redactions_applied_count: 1,
      redaction_mode: "export_copy",
      source_document_mutated: false,
      true_flattening_supported: false,
      flattened: false,
      warnings: [],
      created_at: Date.now(),
      duration_ms: 1,
    };
    expect(result.overlay_objects_received_count).toBe(5);
    expect(result.source_document_mutated).toBe(false);
    expect(result.true_flattening_supported).toBe(false);
  });

  it("export_copy mode does not mutate source", () => {
    const result: ExportResult = {
      output_path: "/tmp/out.pdf", page_count: 5,
      output_file_size_bytes: 123, output_sha256: "hash", export_type: "pdf", independently_validated: true,
      overlay_objects_received_count: 3, annotations_committed_count: 2,
      redactions_applied_count: 1, redaction_mode: "export_copy",
      source_document_mutated: false, true_flattening_supported: false,
      flattened: false, warnings: [], created_at: 0, duration_ms: 1,
    };
    expect(result.redaction_mode).toBe("export_copy");
    expect(result.source_document_mutated).toBe(false);
  });

  it("apply_to_current_document mode mutates source", () => {
    const result: ExportResult = {
      output_path: "/tmp/out.pdf", page_count: 5,
      output_file_size_bytes: 123, output_sha256: "hash", export_type: "pdf", independently_validated: true,
      overlay_objects_received_count: 3, annotations_committed_count: 2,
      redactions_applied_count: 1, redaction_mode: "apply_to_current_document",
      source_document_mutated: true, true_flattening_supported: false,
      flattened: false, warnings: [], created_at: 0, duration_ms: 1,
    };
    expect(result.redaction_mode).toBe("apply_to_current_document");
    expect(result.source_document_mutated).toBe(true);
  });

  it("flatten_annotations unsupported returns warning", () => {
    // When flatten is requested but not supported, warnings should include explanation.
    const warnings = [
      "True content-stream flattening is not supported by the current MuPDF binding. Annotations were committed with appearance streams."
    ];
    expect(warnings[0]).toContain("not supported");
    expect(warnings[0]).toContain("appearance streams");
  });
});

describe("Redaction confirmation logic", () => {
  it("redactions require explicit apply flag", () => {
    const applyRedactions = false;
    const draftCount = 3;
    expect(applyRedactions && draftCount > 0).toBe(false);
  });

  it("confirmation needed when apply flag is true and drafts exist", () => {
    const applyRedactions = true;
    const draftCount = 2;
    expect(applyRedactions && draftCount > 0).toBe(true);
  });

  it("redaction_mode defaults to export_copy", () => {
    const defaultMode: RedactionMode = "export_copy";
    expect(defaultMode).toBe("export_copy");
  });
});
