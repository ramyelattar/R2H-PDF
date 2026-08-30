import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  pdfCompareDocuments,
  pdfGetCompareResult,
  pdfClearCompareResult,
  type CompareResult,
} from "../../lib/ipc";

const sampleResult: CompareResult = {
  compare_id: "cmp-123",
  base_document: { session_id: "doc-session-1", source_path: null, page_count: 2 },
  revised_document: { session_id: null, source_path: "C:/tmp/rev.pdf", page_count: 2 },
  mode: "text_only",
  summary: {
    pages_compared: 2,
    pages_with_changes: 1,
    lines_added: 1,
    lines_removed: 1,
    lines_modified: 1,
    identical: false,
  },
  page_results: [
    { page_index: 0, base_line_count: 5, revised_line_count: 5, added: 0, removed: 0, modified: 1 },
    { page_index: 1, base_line_count: 3, revised_line_count: 4, added: 1, removed: 1, modified: 0 },
  ],
  text_changes: [
    { page_index: 0, change_type: "modified", old_text: "old line", new_text: "new line", bbox: null, confidence: 0.66, citation: "page 1" },
    { page_index: 1, change_type: "added", old_text: null, new_text: "fresh line", bbox: null, confidence: 1.0, citation: "page 2" },
    { page_index: 1, change_type: "removed", old_text: "stale line", new_text: null, bbox: null, confidence: 1.0, citation: "page 2" },
  ],
  visual_changes: [],
  warnings: [],
  created_at_ms: 1700000000000,
};

describe("pdfCompareDocuments IPC wrapper", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("invokes pdf_compare_documents with the request wrapped under `request`", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(sampleResult);

    const result = await pdfCompareDocuments({
      base_session_id: "doc-session-1",
      revised_file_path: "C:/tmp/rev.pdf",
      mode: "text_only",
    });

    expect(invoke).toHaveBeenCalledWith("pdf_compare_documents", {
      request: {
        base_session_id: "doc-session-1",
        revised_file_path: "C:/tmp/rev.pdf",
        mode: "text_only",
      },
    });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.compare_id).toBe("cmp-123");
      expect(result.data.summary.identical).toBe(false);
    }
  });

  it("returns added/removed/modified change types in the response", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(sampleResult);
    const result = await pdfCompareDocuments({
      base_session_id: "s",
      revised_file_path: "x.pdf",
    });
    expect(result.ok).toBe(true);
    if (result.ok) {
      const types = result.data.text_changes.map((c) => c.change_type).sort();
      expect(types).toEqual(["added", "modified", "removed"]);
    }
  });

  it("returns an identical-result when both docs match", async () => {
    const identical: CompareResult = {
      ...sampleResult,
      compare_id: "cmp-id-match",
      summary: { pages_compared: 1, pages_with_changes: 0, lines_added: 0, lines_removed: 0, lines_modified: 0, identical: true },
      page_results: [{ page_index: 0, base_line_count: 1, revised_line_count: 1, added: 0, removed: 0, modified: 0 }],
      text_changes: [],
    };
    vi.mocked(invoke).mockResolvedValueOnce(identical);

    const result = await pdfCompareDocuments({ base_session_id: "s", revised_file_path: "x.pdf" });
    expect(result.ok).toBe(true);
    if (result.ok) {
      expect(result.data.summary.identical).toBe(true);
      expect(result.data.text_changes).toHaveLength(0);
    }
  });

  it("returns an error envelope when the backend rejects an invalid path", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("revised PDF not found at path: /nope.pdf");
    const result = await pdfCompareDocuments({
      base_session_id: "s",
      revised_file_path: "/nope.pdf",
    });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.message).toContain("not found");
    }
  });
});

describe("pdfGetCompareResult & pdfClearCompareResult", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("retrieves a stored result by id", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(sampleResult);
    const result = await pdfGetCompareResult("cmp-123");
    expect(invoke).toHaveBeenCalledWith("pdf_get_compare_result", { compareId: "cmp-123" });
    expect(result.ok).toBe(true);
    if (result.ok && result.data) {
      expect(result.data.compare_id).toBe("cmp-123");
    }
  });

  it("returns null when the result has been cleared", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(null);
    const result = await pdfGetCompareResult("missing");
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.data).toBeNull();
  });

  it("clears a compare result", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(true);
    const result = await pdfClearCompareResult("cmp-123");
    expect(invoke).toHaveBeenCalledWith("pdf_clear_compare_result", { compareId: "cmp-123" });
    expect(result.ok).toBe(true);
    if (result.ok) expect(result.data).toBe(true);
  });
});
