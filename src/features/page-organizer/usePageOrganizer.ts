import { useCallback, useRef, useState } from "react";
import {
  editApplyTransaction,
  getSessionState,
  type EditOperation,
  type EditTransaction,
} from "../../lib/ipc";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { DocumentTab } from "../../types/shell";
import { isBackendPdfSession } from "../../lib/pdfSession";
import { sessionStateToTabPatch } from "../../lib/pdfSession";

interface UsePageOrganizerDeps {
  activeTab: DocumentTab | null;
  updateTab: (tabId: string, patch: Partial<DocumentTab>) => void;
  appendDiagnostic: AppendDiagnostic;
  onPageDeleted?: (pageIndex: number) => void;
  onPageInserted?: (atIndex: number) => void;
  onPageMoved?: (fromIndex: number, toIndex: number) => void;
  onPageRotated?: (pageIndex: number, degrees: number) => void;
}

export function usePageOrganizer(deps: UsePageOrganizerDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [selectedPages, setSelectedPages] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);

  const selectPage = useCallback((pageIndex: number, mode: "replace" | "toggle" = "replace") => {
    setSelectedPages((prev) => {
      if (mode === "toggle") {
        return prev.includes(pageIndex) ? prev.filter((p) => p !== pageIndex) : [...prev, pageIndex];
      }
      return [pageIndex];
    });
  }, []);

  const clearSelection = useCallback(() => setSelectedPages([]), []);

  const applyPageOperation = useCallback(async (
    opType: string,
    payloadJson: string,
    description: string,
    pageIndex: number,
  ): Promise<boolean> => {
    const { activeTab, updateTab, appendDiagnostic } = depsRef.current;
    if (!activeTab || !isBackendPdfSession(activeTab)) return false;
    const totalPages = activeTab.totalPages;
    const validPageIndex = Number.isInteger(pageIndex) && pageIndex >= 0 && pageIndex < totalPages;
    const insertOperation = opType === "InsertBlankPage";
    if ((insertOperation && (!Number.isInteger(pageIndex) || pageIndex < 0 || pageIndex > totalPages)) ||
        (!insertOperation && !validPageIndex)) {
      appendDiagnostic({ level: "ERROR", source: "ui", message: `Page operation rejected: page index ${pageIndex} is outside the current document.` });
      return false;
    }
    if (opType === "DeletePage" && totalPages <= 1) {
      appendDiagnostic({ level: "ERROR", source: "ui", message: "Page operation rejected: a PDF must retain at least one page." });
      return false;
    }
    if (busyRef.current) {
      appendDiagnostic({ level: "WARN", source: "ui", message: "Page operation ignored while another page operation is in progress." });
      return false;
    }

    busyRef.current = true;
    setBusy(true);
    try {
      const txId = `page-op-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      const op: EditOperation = {
        id: `op-${txId}`,
        op_type: opType,
        session_id: activeTab.id,
        page_index: pageIndex,
        object_ref: null,
        payload_json: payloadJson,
      };
      const tx: EditTransaction = {
        transaction_id: txId,
        session_id: activeTab.id,
        operations: [op],
        description,
      };

      const result = await editApplyTransaction(tx);
      if (!result.ok || !result.data.success) {
        const msg = result.ok ? (result.data.error ?? "Unknown error") : result.error.message;
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Page operation failed: ${msg}` });
        return false;
      }

      // A backend mutation is not presented as complete until the updated
      // authoritative session state can be read back.
      const state = await getSessionState(activeTab.id);
      if (!state.ok) {
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Page operation committed but session resync failed: ${state.error.message}` });
        return false;
      }
      updateTab(activeTab.id, sessionStateToTabPatch(state.data));
      appendDiagnostic({ level: "INFO", source: "ipc", message: `Page operation: ${description}` });
      return true;
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  const rotatePage = useCallback(async (pageIndex: number, degrees: number) => {
    const payload = JSON.stringify({ page_index: pageIndex, degrees });
    const ok = await applyPageOperation("RotatePage", payload, `Rotate page ${pageIndex + 1} by ${degrees}°`, pageIndex);
    if (ok) depsRef.current.onPageRotated?.(pageIndex, degrees);
    return ok;
  }, [applyPageOperation]);

  const deletePage = useCallback(async (pageIndex: number) => {
    const payload = JSON.stringify({ page_index: pageIndex });
    const ok = await applyPageOperation("DeletePage", payload, `Delete page ${pageIndex + 1}`, pageIndex);
    if (ok) {
      depsRef.current.onPageDeleted?.(pageIndex);
      setSelectedPages((prev) => prev.filter((p) => p !== pageIndex).map((p) => p > pageIndex ? p - 1 : p));
    }
    return ok;
  }, [applyPageOperation]);

  const insertBlankPage = useCallback(async (atIndex: number, widthPts = 612, heightPts = 792) => {
    const payload = JSON.stringify({ at_index: atIndex, width_pts: widthPts, height_pts: heightPts });
    const ok = await applyPageOperation("InsertBlankPage", payload, `Insert blank page at ${atIndex + 1}`, atIndex);
    if (ok) {
      depsRef.current.onPageInserted?.(atIndex);
      setSelectedPages((prev) => prev.map((p) => p >= atIndex ? p + 1 : p));
    }
    return ok;
  }, [applyPageOperation]);

  const movePage = useCallback(async (fromIndex: number, toIndex: number) => {
    const totalPages = depsRef.current.activeTab?.totalPages ?? 0;
    if (!Number.isInteger(fromIndex) || !Number.isInteger(toIndex) || fromIndex < 0 || toIndex < 0 || fromIndex >= totalPages || toIndex >= totalPages) {
      depsRef.current.appendDiagnostic({ level: "ERROR", source: "ui", message: "Move page rejected: source or destination is outside the current document." });
      return false;
    }
    if (fromIndex === toIndex) return true;
    const payload = JSON.stringify({ from_index: fromIndex, to_index: toIndex });
    const ok = await applyPageOperation("MovePage", payload, `Move page ${fromIndex + 1} → ${toIndex + 1}`, fromIndex);
    if (ok) depsRef.current.onPageMoved?.(fromIndex, toIndex);
    return ok;
  }, [applyPageOperation]);

  return {
    selectedPages,
    busy,
    selectPage,
    clearSelection,
    rotatePage,
    deletePage,
    insertBlankPage,
    movePage,
  };
}
