import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { pdfCompareDocuments } from "../../lib/ipc";

describe("Phase 26C — Compare auto-OCR / include-OCR wiring", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("threads auto_ocr_scanned=true and include_ocr=true to the backend", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      compare_id: "cmp-1",
      base_document: { session_id: "s", source_path: null, page_count: 1 },
      revised_document: { session_id: null, source_path: "x.pdf", page_count: 1 },
      mode: "text_only",
      summary: { pages_compared: 1, pages_with_changes: 0, lines_added: 0, lines_removed: 0, lines_modified: 0, identical: true },
      page_results: [],
      text_changes: [],
      visual_changes: [],
      warnings: [],
      created_at_ms: 0,
    });

    await pdfCompareDocuments({
      base_session_id: "s",
      revised_file_path: "x.pdf",
      mode: "text_only",
      include_ocr: true,
      auto_ocr_scanned: true,
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "pdf_compare_documents")?.[1] as {
      request: { include_ocr: boolean; auto_ocr_scanned: boolean };
    };
    expect(callArgs.request.include_ocr).toBe(true);
    expect(callArgs.request.auto_ocr_scanned).toBe(true);
  });

  it("allows include_ocr=false (turning OCR off entirely)", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      compare_id: "cmp-2",
      base_document: { session_id: "s", source_path: null, page_count: 1 },
      revised_document: { session_id: null, source_path: "x.pdf", page_count: 1 },
      mode: "text_only",
      summary: { pages_compared: 1, pages_with_changes: 0, lines_added: 0, lines_removed: 0, lines_modified: 0, identical: true },
      page_results: [],
      text_changes: [],
      visual_changes: [],
      warnings: [],
      created_at_ms: 0,
    });

    await pdfCompareDocuments({
      base_session_id: "s",
      revised_file_path: "x.pdf",
      include_ocr: false,
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "pdf_compare_documents")?.[1] as {
      request: { include_ocr: boolean; auto_ocr_scanned?: boolean };
    };
    expect(callArgs.request.include_ocr).toBe(false);
  });
});
