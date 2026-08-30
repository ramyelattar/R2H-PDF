import { useCallback, useRef, useState } from "react";
import { goToPage, setZoom } from "../lib/ipc";
import { calculateFitToPageZoom, calculateFitToWidthZoom } from "../lib/fitMode";
import { isBackendPdfSession, sessionStateToTabPatch, type BackendPdfTab } from "../lib/pdfSession";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { DocumentTab } from "../types/shell";

interface UsePdfNavigationDeps {
  activeTab: DocumentTab | null;
  updateTab: (tabId: string, patch: Partial<DocumentTab>) => void;
  appendDiagnostic: AppendDiagnostic;
  reopenPdfTab: (tab: BackendPdfTab) => Promise<string | null>;
}

export function usePdfNavigation(deps: UsePdfNavigationDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [fitMode, setFitMode] = useState<"page" | "width">("page");

  const stepPage = useCallback(async (delta: number) => {
    const { activeTab, updateTab, appendDiagnostic, reopenPdfTab } = depsRef.current;
    if (!activeTab) return;

    if (isBackendPdfSession(activeTab)) {
      const maxPages = Math.max(activeTab.totalPages || 1, 1);
      const nextPage = Math.max(1, Math.min(maxPages, activeTab.page + delta));
      const moved = await goToPage(activeTab.id, nextPage - 1);
      if (!moved.ok) {
        if (moved.error.code === "SESSION_NOT_FOUND") {
          const reopenedId = await reopenPdfTab(activeTab);
          if (reopenedId) {
            const recovered = await goToPage(reopenedId, nextPage - 1);
            if (recovered.ok) updateTab(reopenedId, sessionStateToTabPatch(recovered.data));
          }
          return;
        }
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Go to page failed: ${moved.error.message}` });
        return;
      }
      updateTab(activeTab.id, sessionStateToTabPatch(moved.data));
      appendDiagnostic({ level: "INFO", source: "state", message: `Navigation updated to page ${moved.data.current_page + 1}/${moved.data.page_count}` });
      return;
    }

    const maxPages = Math.max(activeTab.totalPages || 1, 1);
    const nextPage = Math.max(1, Math.min(maxPages, activeTab.page + delta));
    updateTab(activeTab.id, { page: nextPage });
  }, []);

  const jumpToPage = useCallback(async (page: number) => {
    const { activeTab, updateTab, appendDiagnostic, reopenPdfTab } = depsRef.current;
    if (!activeTab) return;
    const clamped = Math.max(1, Math.min(activeTab.totalPages || 1, page));
    if (isBackendPdfSession(activeTab)) {
      const moved = await goToPage(activeTab.id, clamped - 1);
      if (!moved.ok) {
        if (moved.error.code === "SESSION_NOT_FOUND") {
          const reopenedId = await reopenPdfTab(activeTab);
          if (reopenedId) {
            const recovered = await goToPage(reopenedId, clamped - 1);
            if (recovered.ok) updateTab(reopenedId, sessionStateToTabPatch(recovered.data));
          }
          return;
        }
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Jump to page failed: ${moved.error.message}` });
        return;
      }
      updateTab(activeTab.id, sessionStateToTabPatch(moved.data));
      return;
    }
    updateTab(activeTab.id, { page: clamped });
  }, []);

  const stepZoom = useCallback(async (delta: number) => {
    const { activeTab, updateTab, appendDiagnostic, reopenPdfTab } = depsRef.current;
    if (!activeTab) return;

    const nextZoom = Math.max(25, Math.min(400, activeTab.zoom + delta));
    if (isBackendPdfSession(activeTab)) {
      const result = await setZoom(activeTab.id, nextZoom / 100);
      if (!result.ok) {
        if (result.error.code === "SESSION_NOT_FOUND") {
          const reopenedId = await reopenPdfTab(activeTab);
          if (reopenedId) {
            const recovered = await setZoom(reopenedId, nextZoom / 100);
            if (recovered.ok) updateTab(reopenedId, sessionStateToTabPatch(recovered.data));
          }
          return;
        }
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Set zoom failed: ${result.error.message}` });
        return;
      }
      updateTab(activeTab.id, sessionStateToTabPatch(result.data));
      appendDiagnostic({ level: "INFO", source: "state", message: `Zoom updated to ${Math.round(result.data.zoom * 100)}%` });
      return;
    }
    updateTab(activeTab.id, { zoom: nextZoom });
  }, []);

  const setZoomPreset = useCallback(async (zoom: number) => {
    const { activeTab, updateTab, reopenPdfTab } = depsRef.current;
    if (!activeTab) return;
    if (isBackendPdfSession(activeTab)) {
      const result = await setZoom(activeTab.id, zoom / 100);
      if (result.ok) {
        updateTab(activeTab.id, sessionStateToTabPatch(result.data));
      } else if (result.error.code === "SESSION_NOT_FOUND") {
        const reopenedId = await reopenPdfTab(activeTab);
        if (reopenedId) {
          const recovered = await setZoom(reopenedId, zoom / 100);
          if (recovered.ok) updateTab(reopenedId, sessionStateToTabPatch(recovered.data));
        }
      }
      return;
    }
    updateTab(activeTab.id, { zoom });
  }, []);

  const handleFitPage = useCallback(() => {
    setFitMode("page");
    const { activeTab } = depsRef.current;
    if (!activeTab || !isBackendPdfSession(activeTab)) return;
    const host = document.querySelector(".pdf-canvas-viewer") as HTMLElement | null;
    if (!host) return;
    const zoom = calculateFitToPageZoom({
      viewportWidth: host.clientWidth - 24,
      viewportHeight: host.clientHeight - 24,
      pageWidth: 612,
      pageHeight: 792,
    });
    void setZoomPreset(zoom);
  }, [setZoomPreset]);

  const handleFitWidth = useCallback(() => {
    setFitMode("width");
    const { activeTab } = depsRef.current;
    if (!activeTab || !isBackendPdfSession(activeTab)) return;
    const host = document.querySelector(".pdf-canvas-viewer") as HTMLElement | null;
    if (!host) return;
    const zoom = calculateFitToWidthZoom({
      viewportWidth: host.clientWidth - 24,
      pageWidth: 612,
    });
    void setZoomPreset(zoom);
  }, [setZoomPreset]);

  return {
    fitMode,
    stepPage,
    jumpToPage,
    stepZoom,
    setZoomPreset,
    handleFitPage,
    handleFitWidth,
  };
}
