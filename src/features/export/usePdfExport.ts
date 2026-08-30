import { useCallback, useRef, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { invokeSafe, libraryRecordExport } from "../../lib/ipc";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { EditorObject } from "../pdf-editor/types";
import type { ExportResult, ExportStatus, OverlayObjectPayload, RedactionMode } from "./types";

interface UsePdfExportDeps {
  sessionId: string;
  sourcePath: string;
  appendDiagnostic: AppendDiagnostic;
  onLibraryUpdated?: (snapshot: import("../../types/shell").LibrarySnapshot) => void;
}

export function usePdfExport(deps: UsePdfExportDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [status, setStatus] = useState<ExportStatus>("idle");
  const [lastResult, setLastResult] = useState<ExportResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const inFlightRef = useRef(false);

  const mapObjectsToPayload = useCallback((objects: EditorObject[]): OverlayObjectPayload[] => {
    return objects
      .filter((obj) => !obj.hidden)
      .map((obj) => ({
        id: obj.id,
        object_type: obj.type,
        page_index: obj.pageIndex,
        rect: [obj.rect.x, obj.rect.y, obj.rect.x + obj.rect.width, obj.rect.y + obj.rect.height] as [number, number, number, number],
        text: "text" in obj ? (obj as { text: string }).text : ("contents" in obj ? (obj as { contents: string }).contents : null),
        color: "color" in obj ? (obj as { color: string }).color : null,
        font_size: "fontSize" in obj ? (obj as { fontSize: number }).fontSize : null,
        author: "author" in obj ? (obj as { author: string }).author : null,
        stamp_text: "stampText" in obj ? (obj as { stampText: string }).stampText : null,
        stamp_type: "stampType" in obj ? (obj as { stampType: string }).stampType : null,
        // Phase 27E — read opacity from typed field OR from metadata.opacity
        // (used by path covers that re-use ShapeObject which has no opacity field).
        opacity:
          "opacity" in obj
            ? (obj as { opacity: number }).opacity
            : typeof obj.metadata?.opacity === "number"
              ? (obj.metadata.opacity as number)
              : null,
        status: "status" in obj ? (obj as { status: string }).status : null,
        reason: "reason" in obj ? (obj as { reason: string }).reason : null,
        replacement_text: "replacementText" in obj ? (obj as { replacementText: string }).replacementText : null,
        stroke_color: "strokeColor" in obj ? (obj as { strokeColor: string }).strokeColor : null,
        stroke_width: "strokeWidth" in obj ? (obj as { strokeWidth: number }).strokeWidth : null,
        fill_color: "fillColor" in obj ? (obj as { fillColor: string }).fillColor : null,
        // Phase 24G — provenance tag persisted in metadata.source by 24A/B/D.
        source: typeof obj.metadata?.source === "string" ? (obj.metadata.source as string) : null,
        // Phase 26A — image data URL for visual signature embedding.
        image_data_url:
          typeof obj.metadata?.image_data_url === "string"
            ? (obj.metadata.image_data_url as string)
            : null,
        // Phase 27A — preserve-aspect signal for signature image draw rect.
        preserve_aspect:
          typeof obj.metadata?.preserve_aspect === "boolean"
            ? (obj.metadata.preserve_aspect as boolean)
            : null,
        image_natural_width:
          typeof obj.metadata?.image_natural_width === "number"
            ? (obj.metadata.image_natural_width as number)
            : null,
        image_natural_height:
          typeof obj.metadata?.image_natural_height === "number"
            ? (obj.metadata.image_natural_height as number)
            : null,
      }));
  }, []);

  const exportPdf = useCallback(async (
    objects: EditorObject[],
    options: {
      commitAnnotations?: boolean;
      flattenAnnotations?: boolean;
      applyRedactions?: boolean;
      redactionMode?: RedactionMode;
      overwriteExisting?: boolean;
    } = {},
  ): Promise<ExportResult | null> => {
    const { sessionId, sourcePath, appendDiagnostic, onLibraryUpdated } = depsRef.current;
    if (inFlightRef.current) {
      setError("Export is already running for this document.");
      return null;
    }
    inFlightRef.current = true;
    setStatus("running");
    setError(null);
    try {
      let outputPath: string | null;
      try {
        outputPath = await save({
          filters: [{ name: "PDF", extensions: ["pdf"] }],
          defaultPath: sourcePath.replace(/\.pdf$/i, "_export.pdf"),
        });
      } catch (selectionError) {
        const message = selectionError instanceof Error ? selectionError.message : String(selectionError);
        setStatus("failed");
        setError(message);
        appendDiagnostic({ level: "ERROR", source: "ui", message: `Export destination selection failed: ${message}` });
        return null;
      }
      if (!outputPath) {
        setStatus("idle");
        appendDiagnostic({ level: "INFO", source: "ui", message: "Export cancelled" });
        return null;
      }

      appendDiagnostic({ level: "INFO", source: "ipc", message: "Export started" });

      const payload = {
        request: {
          options: {
            session_id: sessionId,
            output_path: outputPath,
            commit_annotations: options.commitAnnotations ?? true,
            flatten_annotations: options.flattenAnnotations ?? false,
            apply_redactions: options.applyRedactions ?? false,
            redaction_mode: options.redactionMode ?? "export_copy",
            overwrite_existing: options.overwriteExisting ?? false,
          },
          overlay_objects: mapObjectsToPayload(objects),
        },
      };

      const result = await invokeSafe<ExportResult>("doc_export", payload);

      if (!result.ok) {
        setStatus("failed");
        setError(result.error.message);
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Export failed: ${result.error.message}` });
        return null;
      }

      setStatus("completed");
      setLastResult(result.data);
      appendDiagnostic({
        level: "INFO",
        source: "ipc",
        message: `Export completed: ${result.data.output_path} (${result.data.output_file_size_bytes} bytes, ${result.data.output_sha256})`,
      });

      for (const w of result.data.warnings) {
        appendDiagnostic({ level: "WARN", source: "ipc", message: `Export: ${w}` });
      }

      // Export history is written only after doc_export has returned its
      // independently validated output metadata.  A library failure is
      // visible but cannot turn an unsuccessful or partial export into a
      // completed history entry.
      const library = await libraryRecordExport({
        source_path: sourcePath,
        destination: result.data.output_path,
        export_type: result.data.export_type,
        timestamp_epoch_ms: result.data.created_at,
        size_bytes: result.data.output_file_size_bytes,
        sha256: result.data.output_sha256,
        page_count: result.data.page_count,
        included_overlays: result.data.overlay_objects_received_count,
        warnings: result.data.warnings,
      });
      if (library.ok) {
        onLibraryUpdated?.(library.data);
      } else {
        appendDiagnostic({ level: "ERROR", source: "state", message: `Export history update failed after validated export: ${library.error.message}` });
      }

      return result.data;
    } finally {
      inFlightRef.current = false;
    }
  }, [mapObjectsToPayload]);

  return { status, lastResult, error, exportPdf, mapObjectsToPayload };
}
