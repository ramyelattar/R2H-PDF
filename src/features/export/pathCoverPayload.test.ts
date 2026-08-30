import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn().mockResolvedValue("/tmp/out.pdf"),
}));

import { invoke } from "@tauri-apps/api/core";
import { usePdfExport } from "./usePdfExport";
import type { ShapeObject } from "../pdf-editor/types";

function pathCoverOverlay(opts: { color: string; opacity: number }): ShapeObject {
  return {
    id: "pc-1", sessionId: "s1", pageIndex: 0, type: "rectangle",
    rect: { x: 10, y: 20, width: 100, height: 30 },
    rotation: 0, zIndex: 10, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0,
    metadata: {
      source: "path_cover",
      method: "safe_visual_replacement",
      cover_color: opts.color,
      cover_opacity: opts.opacity,
      opacity: opts.opacity,
    },
    strokeColor: "transparent",
    strokeWidth: 0,
    fillColor: opts.color,
  };
}

describe("Phase 27E — path-cover styling threads into the export payload", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({
      output_path: "/tmp/out.pdf",
      page_count: 1,
      overlay_objects_received_count: 1,
      annotations_committed_count: 1,
      redactions_applied_count: 0,
      redaction_mode: "export_copy",
      source_document_mutated: false,
      true_flattening_supported: false,
      flattened: false,
      warnings: [],
      created_at: 0,
      path_covers_count: 1,
    });
  });

  it("forwards source=path_cover, custom color, and opacity through metadata.opacity", async () => {
    const { result } = renderHook(() =>
      usePdfExport({
        sessionId: "s1",
        sourcePath: "/tmp/in.pdf",
        appendDiagnostic: () => {},
      }),
    );

    const overlay = pathCoverOverlay({ color: "#ff8800", opacity: 0.7 });
    await act(async () => {
      await result.current.exportPdf([overlay]);
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "doc_export")?.[1] as {
      request: { overlay_objects: Array<{
        source: string | null;
        fill_color: string | null;
        opacity: number | null;
      }> };
    };
    expect(callArgs).toBeDefined();
    const obj = callArgs.request.overlay_objects[0];
    expect(obj.source).toBe("path_cover");
    expect(obj.fill_color).toBe("#ff8800");
    // Phase 27E — opacity should come either from typed field or
    // metadata.opacity; ShapeObject has no typed opacity so metadata wins.
    expect(obj.opacity).toBeCloseTo(0.7, 3);
  });
});
