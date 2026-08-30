import { describe, expect, it } from "vitest";
import {
  createOcrEditorObjects,
  isOcrPageResult,
  validateOcrResult,
} from "./ocrContract";
import type { OcrPageResult } from "./types";

const validResult = (): OcrPageResult => ({
  schema_version: 1,
  operation_id: "ocr-op-1",
  document_id: "document-1",
  session_id: "doc-session-1",
  page_index: 0,
  page_width_points: 612,
  page_height_points: 792,
  rotation_degrees: 0,
  text: "Hello",
  blocks: [{
    id: "block-1",
    text: "Hello",
    bbox: { x: 72, y: 700, width: 60, height: 18 },
    confidence: null,
    block_type: "text",
    reading_order: null,
  }],
  confidence: null,
  language: "auto",
  language_source: "requested",
  engine: "PaddleOCR-VL",
  worker_version: "1.0.0",
  model_version: null,
  duration_ms: 42,
  warnings: [],
  status: "completed",
  created_at: 1,
  image_width_px: 1700,
  image_height_px: 2200,
  bbox_coordinate_space: "image_px",
});

describe("OCR result contract", () => {
  it("accepts only validated real geometry and nullable confidence", () => {
    const result = validResult();
    expect(isOcrPageResult(result)).toBe(true);
    expect(validateOcrResult(result)).toEqual({ ok: true, blockCount: 1 });
    expect(result.confidence).toBeNull();
  });

  it("rejects text-only output instead of manufacturing geometry", () => {
    const result = { ...validResult(), blocks: [] };
    expect(isOcrPageResult(result)).toBe(false);
    expect(validateOcrResult(result)).toMatchObject({
      ok: false,
      code: "OCR_GEOMETRY_UNAVAILABLE",
    });
  });

  it("rejects non-finite or out-of-page geometry", () => {
    const result = validResult();
    result.blocks[0].bbox.x = Number.NaN;
    expect(validateOcrResult(result)).toMatchObject({ ok: false, code: "OCR_INVALID_GEOMETRY" });

    const outside = validResult();
    outside.blocks[0].bbox.x = 1660;
    expect(validateOcrResult(outside)).toMatchObject({ ok: false, code: "OCR_INVALID_GEOMETRY" });
  });

  it("rejects untrusted coordinate spaces and non-normalized rotation", () => {
    const wrongSpace = validResult();
    wrongSpace.bbox_coordinate_space = "viewport_px";
    expect(validateOcrResult(wrongSpace)).toMatchObject({ ok: false, code: "OCR_UNSUPPORTED_COORDINATE_SPACE" });

    const wrongRotation = validResult();
    wrongRotation.rotation_degrees = 45;
    expect(validateOcrResult(wrongRotation)).toMatchObject({ ok: false, code: "OCR_INVALID_ROTATION" });
  });

  it("creates deterministic PDF-coordinate overlay objects without session IDs", () => {
    const result = validResult();
    const [object] = createOcrEditorObjects(result, [{
      id: "overlay-source-1",
      page_index: 0,
      text: "Hello",
      bbox: [72, 700, 132, 718],
      confidence: null,
      block_type: "text",
      original_bbox: [200, 200, 366, 250],
      coordinate_space: "image_px",
      conversion_scale: [0.36, 0.36],
    }], 1);

    expect(object.id).toBe("ocr-document-1-p0-block-1");
    expect(object.sessionId).toBe("doc-session-1");
    expect(object.rect).toEqual({ x: 72, y: 700, width: 60, height: 18 });
    expect(object.metadata).toMatchObject({ source: "ocr", ocr_confidence: null });
    expect(object.id).not.toContain("doc-session-1");
  });
});
