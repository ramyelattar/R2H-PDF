import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  aiReviewCompareResult,
  pdfCompareDocuments,
  reportExportCompareReview,
  type DocumentReviewResult,
  type ExportCompareReportResult,
} from "../../lib/ipc";

describe("Phase 24 IPC wrappers — Compare extras", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("threads include_ocr=false and visual_max_pages through pdfCompareDocuments", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      compare_id: "cmp",
      base_document: { session_id: "s", source_path: null, page_count: 1 },
      revised_document: { session_id: null, source_path: "x.pdf", page_count: 1 },
      mode: "combined",
      summary: { pages_compared: 1, pages_with_changes: 0, lines_added: 0, lines_removed: 0, lines_modified: 0, identical: true },
      page_results: [],
      text_changes: [],
      visual_changes: [],
      warnings: [],
      created_at_ms: 0,
    });
    const result = await pdfCompareDocuments({
      base_session_id: "s",
      revised_file_path: "x.pdf",
      mode: "combined",
      include_ocr: false,
      visual_max_pages: 20,
    });
    expect(invoke).toHaveBeenCalledWith("pdf_compare_documents", {
      request: {
        base_session_id: "s",
        revised_file_path: "x.pdf",
        mode: "combined",
        include_ocr: false,
        visual_max_pages: 20,
      },
    });
    expect(result.ok).toBe(true);
  });

  it("reportExportCompareReview returns counts after writing", async () => {
    const expected: ExportCompareReportResult = {
      compare_id: "cmp",
      output_path: "C:/tmp/r.html",
      bytes_written: 5000,
      text_changes_count: 4,
      visual_changes_count: 2,
      pages_compared: 3,
      warnings_count: 1,
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const res = await reportExportCompareReview({
      compare_id: "cmp",
      output_path: "C:/tmp/r.html",
      overwrite_existing: true,
    });

    expect(invoke).toHaveBeenCalledWith("report_export_compare_review", {
      request: { compare_id: "cmp", output_path: "C:/tmp/r.html", overwrite_existing: true },
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.bytes_written).toBe(5000);
      expect(res.data.text_changes_count).toBe(4);
    }
  });

  it("aiReviewCompareResult bubbles up local-AI failure", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(
      "No default local AI model configured. Open Models tab to install one."
    );
    const res = await aiReviewCompareResult({
      compare_id: "cmp",
      session_id: "doc-session-1",
    });
    expect(res.ok).toBe(false);
    if (!res.ok) {
      expect(res.error.message).toContain("Models tab");
    }
  });

  it("aiReviewCompareResult returns DocumentReviewResult shape when AI succeeds", async () => {
    const expected: DocumentReviewResult = {
      review_id: "compare-review-1",
      session_id: "doc-session-1",
      summary: "Two paragraphs were rewritten.",
      findings: [],
      risks: [],
      missing_information: [],
      engineering_findings: [],
      suggested_actions: [{
        action_id: "a1",
        action_type: "add_comment",
        page_index: 0,
        text: "Verify the new pricing table.",
        reason: "Modified line on page 1",
        citations: [],
        confidence: 0.8,
      }],
      citations: [],
      warnings: [],
      elapsed_ms: 1200,
      created_at: 0,
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const res = await aiReviewCompareResult({ compare_id: "cmp" });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.suggested_actions).toHaveLength(1);
      expect(res.data.suggested_actions[0].action_type).toBe("add_comment");
    }
  });
});
