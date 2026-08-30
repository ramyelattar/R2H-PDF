import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("./useOcr", () => ({
  useOcr: () => ({
    state: {
      status: "completed",
      available: true,
      availability: {
        status: "available",
        available: true,
        message: "OCR is ready.",
        resource_source: "external_local_ai_pack",
        model_id: "PaddleOCR-VL",
        missing_assets: [],
      },
      operationId: null,
      acceptance: "none",
      persistedBlockCount: 0,
      progressCurrent: 0,
      progressTotal: 0,
      currentPage: null,
      error: null,
      lastResult: {
        page_index: 0,
        schema_version: 1,
        operation_id: "ocr-op-1",
        document_id: "document-1",
        session_id: "doc-session-test",
        page_width_points: 612,
        page_height_points: 792,
        rotation_degrees: 0,
        engine: "PaddleOCR-VL",
        confidence: 0.91,
        text: "Panel schedule",
        blocks: [
          { id: "b1", block_type: "text", text: "Panel schedule", bbox: { x: 0, y: 0, width: 10, height: 10 }, confidence: 0.91, reading_order: 0 },
        ],
        warnings: ["Low contrast"],
        created_at: 0,
        language_source: "requested",
        worker_version: "1.2.0",
        model_version: null,
        duration_ms: 1,
        bbox_coordinate_space: "image_px",
        image_width_px: 1000,
        image_height_px: 1000,
        status: "completed",
      },
    },
    checkAvailability: vi.fn(),
    ocrCurrentPage: vi.fn(),
    ocrPageRange: vi.fn(),
    cancelOcr: vi.fn(),
    setAcceptance: vi.fn(),
  }),
}));

vi.mock("../../lib/ipc", () => ({
  pdfCreateOcrEditableOverlays: vi.fn(),
  pdfCreateOcrTextLayer: vi.fn(),
}));

import { OcrPanel } from "./OcrPanel";
import { pdfCreateOcrEditableOverlays } from "../../lib/ipc";

describe("Pass 2B — OcrPanel polish", () => {
  it("renders Run OCR as the primary action, overlay action, index option, and result summary", () => {
    render(
      <OcrPanel
        sessionId="doc-session-test"
        currentPageIndex={0}
        totalPages={2}
        appendDiagnostic={() => {}}
        onAcceptOcrOverlays={vi.fn()}
      />,
    );

    expect(screen.getByTestId("ocr-run-primary").textContent).toMatch(/Run OCR/);
    expect(screen.getByTestId("ocr-create-overlays").textContent).toMatch(/Accept OCR overlays/);
    expect(screen.getByLabelText(/Use OCR in index/)).toBeTruthy();
    expect(screen.getByText(/1 blocks/)).toBeTruthy();
    expect(screen.getByText(/Confidence: 91%/)).toBeTruthy();
  });

  it("does not show persisted acceptance when the canonical App save rejects the overlays", async () => {
    vi.mocked(pdfCreateOcrEditableOverlays).mockResolvedValue({
      ok: true,
      data: {
        document_id: "document-1",
        session_id: "doc-session-test",
        page_index: 0,
        status: "completed",
        overlays: [{
          id: "overlay-1",
          page_index: 0,
          text: "Panel schedule",
          bbox: [0, 0, 10, 10],
          confidence: 0.91,
          block_type: "text",
          block_id: "b1",
          original_bbox: [0, 0, 10, 10],
          coordinate_space: "image_px",
          conversion_scale: [1, 1],
        }],
        warnings: [],
      },
    });
    const accept = vi.fn().mockResolvedValue(false);
    render(
      <OcrPanel
        sessionId="doc-session-test"
        currentPageIndex={0}
        totalPages={2}
        appendDiagnostic={() => {}}
        onAcceptOcrOverlays={accept}
      />,
    );

    fireEvent.click(screen.getByTestId("ocr-create-overlays"));
    await waitFor(() => expect(screen.getByText(/OCR overlays were not persisted/)).toBeTruthy());
    expect(accept).toHaveBeenCalledTimes(1);
    expect(screen.queryByText(/Persisted 1 validated OCR overlay/)).toBeNull();
  });
});
