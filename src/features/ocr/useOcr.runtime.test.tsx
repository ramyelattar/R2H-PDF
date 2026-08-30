import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  ocrCancel,
  ocrCheckAvailability,
  ocrHasResult,
  ocrRunPage,
} from "../../lib/ipc";
import { useOcr } from "./useOcr";
import type { OcrAvailability, OcrPageResult } from "./types";

vi.mock("../../lib/ipc", () => ({
  ocrCancel: vi.fn(),
  ocrCheckAvailability: vi.fn(),
  ocrHasResult: vi.fn(),
  ocrRunPage: vi.fn(),
}));

const ready: OcrAvailability = {
  status: "available",
  available: true,
  message: "OCR is ready.",
  resource_source: "external_local_ai_pack",
  model_id: "PaddleOCR-VL",
  missing_assets: [],
};

const result = (overrides: Partial<OcrPageResult> = {}): OcrPageResult => ({
  schema_version: 1,
  operation_id: "ocr-op-test",
  document_id: "document-test",
  session_id: "doc-session-test",
  page_index: 0,
  page_width_points: 612,
  page_height_points: 792,
  rotation_degrees: 0,
  text: "Hello scanned page",
  blocks: [{
    id: "block-1",
    text: "Hello scanned page",
    bbox: { x: 20, y: 30, width: 180, height: 32 },
    confidence: null,
    block_type: "text",
    reading_order: null,
  }],
  confidence: null,
  language: "auto",
  language_source: "requested",
  engine: "PaddleOCR-VL",
  worker_version: "1.1.0",
  model_version: null,
  duration_ms: 5,
  warnings: [],
  status: "completed",
  created_at: 1,
  bbox_coordinate_space: "image_px",
  image_width_px: 1224,
  image_height_px: 1584,
  ...overrides,
});

const ok = <T,>(data: T) => ({ ok: true as const, data });

describe("useOcr production controller", () => {
  beforeEach(() => {
    vi.mocked(ocrCheckAvailability).mockReset();
    vi.mocked(ocrRunPage).mockReset();
    vi.mocked(ocrCancel).mockReset();
    vi.mocked(ocrHasResult).mockReset();
    vi.mocked(ocrHasResult).mockResolvedValue(ok(false));
  });

  it("maps missing model availability to blocked and never invokes the worker", async () => {
    vi.mocked(ocrCheckAvailability).mockResolvedValue(ok({
      ...ready,
      status: "missing_model",
      available: false,
      message: "The local OCR model pack is not installed.",
      missing_assets: ["models/ocr/PaddleOCR-VL"],
    }));
    const { result: hook } = renderHook(() => useOcr({ sessionId: "doc-session-test", totalPages: 1, appendDiagnostic: vi.fn() }));

    await act(async () => { await hook.current.checkAvailability(); });
    expect(hook.current.state.status).toBe("blocked");
    expect(hook.current.state.availability?.status).toBe("missing_model");

    await act(async () => { await hook.current.ocrCurrentPage(0); });
    expect(ocrRunPage).not.toHaveBeenCalled();
  });

  it("keeps a valid worker result running/completed state and dispatches a fully scoped request", async () => {
    vi.mocked(ocrCheckAvailability).mockResolvedValue(ok(ready));
    vi.mocked(ocrRunPage).mockResolvedValue(ok(result()));
    const appendDiagnostic = vi.fn();
    const { result: hook } = renderHook(() => useOcr({ sessionId: "doc-session-test", totalPages: 1, appendDiagnostic }));

    await act(async () => { await hook.current.checkAvailability(); });
    let output: OcrPageResult | null = null;
    await act(async () => { output = await hook.current.ocrCurrentPage(0); });

    expect(output?.text).toBe("Hello scanned page");
    expect(hook.current.state.status).toBe("completed");
    expect(hook.current.state.progressCurrent).toBe(1);
    expect(ocrRunPage).toHaveBeenCalledWith(expect.objectContaining({
      session_id: "doc-session-test",
      page_index: 0,
      language: "auto",
      model_id: "PaddleOCR-VL",
      output_format_version: 1,
    }));
  });

  it("maps text-only or malformed results to failed, never completed", async () => {
    vi.mocked(ocrCheckAvailability).mockResolvedValue(ok(ready));
    vi.mocked(ocrRunPage).mockResolvedValue(ok(result({ blocks: [], text: "real model text" })) as never);
    const { result: hook } = renderHook(() => useOcr({ sessionId: "doc-session-test", totalPages: 1, appendDiagnostic: vi.fn() }));

    await act(async () => { await hook.current.checkAvailability(); });
    await act(async () => { await hook.current.ocrCurrentPage(0); });
    expect(hook.current.state.status).toBe("failed");
    expect(hook.current.state.error).toContain("OCR_GEOMETRY_UNAVAILABLE");
  });

  it("reports no_text_detected without creating an accepted result", async () => {
    vi.mocked(ocrCheckAvailability).mockResolvedValue(ok(ready));
    vi.mocked(ocrRunPage).mockResolvedValue(ok(result({ status: "no_text_detected", text: "", blocks: [] })));
    const { result: hook } = renderHook(() => useOcr({ sessionId: "doc-session-test", totalPages: 1, appendDiagnostic: vi.fn() }));

    await act(async () => { await hook.current.checkAvailability(); });
    await act(async () => { await hook.current.ocrCurrentPage(0); });
    expect(hook.current.state.status).toBe("no_text_detected");
    expect(hook.current.state.acceptance).toBe("none");
  });

  it("routes user cancellation to the typed cancel command", async () => {
    vi.mocked(ocrCheckAvailability).mockResolvedValue(ok(ready));
    let resolveRun: ((value: ReturnType<typeof ok>) => void) | undefined;
    vi.mocked(ocrRunPage).mockImplementation(() => new Promise((resolve) => { resolveRun = resolve as never; }) as never);
    vi.mocked(ocrCancel).mockResolvedValue(ok({ operation_id: "ocr-op-test", cancelled: true }));
    const { result: hook } = renderHook(() => useOcr({ sessionId: "doc-session-test", totalPages: 1, appendDiagnostic: vi.fn() }));

    await act(async () => { await hook.current.checkAvailability(); });
    let runPromise: Promise<OcrPageResult | null> | undefined;
    act(() => { runPromise = hook.current.ocrCurrentPage(0); });
    await waitFor(() => expect(ocrRunPage).toHaveBeenCalledTimes(1));
    await act(async () => { await hook.current.cancelOcr(); });
    expect(ocrCancel).toHaveBeenCalledWith(expect.any(String));
    resolveRun?.({ ok: false, error: { code: "OCR_CANCELLED", message: "OCR_CANCELLED" } } as never);
    await act(async () => { await runPromise; });
    expect(hook.current.state.status).toBe("cancelled");
  });
});
