import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { pdfCoverPathObject, type CoverPathResult } from "../../lib/ipc";

describe("Phase 26D — pdf_cover_path_object IPC", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("forwards the cover-path request and returns a spec", async () => {
    const expected: CoverPathResult = {
      success: true,
      spec: {
        id: "path-cover-0-123",
        page_index: 0,
        bbox: [100, 100, 300, 150],
        method: "safe_visual_removal",
        diagnostics: ["Cover rectangle drawn over path bbox."],
      },
      warnings: [],
      method_label: "Safe Visual Removal — not true vector point editing",
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);

    const res = await pdfCoverPathObject({
      session_id: "s1",
      page_index: 0,
      content_object_id: "co-12345-0-path-0",
    });

    expect(invoke).toHaveBeenCalledWith("pdf_cover_path_object", {
      request: { session_id: "s1", page_index: 0, content_object_id: "co-12345-0-path-0" },
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.spec?.bbox).toEqual([100, 100, 300, 150]);
      expect(res.data.method_label.toLowerCase()).toContain("not true vector");
    }
  });

  it("returns success=false + warnings when bbox is degenerate", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      success: false,
      spec: null,
      warnings: ["Path bbox is degenerate; the cover rectangle will be a no-op."],
      method_label: "Safe Visual Removal — not true vector point editing",
    } satisfies CoverPathResult);

    const res = await pdfCoverPathObject({
      session_id: "s1", page_index: 0, content_object_id: "co-deg",
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.success).toBe(false);
      expect(res.data.spec).toBeNull();
      expect(res.data.warnings[0]).toMatch(/degenerate/i);
    }
  });

  it("bubbles up backend errors", async () => {
    vi.mocked(invoke).mockRejectedValueOnce(
      "Path object co-missing not found on page 1",
    );
    const res = await pdfCoverPathObject({
      session_id: "s1", page_index: 0, content_object_id: "co-missing",
    });
    expect(res.ok).toBe(false);
    if (!res.ok) expect(res.error.message).toContain("not found");
  });
});
