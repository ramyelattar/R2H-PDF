import { useCallback, useRef } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { docIncrementalSave, libraryRecordProject, projectSave } from "../lib/ipc";
import { isBackendPdfSession } from "../lib/pdfSession";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { DocumentTab } from "../types/shell";
import type { EditorObject } from "../features/pdf-editor/types";
import { createProjectSaveRequest } from "../features/pdf-editor/projectPersistence";

export type PdfSaveOutcome =
  | { ok: true; status: "project_saved" | "pdf_saved" | "project_and_pdf_saved"; savedTo: string }
  | { ok: false; status: "cancelled" | "validation_failed" | "write_failed" | "partial_failure" | "busy"; error?: string };

interface UsePdfSaveDeps {
  activeTab: DocumentTab | null;
  editorObjects: EditorObject[];
  updateTab: (tabId: string, patch: Partial<DocumentTab>) => void;
  appendDiagnostic: AppendDiagnostic;
  onLibraryUpdated?: (snapshot: import("../types/shell").LibrarySnapshot) => void;
}

export function usePdfSave(deps: UsePdfSaveDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;
  const saveInFlightRef = useRef(false);

  const runSave = useCallback(async (targetPath: string | null, editorObjectsOverride?: EditorObject[]): Promise<PdfSaveOutcome> => {
    if (saveInFlightRef.current) {
      return { ok: false, status: "busy", error: "A save is already in progress." };
    }
    saveInFlightRef.current = true;
    try {
      const { activeTab, editorObjects, updateTab, appendDiagnostic, onLibraryUpdated } = depsRef.current;
      if (!isBackendPdfSession(activeTab)) {
        const message = "Save is only available for opened backend PDF sessions.";
        appendDiagnostic({ level: "WARN", source: "ui", message });
        return { ok: false, status: "validation_failed", error: message };
      }

      const saved = await docIncrementalSave({
        session_id: activeTab.id,
        target_path: targetPath,
        fsync: true,
      });
      if (!saved.ok) {
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `PDF save failed: ${saved.error.message}` });
        return { ok: false, status: "write_failed", error: saved.error.message };
      }

      const projectTab: DocumentTab = {
        ...activeTab,
        sourcePath: saved.data.saved_to,
      };
      const project = await projectSave(createProjectSaveRequest(projectTab, editorObjectsOverride ?? editorObjects));
      if (!project.ok) {
        updateTab(activeTab.id, {
          sourcePath: saved.data.saved_to,
          dirty: true,
          ...(targetPath ? {
            title: saved.data.saved_to.split("/").pop() ?? saved.data.saved_to.split("\\").pop() ?? activeTab.title,
          } : {}),
        });
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Project save failed after PDF save: ${project.error.message}` });
        return { ok: false, status: "partial_failure", error: project.error.message };
      }

      updateTab(activeTab.id, {
        dirty: false,
        lastSavedAt: project.data.saved_at_epoch_ms,
        ...(targetPath ? {
          sourcePath: saved.data.saved_to,
          title: saved.data.saved_to.split("/").pop() ?? saved.data.saved_to.split("\\").pop() ?? activeTab.title,
        } : {}),
      });
      appendDiagnostic({
        level: "INFO",
        source: "ipc",
        message: `Saved PDF and project state: ${saved.data.saved_to} (${saved.data.bytes_written} bytes)`,
      });

      // The project sidecar is authoritative.  Only after that write has
      // succeeded do we update the shell library's project/workspace index.
      // A registry failure is surfaced as an error and never represented as
      // a successful history record.
      const library = await libraryRecordProject({
        source_path: projectTab.sourcePath,
        project_name: projectTab.title,
        workspace_id: projectTab.workspaceId || null,
        page_count: projectTab.totalPages,
      });
      if (library.ok) {
        onLibraryUpdated?.(library.data);
      } else {
        appendDiagnostic({ level: "ERROR", source: "state", message: `Project library update failed after save: ${library.error.message}` });
      }
      return { ok: true, status: "project_and_pdf_saved", savedTo: saved.data.saved_to };
    } finally {
      saveInFlightRef.current = false;
    }
  }, []);

  const handleSave = useCallback(async (editorObjectsOverride?: EditorObject[]) => {
    return runSave(null, editorObjectsOverride);
  }, [runSave]);

  const handleSaveAs = useCallback(async () => {
    const { activeTab, appendDiagnostic } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) {
      const message = "Save As is only available for opened backend PDF sessions.";
      appendDiagnostic({ level: "WARN", source: "ui", message });
      return { ok: false as const, status: "validation_failed" as const, error: message };
    }

    const targetPath = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: activeTab.sourcePath,
    });
    if (!targetPath) return { ok: false as const, status: "cancelled" as const };

    return runSave(targetPath);
  }, [runSave]);

  return { handleSave, handleSaveAs };
}
