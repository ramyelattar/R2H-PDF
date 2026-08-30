import { useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { closeSession, editRedo, editUndo, searchClearIndex } from "../lib/ipc";
import { isBackendPdfSession } from "../lib/pdfSession";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { CommandItem } from "../types/shell";
import type { InspectorTab } from "../components/shell/RightInspector";
import { R2H_FEATURE_OWNERSHIP } from "../state/r2hFeatureOwnership";

interface UseMenuActionsDeps {
  activeTab: import("../types/shell").DocumentTab | null;
  diagnosticsOpen: boolean;
  closeTab: (tabId: string) => void;
  nextTab: () => void;
  setCurrentMode: (mode: "read" | "review" | "annotate") => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setPreferencesOpen: (open: boolean) => void;
  setDiagnosticsOpen: (open: boolean) => void;
  setIsLeftSidebarOpen: (open: boolean) => void;
  setIsRightSidebarOpen: (open: boolean) => void;
  isLeftSidebarOpen: boolean;
  isRightSidebarOpen: boolean;
  appendDiagnostic: AppendDiagnostic;
  requestExport: () => void;
  requestInspectorTab: (tab: InspectorTab) => void;
}

export function useMenuActions(deps: UseMenuActionsDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const handleCloseDocument = useCallback(async () => {
    const { activeTab, closeTab, appendDiagnostic } = depsRef.current;
    if (!activeTab) return;

    if (isBackendPdfSession(activeTab)) {
      appendDiagnostic({ level: "INFO", source: "ipc", message: `close_session start: ${activeTab.id}` });
      const closed = await closeSession(activeTab.id);
      if (!closed.ok) {
        appendDiagnostic({ level: "WARN", source: "ipc", message: `Backend close failed: ${closed.error.message}` });
      } else {
        appendDiagnostic({ level: "INFO", source: "ipc", message: `close_session success: ${activeTab.id}` });
      }
      await searchClearIndex(activeTab.id);
    }
    closeTab(activeTab.id);
  }, []);

  const handleExport = useCallback(() => {
    const { setCurrentMode, activeTab, appendDiagnostic, requestExport } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) {
      appendDiagnostic({ level: "WARN", source: "ui", message: "Export requires an active PDF document." });
      return;
    }
    setCurrentMode("read");
    requestExport();
  }, []);

  const handleExit = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  const handleUndo = useCallback(() => {
    const { activeTab, appendDiagnostic } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) return;
    void editUndo(activeTab.id).then((result) => {
      if (!result.ok) appendDiagnostic({ level: "ERROR", source: "edit", message: result.error.message });
    });
  }, []);

  const handleRedo = useCallback(() => {
    const { activeTab, appendDiagnostic } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) return;
    void editRedo(activeTab.id).then((result) => {
      if (!result.ok) appendDiagnostic({ level: "ERROR", source: "edit", message: result.error.message });
    });
  }, []);

  const handleCut = useCallback(() => {
    const { appendDiagnostic } = depsRef.current;
    appendDiagnostic({ level: "INFO", source: "ui", message: "Cut command triggered from menu" });
    appendDiagnostic({ level: "WARN", source: "ui", message: "Cut is currently available for text inputs only." });
  }, []);

  const handleCopy = useCallback(() => {
    const { appendDiagnostic } = depsRef.current;
    appendDiagnostic({ level: "INFO", source: "ui", message: "Copy command triggered from menu" });
    appendDiagnostic({ level: "WARN", source: "ui", message: "Copy is currently available for text inputs only." });
  }, []);

  const handlePaste = useCallback(() => {
    const { appendDiagnostic } = depsRef.current;
    appendDiagnostic({ level: "INFO", source: "ui", message: "Paste command triggered from menu" });
    appendDiagnostic({ level: "WARN", source: "ui", message: "Paste is currently available for text inputs only." });
  }, []);

  const handlePreferences = useCallback(() => {
    depsRef.current.setPreferencesOpen(true);
  }, []);

  const handleToggleLeftSidebar = useCallback(() => {
    const d = depsRef.current;
    d.setIsLeftSidebarOpen(!d.isLeftSidebarOpen);
  }, []);

  const handleToggleRightSidebar = useCallback(() => {
    const d = depsRef.current;
    d.setIsRightSidebarOpen(!d.isRightSidebarOpen);
  }, []);

  const handleFullScreen = useCallback(() => {
    void (async () => {
      const win = getCurrentWindow();
      const current = await win.isFullscreen();
      await win.setFullscreen(!current);
    })();
  }, []);

  const executeCommand = useCallback((command: CommandItem) => {
    const d = depsRef.current;
    const r2hFeature = R2H_FEATURE_OWNERSHIP.find((feature) => feature.commandId === command.id);
    if (command.requiresDocument && !isBackendPdfSession(d.activeTab)) {
      d.appendDiagnostic({
        level: "WARN",
        source: "ui",
        message: `${command.title} requires an active PDF document.`,
      });
      return;
    }
    switch (command.id) {
      case "cmd.open.commandPalette": d.setCommandPaletteOpen(true); break;
      case "cmd.open.preferences": d.setPreferencesOpen(true); break;
      case "cmd.open.diagnostics": d.setDiagnosticsOpen(!d.diagnosticsOpen); break;
      case "cmd.tab.next": d.nextTab(); break;
      case "cmd.tab.close": void handleCloseDocument(); break;
      default:
        if (r2hFeature) d.requestInspectorTab(r2hFeature.tabId);
        break;
    }
    d.appendDiagnostic({ level: "INFO", source: "ui", message: `Executed command: ${command.title}` });
    d.setCommandPaletteOpen(false);
  }, [handleCloseDocument, handleExport]);

  return {
    handleCloseDocument,
    handleExport,
    handleExit,
    handleUndo,
    handleRedo,
    handleCut,
    handleCopy,
    handlePaste,
    handlePreferences,
    handleToggleLeftSidebar,
    handleToggleRightSidebar,
    handleFullScreen,
    executeCommand,
    isCommandAvailable: (command: CommandItem) =>
      !command.requiresDocument || isBackendPdfSession(depsRef.current.activeTab),
  };
}
