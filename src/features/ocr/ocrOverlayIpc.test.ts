import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  ocrCancel,
  ocrCheckAvailability,
  ocrRunPage,
  pdfCreateOcrEditableOverlays,
  pdfCreateOcrTextLayer,
  type OcrAvailability,
  type OcrOverlayResult,
  type OcrPageRequest,
  type OcrTextLayerStatus,
} from "../../lib/ipc";

describe("Phase 25D — OCR overlay IPC", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("uses typed OCR availability and run commands", async () => {
    const availability: OcrAvailability = {
      status: "available",
      available: true,
      message: "OCR is ready.",
      resource_source: "external_local_ai_pack",
      model_id: "PaddleOCR-VL",
      missing_assets: [],
    };
    const request: OcrPageRequest = {
      operation_id: "ocr-op-1",
      session_id: "doc-session-1",
      page_index: 0,
      force: false,
      dpi: 200,
      language: "auto",
      model_id: "PaddleOCR-VL",
      output_format_version: 1,
      timeout_secs: 120,
      cancellation_id: "ocr-op-1",
    };
    vi.mocked(invoke)
      .mockResolvedValueOnce(availability)
      .mockResolvedValueOnce({ ...availability, status: "available", available: true });

    const availabilityResult = await ocrCheckAvailability();
    const runResult = await ocrRunPage(request);

    expect(invoke).toHaveBeenNthCalledWith(1, "ocr_check_availability", undefined);
    expect(invoke).toHaveBeenNthCalledWith(2, "ocr_run_page", { request });
    expect(availabilityResult.ok).toBe(true);
    expect(runResult.ok).toBe(true);
  });

  it("routes cancellation through the typed command", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ cancelled: true, operation_id: "ocr-op-1" });
    const result = await ocrCancel("ocr-op-1");
    expect(invoke).toHaveBeenCalledWith("ocr_cancel", { request: { operation_id: "ocr-op-1" } });
    expect(result.ok).toBe(true);
  });

  it("pdfCreateOcrEditableOverlays returns overlay specs + warnings", async () => {
      const expected: OcrOverlayResult = {
        document_id: "document-s1",
        session_id: "s1", page_index: 0,
        status: "completed",
      overlays: [{
        id: "ocr-ov-s1-0-0",
        page_index: 0,
        text: "Hello scanned page",
        bbox: [10, 700, 200, 720],
        confidence: 0.92,
        block_type: "paragraph",
      }],
      warnings: [
        "Editable OCR overlay — not native original text. Bbox was validated in PDF coordinates.",
      ],
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const res = await pdfCreateOcrEditableOverlays({ session_id: "s1", page_index: 0 });

    expect(invoke).toHaveBeenCalledWith("pdf_create_ocr_editable_overlays", {
      request: { session_id: "s1", page_index: 0 },
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.overlays).toHaveLength(1);
      expect(res.data.warnings.some((w) => w.includes("not native original text"))).toBe(true);
    }
  });

  it("pdfCreateOcrEditableOverlays bubbles up missing-cache error", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("No OCR result cached for page 3. Run OCR first.");
    const res = await pdfCreateOcrEditableOverlays({ session_id: "s1", page_index: 2 });
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error.message).toContain("Run OCR first");
  });

  it("pdfCreateOcrTextLayer returns honest not_implemented status", async () => {
    const expected: OcrTextLayerStatus = {
      session_id: "s1", page_index: 0,
      status: "not_implemented",
      overlays_available: true,
      note: "True invisible PDF text-layer injection is NOT implemented in this build. Use 'Create Editable OCR Overlays' instead.",
      warnings: [
        "OCR text-layer command intentionally returns not_implemented; no PDF bytes were modified.",
        "Use 'Create Editable OCR Overlays' for visual editable annotations; they are not embedded searchable PDF text.",
      ],
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);
    const res = await pdfCreateOcrTextLayer({ session_id: "s1", page_index: 0 });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.status).toBe("not_implemented");
      expect(res.data.note.toLowerCase()).toContain("editable ocr overlays");
    }
  });
});
