import { useCallback, useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  docExtractAllText,
  closeSession,
  getSessionState,
  libraryRecordOpen,
  libraryRecordProject,
  openPdf,
  projectLoad,
  searchIndexDocument,
  searchIndexDocumentWithSpans,
} from "../lib/ipc";
import { sessionStateToTabPatch } from "../lib/pdfSession";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { DocumentTab } from "../types/shell";

interface UsePdfOpenDeps {
  activeWorkspaceId: string;
  recentFiles: { path: string }[];
  openTab: (tab: DocumentTab) => void;
  updateTab: (tabId: string, patch: Partial<DocumentTab>) => void;
  appendDiagnostic: AppendDiagnostic;
  onProjectLoaded?: (sessionId: string, project: NonNullable<import("../lib/ipc").ProjectLoadResponse["project"]>, documentId: string) => void;
  onLibraryUpdated?: (snapshot: import("../types/shell").LibrarySnapshot) => void;
}

export function usePdfOpen(deps: UsePdfOpenDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const openPdfPath = useCallback(async (path: string): Promise<string | null> => {
    const { openTab, updateTab, appendDiagnostic, activeWorkspaceId, onLibraryUpdated } = depsRef.current;

    appendDiagnostic({ level: "INFO", source: "ipc", message: `open_pdf start: ${path}` });

    const opened = await openPdf(path);
    if (!opened.ok) {
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Failed to open ${path}: ${opened.error.message}` });
      return null;
    }

    const response = opened.data;
    const rawName = path.split("/").pop() ?? path.split("\\").pop() ?? path;
    const title = response.summary.title ?? rawName;

    const tab: DocumentTab = {
      id: response.session_id,
      title,
      kind: "pdf",
      workspaceId: activeWorkspaceId,
      sourcePath: response.source_path,
      pinned: false,
      dirty: false,
      page: 1,
      totalPages: response.summary.page_count,
      zoom: 100,
      loadState: "loading",
    };
    openTab(tab);
    appendDiagnostic({ level: "INFO", source: "ipc", message: `open_pdf success: ${title} (${response.summary.page_count} pages)` });

    const sessionState = await getSessionState(response.session_id);
    if (sessionState.ok) {
      updateTab(response.session_id, sessionStateToTabPatch(sessionState.data));
      appendDiagnostic({ level: "INFO", source: "state", message: `Session state synced for ${sessionState.data.title}` });
    } else {
      // A PDF session is not usable until its authoritative state is known.
      // Close the half-initialized backend session instead of presenting a
      // misleading ready tab.
      updateTab(response.session_id, {
        loadState: "error",
        errorMessage: `Session initialization failed: ${sessionState.error.message}`,
      });
      await closeSession(response.session_id);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Session state fetch failed: ${sessionState.error.message}` });
      return null;
    }

    // A recent-file record is written only after the backend session has
    // completed its authoritative state handshake.  A failed registry write
    // is visible but never turns the failed write into a fake recent entry.
    const library = await libraryRecordOpen({
      source_path: response.source_path,
      workspace_id: activeWorkspaceId || null,
      page_count: response.summary.page_count,
    });
    if (library.ok) {
      onLibraryUpdated?.(library.data);
      const recent = library.data.recentFiles.find((file) => file.path === response.source_path);
      if (recent) updateTab(response.session_id, { workspaceId: recent.workspaceId });
    } else {
      appendDiagnostic({ level: "ERROR", source: "state", message: `Recent-file registry update failed: ${library.error.message}` });
    }

    const project = await projectLoad(response.source_path);
    if (!project.ok) {
      updateTab(response.session_id, {
        loadState: "ready",
        errorMessage: `Project state was not restored: ${project.error.message}`,
      });
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Project state load failed: ${project.error.message}` });
    } else if (project.data.project) {
      depsRef.current.onProjectLoaded?.(response.session_id, project.data.project, project.data.document_id);
      const persistedDocument = project.data.project.documents.find((document) => document.document_id === project.data.document_id);
      const library = await libraryRecordProject({
        source_path: response.source_path,
        project_name: project.data.project.project_name,
        workspace_id: project.data.project.workspace_id,
        page_count: persistedDocument?.page_count ?? response.summary.page_count,
      });
      if (library.ok) {
        onLibraryUpdated?.(library.data);
        const recent = library.data.recentFiles.find((file) => file.path === response.source_path);
        if (recent) updateTab(response.session_id, { workspaceId: recent.workspaceId });
      } else {
        appendDiagnostic({ level: "ERROR", source: "state", message: `Persisted project index update failed: ${library.error.message}` });
      }
      if (project.data.migrated) {
        appendDiagnostic({ level: "INFO", source: "state", message: "Project state loaded through a version migration; save to commit the current schema." });
      } else {
        appendDiagnostic({ level: "INFO", source: "state", message: "Project state restored from the application-data sidecar." });
      }
    }

    void buildSearchIndex(response.session_id, appendDiagnostic);
    return response.session_id;
  }, []);

  const handleOpenFileDialog = useCallback(async () => {
    const filePath = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!filePath) return;
    await openPdfPath(filePath);
  }, [openPdfPath]);

  const openMostRecentFile = useCallback(() => {
    const firstRecent = depsRef.current.recentFiles[0];
    if (!firstRecent) return;
    void openPdfPath(firstRecent.path);
  }, [openPdfPath]);

  return { openPdfPath, handleOpenFileDialog, openMostRecentFile };
}

async function buildSearchIndex(
  sessionId: string,
  appendDiagnostic: AppendDiagnostic,
) {
  try {
    const extraction = await docExtractAllText(sessionId);
    if (!extraction.ok) throw new Error(extraction.error.message);

    const pagesWithSpans = extraction.data.map((text) => ({ text, spans: [] }));
    const spanResult = await searchIndexDocumentWithSpans(sessionId, pagesWithSpans);
    if (spanResult.ok) {
      appendDiagnostic({ level: "INFO", source: "ipc", message: `Search index ready (spans): ${spanResult.data.indexed_pages}/${spanResult.data.total_pages} pages` });
      return;
    }

    const indexed = await searchIndexDocument(sessionId, extraction.data);
    if (!indexed.ok) throw new Error(indexed.error.message);
    appendDiagnostic({ level: "INFO", source: "ipc", message: `Search index ready: ${indexed.data.indexed_pages}/${indexed.data.total_pages} pages` });
  } catch (error) {
    appendDiagnostic({ level: "WARN", source: "ipc", message: `Search index build failed: ${String(error)}` });
  }
}
