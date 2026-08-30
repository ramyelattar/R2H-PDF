import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn().mockResolvedValue("/tmp/out.pdf"),
}));

import { invoke } from "@tauri-apps/api/core";
import { usePdfExport } from "../export/usePdfExport";
import type { StampObject } from "../pdf-editor/types";

function signatureOverlay(extra: Record<string, unknown> = {}): StampObject {
  return {
    id: "sig-1", sessionId: "s1", pageIndex: 0, type: "stamp",
    rect: { x: 100, y: 100, width: 200, height: 60 },
    rotation: 0, zIndex: 5, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0,
    metadata: {
      source: "signature",
      signature_kind: "visual_image_only",
      image_data_url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=",
      image_label: "alice.png",
      ...extra,
    },
    stampText: "Signature — alice.png",
    stampType: "CUSTOM",
    color: "#0c4ea3",
  };
}

describe("Phase 26A — signature payload threading", () => {
  beforeEach(() => { vi.mocked(invoke).mockReset(); });

  it("forwards image_data_url and source=signature in the export payload", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
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
      signatures_count: 1,
      signatures_embedded_count: 1,
      signatures_failed_count: 0,
      signatures_failed_items: [],
    });

    const { result } = renderHook(() => usePdfExport({
      sessionId: "s1",
      sourcePath: "/tmp/in.pdf",
      appendDiagnostic: () => {},
    }));

    await act(async () => {
      await result.current.exportPdf([signatureOverlay()]);
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "doc_export")?.[1] as {
      request: { overlay_objects: Array<{ source: string | null; image_data_url: string | null }> };
    };
    expect(callArgs).toBeDefined();
    const obj = callArgs.request.overlay_objects[0];
    expect(obj.source).toBe("signature");
    expect(obj.image_data_url).toMatch(/^data:image\/png;base64,/);
  });

  // ─── Phase 27A: preserve_aspect threads through the payload ──────
  it("forwards preserve_aspect=true and natural image dims when metadata is set", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
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
      signatures_count: 1,
      signatures_embedded_count: 1,
      signatures_failed_count: 0,
      signatures_failed_items: [],
      signatures_aspect_preserved_count: 1,
    });

    const { result } = renderHook(() => usePdfExport({
      sessionId: "s1",
      sourcePath: "/tmp/in.pdf",
      appendDiagnostic: () => {},
    }));

    await act(async () => {
      await result.current.exportPdf([
        signatureOverlay({
          preserve_aspect: true,
          image_natural_width: 600,
          image_natural_height: 200,
        }),
      ]);
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "doc_export")?.[1] as {
      request: { overlay_objects: Array<{
        preserve_aspect: boolean | null;
        image_natural_width: number | null;
        image_natural_height: number | null;
      }> };
    };
    expect(callArgs).toBeDefined();
    const obj = callArgs.request.overlay_objects[0];
    expect(obj.preserve_aspect).toBe(true);
    expect(obj.image_natural_width).toBe(600);
    expect(obj.image_natural_height).toBe(200);
  });

  it("forwards preserve_aspect=false when explicitly disabled", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
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
    });

    const { result } = renderHook(() => usePdfExport({
      sessionId: "s1",
      sourcePath: "/tmp/in.pdf",
      appendDiagnostic: () => {},
    }));

    await act(async () => {
      await result.current.exportPdf([signatureOverlay({ preserve_aspect: false })]);
    });

    const callArgs = vi.mocked(invoke).mock.calls.find((c) => c[0] === "doc_export")?.[1] as {
      request: { overlay_objects: Array<{ preserve_aspect: boolean | null }> };
    };
    const obj = callArgs.request.overlay_objects[0];
    expect(obj.preserve_aspect).toBe(false);
  });

  it("loud failure warning surfaces when signatures_failed_count > 0", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      output_path: "/tmp/out.pdf",
      page_count: 1,
      overlay_objects_received_count: 1,
      annotations_committed_count: 1,
      redactions_applied_count: 0,
      redaction_mode: "export_copy",
      source_document_mutated: false,
      true_flattening_supported: false,
      flattened: false,
      warnings: ["Signature sig-1 has no image data; only the Stamp label will appear externally."],
      created_at: 0,
      signatures_count: 1,
      signatures_embedded_count: 0,
      signatures_failed_count: 1,
      signatures_failed_items: ["sig-1 (no image_data_url)"],
    });

    const { result } = renderHook(() => usePdfExport({
      sessionId: "s1",
      sourcePath: "/tmp/in.pdf",
      appendDiagnostic: () => {},
    }));

    let res: { signatures_failed_count?: number; warnings: string[] } | null = null;
    await act(async () => {
      res = await result.current.exportPdf([signatureOverlay()]) as typeof res;
    });
    expect(res).toBeTruthy();
    if (res) {
      expect((res as { signatures_failed_count?: number }).signatures_failed_count).toBe(1);
      expect((res as { warnings: string[] }).warnings[0]).toContain("Stamp label");
    }
  });
});
