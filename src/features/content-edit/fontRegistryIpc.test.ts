import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { pdfGetPageFontRegistry, type PageFontRegistry } from "../../lib/ipc";

describe("Phase 29A — page font registry IPC", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("forwards sessionId / pageIndex and parses the registry", async () => {
    const expected: PageFontRegistry = {
      session_id: "s1",
      page_index: 0,
      fonts: [
        {
          resource_name: "F1",
          base_font_name: "Helvetica",
          subtype: "Type1",
          encoding_kind: "win_ansi",
          base_encoding: "WinAnsiEncoding",
          differences_count: 0,
          has_to_unicode: false,
          is_subset: false,
          is_embedded: true,
          is_cid_font: false,
          is_type0: false,
          is_true_type: false,
          is_type1: true,
          is_type3: false,
          can_native_edit_ascii: true,
          can_native_edit_latin1: true,
          can_native_edit_arabic: false,
          can_native_edit_cjk: false,
          unsupported_reasons: [],
        },
      ],
      warnings: [],
    };
    vi.mocked(invoke).mockResolvedValueOnce(expected);
    const res = await pdfGetPageFontRegistry("s1", 0);
    expect(invoke).toHaveBeenCalledWith("pdf_get_page_font_registry", {
      sessionId: "s1", pageIndex: 0,
    });
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.fonts).toHaveLength(1);
      expect(res.data.fonts[0].can_native_edit_latin1).toBe(true);
      expect(res.data.fonts[0].encoding_kind).toBe("win_ansi");
    }
  });

  it("surfaces backend warnings without rejecting", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      session_id: "s1",
      page_index: 3,
      fonts: [],
      warnings: ["Page /Resources has no /Font dictionary."],
    } satisfies PageFontRegistry);
    const res = await pdfGetPageFontRegistry("s1", 3);
    expect(res.ok).toBe(true);
    if (res.ok) {
      expect(res.data.fonts).toHaveLength(0);
      expect(res.data.warnings[0]).toContain("no /Font");
    }
  });
});
