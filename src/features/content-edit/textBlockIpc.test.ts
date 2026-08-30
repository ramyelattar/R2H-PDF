import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import {
  pdfPrepareTextBlockEdit,
  pdfApplyTextBlockEdit,
  type TextBlock,
  type TextBlockEditResult,
} from "../../lib/ipc";

describe("Phase 28D — text block IPC contract", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("pdfPrepareTextBlockEdit returns the list of blocks", async () => {
    const blocks: TextBlock[] = [
      {
        block_id: "blk-0-0",
        session_id: "s1",
        page_index: 0,
        bbox: [10, 100, 200, 130],
        member_ids: ["co-1", "co-2"],
        combined_text: "Line A\nLine B",
        font_size: 12,
        all_native_editable: true,
        diagnostics: ["2 member span(s) grouped."],
      },
    ];
    vi.mocked(invoke).mockResolvedValueOnce(blocks);

    const res = await pdfPrepareTextBlockEdit("s1", 0);
    expect(invoke).toHaveBeenCalledWith("pdf_prepare_text_block_edit", {
      sessionId: "s1", pageIndex: 0,
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data).toHaveLength(1);
      expect(res.data[0].block_id).toBe("blk-0-0");
      expect(res.data[0].all_native_editable).toBe(true);
    }
  });

  it("pdfApplyTextBlockEdit forwards the request and parses the result", async () => {
    const result: TextBlockEditResult = {
      edit_id: "blk-edit-1",
      session_id: "s1",
      page_index: 0,
      block_id: "blk-0-0",
      method: "native_multi_operator",
      edited_object_ids: ["co-1", "co-2"],
      before_text: "Line A\nLine B",
      after_text: "Line A2\nLine B2",
      success: true,
      warnings: ["Native multi-operator block edit applied. Original font and per-line position preserved."],
    };
    vi.mocked(invoke).mockResolvedValueOnce(result);

    const res = await pdfApplyTextBlockEdit({
      session_id: "s1",
      page_index: 0,
      block_id: "blk-0-0",
      replacement_text: "Line A2\nLine B2",
      strategy: "auto",
      overflow_policy: "shrink_to_fit",
    });
    expect(invoke).toHaveBeenCalledWith("pdf_apply_text_block_edit", {
      request: {
        session_id: "s1",
        page_index: 0,
        block_id: "blk-0-0",
        replacement_text: "Line A2\nLine B2",
        strategy: "auto",
        overflow_policy: "shrink_to_fit",
      },
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.method).toBe("native_multi_operator");
      expect(res.data.success).toBe(true);
    }
  });

  it("pdfApplyTextBlockEdit surfaces rejection without claiming success", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      edit_id: "blk-edit-2",
      session_id: "s1",
      page_index: 0,
      block_id: "blk-0-0",
      method: "rejected",
      edited_object_ids: [],
      before_text: "old",
      after_text: "new",
      success: false,
      warnings: ["Native multi-operator edit requires same line count."],
    } satisfies TextBlockEditResult);

    const res = await pdfApplyTextBlockEdit({
      session_id: "s1",
      page_index: 0,
      block_id: "blk-0-0",
      replacement_text: "new\nextra",
      strategy: "native_multi_operator",
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.success).toBe(false);
      expect(res.data.method).toBe("rejected");
      expect(res.data.warnings[0]).toContain("same line count");
    }
  });
});
