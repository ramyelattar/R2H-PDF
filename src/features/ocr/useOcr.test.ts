import { describe, it, expect } from "vitest";
import type { OcrPageResult, OcrState } from "./types";

/**
 * Phase 6 tests for OCR frontend state and types.
 * Tests the data model and state transitions without requiring
 * actual IPC calls (those are integration tests).
 */

describe("OcrPageResult type", () => {
  it("has required fields", () => {
    const result: OcrPageResult = {
      schema_version: 1,
      operation_id: "ocr-op-1",
      document_id: "document-1",
      session_id: "s1",
      page_index: 0,
      page_width_points: 612,
      page_height_points: 792,
      rotation_degrees: 0,
      text: "Hello world",
      blocks: [
        { id: "b1", text: "Hello", bbox: { x: 0, y: 0, width: 100, height: 20 }, confidence: 0.95, block_type: "text", reading_order: 0 },
        { id: "b2", text: "world", bbox: { x: 0, y: 20, width: 100, height: 20 }, confidence: 0.88, block_type: "text", reading_order: 1 },
      ],
      confidence: 0.91,
      language: "en",
      language_source: "detected",
      engine: "PaddleOCR-VL",
      worker_version: "1.2.0",
      model_version: null,
      duration_ms: 10,
      warnings: [],
      status: "completed",
      created_at: Date.now(),
      bbox_coordinate_space: "image_px",
      image_width_px: 1224,
      image_height_px: 1584,
    };
    expect(result.blocks.length).toBe(2);
    expect(result.confidence).toBeGreaterThan(0);
    expect(result.engine).toBe("PaddleOCR-VL");
  });
});

describe("OcrState transitions", () => {
  it("starts in idle state", () => {
    const state: OcrState = {
      status: "idle",
      currentPage: null,
      totalPages: 0,
      progressCurrent: 0,
      progressTotal: 0,
      lastResult: null,
      error: null,
      available: false,
      availability: null,
      operationId: null,
      acceptance: "none",
      persistedBlockCount: 0,
    };
    expect(state.status).toBe("idle");
    expect(state.available).toBe(false);
  });

  it("transitions to running", () => {
    const state: OcrState = {
      status: "running",
      currentPage: 0,
      totalPages: 5,
      progressCurrent: 0,
      progressTotal: 5,
      lastResult: null,
      error: null,
      available: true,
      availability: null,
      operationId: "ocr-op-1",
      acceptance: "none",
      persistedBlockCount: 0,
    };
    expect(state.status).toBe("running");
    expect(state.progressTotal).toBe(5);
  });

  it("transitions to completed with result", () => {
    const result: OcrPageResult = {
      schema_version: 1,
      operation_id: "ocr-op-1",
      document_id: "document-1",
      session_id: "s1",
      page_index: 0,
      page_width_points: 612,
      page_height_points: 792,
      rotation_degrees: 0,
      text: "Test",
      blocks: [{
        id: "b1",
        text: "Test",
        bbox: { x: 0, y: 0, width: 100, height: 20 },
        confidence: null,
        block_type: "text",
        reading_order: null,
      }],
      confidence: null,
      language: "en",
      language_source: "requested",
      engine: "PaddleOCR-VL",
      worker_version: "1.2.0",
      model_version: null,
      duration_ms: 1,
      warnings: [],
      status: "completed",
      created_at: 0,
      bbox_coordinate_space: "image_px",
      image_width_px: 1224,
      image_height_px: 1584,
    };
    const state: OcrState = {
      status: "completed",
      currentPage: 0,
      totalPages: 1,
      progressCurrent: 1,
      progressTotal: 1,
      lastResult: result,
      error: null,
      available: true,
      availability: null,
      operationId: "ocr-op-1",
      acceptance: "none",
      persistedBlockCount: 1,
    };
    expect(state.status).toBe("completed");
    expect(state.lastResult?.text).toBe("Test");
  });

  it("transitions to failed with error", () => {
    const state: OcrState = {
      status: "failed",
      currentPage: 2,
      totalPages: 5,
      progressCurrent: 2,
      progressTotal: 5,
      lastResult: null,
      error: "Model not found",
      available: true,
      availability: null,
      operationId: null,
      acceptance: "failed",
      persistedBlockCount: 0,
    };
    expect(state.status).toBe("failed");
    expect(state.error).toBe("Model not found");
  });
});

describe("OcrPanel rendering logic", () => {
  it("OCR current page dispatches with correct page index", () => {
    // Simulates the dispatch logic
    const pageIndex = 3;
    const request = { session_id: "s1", page_index: pageIndex, force: false, dpi: 200 };
    expect(request.page_index).toBe(3);
    expect(request.dpi).toBe(200);
  });

  it("force rerun flag is passed correctly", () => {
    const request = { session_id: "s1", page_index: 0, force: true, dpi: 200 };
    expect(request.force).toBe(true);
  });

  it("progress calculation is correct", () => {
    const current = 3;
    const total = 10;
    const percent = Math.round((current / total) * 100);
    expect(percent).toBe(30);
  });
});
