import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import { CommandPalette } from "./components/overlays/CommandPalette";
import { DiagnosticsPanel } from "./components/overlays/DiagnosticsPanel";
import { DocumentPropertiesModal } from "./components/overlays/DocumentPropertiesModal";
import { PreferencesPanel } from "./components/overlays/PreferencesPanel";
import { CenterWorkspace } from "./components/shell/CenterWorkspace";
import { LeftPanel } from "./components/shell/LeftPanel";
import { MenuBar } from "./components/shell/MenuBar";
import { RightInspector, type InspectorTab } from "./components/shell/RightInspector";
import { StatusBar } from "./components/shell/StatusBar";
import { TopBar } from "./components/shell/TopBar";
import { ThumbnailStrip } from "./components/viewer/ThumbnailStrip";
import { EditorToolbar, usePdfEditorState, useEditorKeyboard, restoreProjectObjects } from "./features/pdf-editor";
import type { EditorObject, EditorRect, TextBoxObject, ShapeObject, RedactionObject, CommentObject, HighlightObject, StampObject } from "./features/pdf-editor";
import { useAuditLog } from "./features/audit-log";
import { applyAiActionsToEditorObjects } from "./features/ai-actions";
import type { StagedImageReplacement } from "./features/content-edit/imageLayout";
import { useAutosave } from "./hooks/useAutosave";
import { useGlobalShortcuts } from "./hooks/useGlobalShortcuts";
import { useMenuActions } from "./hooks/useMenuActions";
import { usePdfNavigation } from "./hooks/usePdfNavigation";
import { usePdfOpen } from "./hooks/usePdfOpen";
import { usePdfSave } from "./hooks/usePdfSave";
import { usePdfSearch } from "./hooks/usePdfSearch";
import { usePageOrganizer } from "./features/page-organizer/usePageOrganizer";
import { useRenderLifecycle } from "./hooks/useRenderLifecycle";
import { useSessionPersistence } from "./hooks/useSessionPersistence";
import { isBackendPdfSession, type BackendPdfTab } from "./lib/pdfSession";
import { prepareRestoredSession } from "./lib/sessionRestore";
import { useWorkstationState } from "./state/useWorkstationState";
import { closeSession, getLicenseStatus, libraryRecordReview, searchClearIndex, type LicenseStatus } from "./lib/ipc";
import { open } from "@tauri-apps/plugin-dialog";
import { openBentoPdfTools } from "./features/bentopdf";
import { createOcrEditorObjects, validateOcrResult } from "./features/ocr/ocrContract";
import type { DocumentTab } from "./types/shell";
import type { OcrPageResult } from "./features/ocr/types";
import type { OcrOverlaySpec } from "./lib/ipc";

function App() {
  const { state, derived, actions, sessionSnapshot } = useWorkstationState();
  const [showThumbnails, setShowThumbnails] = useState(false);
  const [showDocProps, setShowDocProps] = useState(false);
  const [renderVersion, setRenderVersion] = useState(0);
  // Phase 28F — true after any native/visual text edit invalidates the
  // current RAG index until the user rebuilds.
  const [ragIndexStale, setRagIndexStale] = useState(false);
  // Phase 30A — pending canvas-overlay inline text edit state. Owned at
  // App level so the canvas (CenterWorkspace) and the side panel
  // (ContentEditPanel) both see the same edit target.
  const [pendingInlineEdit, setPendingInlineEdit] = useState<{
    contentObject: import("./lib/ipc").ContentObject;
    pageHeightPts: number;
    pageIndex: number;
  } | null>(null);
  // Phase 30E — pending canvas-overlay inline block edit.
  const [pendingInlineBlockEdit, setPendingInlineBlockEdit] = useState<{
    block: import("./lib/ipc").TextBlock;
    pageHeightPts: number;
    pageIndex: number;
  } | null>(null);
  // Phase 30A/F — last successful page render dimensions (in PDF points)
  // used to position the inline editor at correct coordinates.
  const [lastRenderedPage, setLastRenderedPage] = useState<{
    pageIndex: number;
    widthPts: number;
    heightPts: number;
  } | null>(null);
  // Phase 30A — content objects for the current page, lifted from
  // ContentEditPanel so the canvas hit-test layer can use them too.
  const [currentPageContentObjects, setCurrentPageContentObjects] = useState<
    import("./lib/ipc").ContentObject[]
  >([]);
  const [currentPageFontRegistry, setCurrentPageFontRegistry] = useState<
    import("./lib/ipc").FontResourceInfo[]
  >([]);
  // Phase 31B — current page rotation (0/90/180/270). When non-zero we
  // disable the canvas inline editor and route the user to the side panel.
  const [currentPageRotation, setCurrentPageRotation] = useState<number>(0);
  // Phase 30F — currently selected content object id (for hit-test outline).
  const [selectedContentObjectId, setSelectedContentObjectId] = useState<string | null>(null);
  const [stagedImageReplace, setStagedImageReplace] = useState<StagedImageReplacement | null>(null);
  // Phase 30C — experimental ToUnicode native edit toggle.
  const [enableExperimentalToUnicode, setEnableExperimentalToUnicode] = useState(false);
  // Phase 32 — one-shot tab request consumed by RightInspector. Used by the
  // TopBar Export button (and other shortcuts) to open the inspector on a
  // specific tab.
  const [requestedInspectorTab, setRequestedInspectorTab] = useState<InspectorTab | null>(null);
  const [licenseStatus, setLicenseStatus] = useState<LicenseStatus | null>(null);

  useEffect(() => {
    let cancelled = false;
    const refreshLicenseStatus = async () => {
      const result = await getLicenseStatus();
      if (!cancelled && result.ok) {
        setLicenseStatus(result.data);
      }
    };
    void refreshLicenseStatus();
    const timer = window.setInterval(refreshLicenseStatus, 30000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, []);

  const requestInspectorTab = useCallback((next: InspectorTab) => {
    actions.setIsRightSidebarOpen(true);
    setRequestedInspectorTab(next);
  }, [actions]);

  // BentoPDF remains a separate optional utility surface. It is not the
  // production route for any R2H-owned OCR, Compare, Forms, Page Organizer,
  // Sign & Stamp, or Local Generation workflow.
  const handleOpenBentoPdfTools = useCallback(() => {
    void openBentoPdfTools()
      .then((result) => {
        actions.appendDiagnostic({
          level: "INFO",
          source: "ui",
          message: result.reusedExistingWindow
            ? "Focused the existing local BentoPDF tools window."
            : `Opened local BentoPDF tools from ${result.bundleSource}.`,
        });
      })
      .catch((error: unknown) => {
        const message = error instanceof Error ? error.message : String(error);

        actions.appendDiagnostic({
          level: "ERROR",
          source: "ipc",
          message: `BentoPDF tools failed to open: ${message}`,
        });

        window.alert(`BentoPDF tools could not be opened.\n\n${message}`);
      });
  }, [actions]);

  /**
   * Phase UX — the welcome screen lets the user pick a workflow before
   * any PDF is open. We remember the choice and apply it once a backend
   * PDF tab becomes the active tab. The pendingWorkflow is consumed on
   * the next render where a backend tab is ready.
   */
  const [pendingWorkflow, setPendingWorkflow] = useState<InspectorTab | null>(null);

  const activeTab = derived.activeTab;

  // The editor projection is initialized before PDF open so a durable project
  // sidecar can be applied during the open lifecycle, before the document is
  // considered fully restored.
  const editor = usePdfEditorState();
  const audit = useAuditLog();

  const handleProjectLoaded = useCallback((sessionId: string, project: import("./lib/ipc").PersistedProject, documentId: string) => {
    editor.replaceObjects(restoreProjectObjects(project, documentId, sessionId));
  }, [editor]);

  // Apply the welcome-screen workflow choice once the user actually has a
  // PDF open. Backend session ids are prefixed `doc-session-`. We wait for
  // the load state to settle so the inspector doesn't switch under a
  // loading panel.
  useEffect(() => {
    if (!pendingWorkflow) return;
    if (!activeTab) return;
    if (activeTab.kind !== "pdf") return;
    if (!activeTab.id.startsWith("doc-session-")) return;
    if (activeTab.loadState !== "ready") return;
    actions.setIsRightSidebarOpen(true);
    setRequestedInspectorTab(pendingWorkflow);
    setPendingWorkflow(null);
  }, [pendingWorkflow, activeTab, actions]);

  // --- PDF Open ---
  const { openPdfPath, handleOpenFileDialog, openMostRecentFile } = usePdfOpen({
    activeWorkspaceId: state.activeWorkspaceId,
    recentFiles: state.recentFiles,
    openTab: actions.openTab,
    updateTab: actions.updateTab,
    appendDiagnostic: actions.appendDiagnostic,
    onProjectLoaded: handleProjectLoaded,
    onLibraryUpdated: actions.setLibrarySnapshot,
  });

  // --- PDF Save ---
  const { handleSave, handleSaveAs } = usePdfSave({
    activeTab,
    editorObjects: activeTab && isBackendPdfSession(activeTab)
      ? Array.from(editor.objects.values()).filter((object) => object.sessionId === activeTab.id)
      : [],
    updateTab: actions.updateTab,
    appendDiagnostic: actions.appendDiagnostic,
    onLibraryUpdated: actions.setLibrarySnapshot,
  });

  // --- Reopen expired session helper ---
  const reopenPdfTab = useCallback(async (tab: BackendPdfTab): Promise<string | null> => {
    if (tab.sourcePath.startsWith("session://")) return null;
    actions.appendDiagnostic({ level: "WARN", source: "ipc", message: `Backend session expired; reopening ${tab.sourcePath}` });
    return openPdfPath(tab.sourcePath);
  }, [openPdfPath, actions]);

  // --- PDF Navigation ---
  const { fitMode, stepPage, jumpToPage, stepZoom, setZoomPreset, handleFitPage, handleFitWidth } = usePdfNavigation({
    activeTab,
    updateTab: actions.updateTab,
    appendDiagnostic: actions.appendDiagnostic,
    reopenPdfTab,
  });

  // --- PDF Search ---
  const search = usePdfSearch({
    activeTab,
    updateTab: actions.updateTab,
    appendDiagnostic: actions.appendDiagnostic,
  });

  // --- Render Lifecycle ---
  const { lastRenderMs, onRenderSuccess: onRenderSuccessRaw, onRenderError } = useRenderLifecycle({
    activeTab,
    setTabLoadState: actions.setTabLoadState,
    appendDiagnostic: actions.appendDiagnostic,
  });
  // Acrobat-UX fix: wrap onRenderSuccess to also capture page dimensions
  // for the canvas inline editor and hit-test overlay. We prefer the
  // explicit *_pts fields (PDF points, zoom-independent) — falling back to
  // width/height only on stale payloads. Using `response.width/height`
  // (which are PIXEL dimensions after zoom * dpr scaling) for "PDF
  // points" silently broke the inline editor at any zoom != 100%.
  const onRenderSuccess = useCallback(
    (response: import("./lib/ipc").RenderResponse) => {
      onRenderSuccessRaw(response);
      const widthPts = response.width_pts && response.width_pts > 0
        ? response.width_pts
        : response.width;
      const heightPts = response.height_pts && response.height_pts > 0
        ? response.height_pts
        : response.height;
      setLastRenderedPage({
        pageIndex: response.page_index,
        widthPts,
        heightPts,
      });
    },
    [onRenderSuccessRaw],
  );

  // --- Menu Actions ---
  const menu = useMenuActions({
    activeTab,
    diagnosticsOpen: state.diagnosticsOpen,
    closeTab: actions.closeTab,
    nextTab: actions.nextTab,
    setCurrentMode: actions.setCurrentMode,
    setCommandPaletteOpen: actions.setCommandPaletteOpen,
    setPreferencesOpen: actions.setPreferencesOpen,
    setDiagnosticsOpen: actions.setDiagnosticsOpen,
    setIsLeftSidebarOpen: actions.setIsLeftSidebarOpen,
    setIsRightSidebarOpen: actions.setIsRightSidebarOpen,
    isLeftSidebarOpen: state.isLeftSidebarOpen,
    isRightSidebarOpen: state.isRightSidebarOpen,
    appendDiagnostic: actions.appendDiagnostic,
    requestExport: () => requestInspectorTab("export"),
    requestInspectorTab,
  });

  // --- Close active tab (with backend session cleanup) ---
  const closeActiveTabWithSession = useCallback(async () => {
    const current = derived.activeTab;
    if (!current) return;
    if (current.kind === "pdf" && current.id.startsWith("doc-session-")) {
      const closed = await closeSession(current.id);
      if (!closed.ok) {
        actions.appendDiagnostic({ level: "WARN", source: "ipc", message: `Close session warning: ${closed.error.message}` });
      }
      await searchClearIndex(current.id);
    }
    actions.closeTab(current.id);
  }, [derived.activeTab, actions]);

  // --- Autosave ---
  useAutosave({
    sessionId: isBackendPdfSession(activeTab) ? activeTab.id : "",
    isDirty: activeTab?.dirty ?? false,
    enabled: isBackendPdfSession(activeTab),
    save: handleSave,
    onSuccess: (savedTo) => {
      if (isBackendPdfSession(activeTab)) actions.updateTab(activeTab.id, { lastSavedAt: Date.now(), dirty: false });
      actions.appendDiagnostic({ level: "INFO", source: "ipc", message: `Autosave succeeded: ${savedTo}` });
    },
    onError: (message) => {
      actions.appendDiagnostic({ level: "WARN", source: "ipc", message: `Autosave failed: ${message}` });
    },
  });

  // --- Session Persistence ---
  useSessionPersistence({
    sessionSnapshot,
    restoreLastSession: state.preferences.restoreLastSession,
    onRestore: (snapshot) => {
      const restored = prepareRestoredSession(snapshot);
      actions.restoreSession(restored.snapshot);
      void (async () => {
        for (const { tabId, path } of restored.pdfTabsToReopen) {
          const sessionId = await openPdfPath(path);
          if (sessionId) { actions.closeTab(tabId); }
          else { actions.setTabLoadState(tabId, "error", `Failed to open ${path}`); }
        }
      })();
    },
  });

  // --- Global Shortcuts ---
  useGlobalShortcuts({
    openCommandPalette: () => actions.setCommandPaletteOpen(true),
    openPreferences: () => actions.setPreferencesOpen(true),
    toggleDiagnostics: () => actions.setDiagnosticsOpen(!state.diagnosticsOpen),
    closeActiveTab: () => { void closeActiveTabWithSession(); },
    nextTab: actions.nextTab,
    previousTab: actions.previousTab,
    nextPage: () => { void stepPage(1); },
    previousPage: () => { void stepPage(-1); },
    zoomIn: () => { void stepZoom(10); },
    zoomOut: () => { void stepZoom(-10); },
    setMode: actions.setCurrentMode,
    onNextMatch: search.nextMatch,
    onPreviousMatch: search.previousMatch,
  });

  // --- Escape key handler ---
  useEffect(() => {
    const onEsc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      actions.setCommandPaletteOpen(false);
      actions.setPreferencesOpen(false);
    };
    window.addEventListener("keydown", onEsc);
    return () => { window.removeEventListener("keydown", onEsc); };
  }, [actions]);

  // --- Tab open helper ---
  const openTab = useCallback((tab: DocumentTab) => {
    if (tab.kind === "pdf" && !tab.sourcePath.startsWith("session://")) {
      void openPdfPath(tab.sourcePath);
      return;
    }
    actions.openTab(tab);
    actions.setCurrentMode(tab.kind === "pdf" ? "read" : "review");
    actions.appendDiagnostic({ level: "INFO", source: "state", message: `Opened tab ${tab.title}` });
  }, [openPdfPath, actions]);

  // --- Workspace switch ---
  const setWorkspace = useCallback((workspaceId: string) => {
    actions.setActiveWorkspaceId(workspaceId);
    actions.appendDiagnostic({ level: "INFO", source: "state", message: `Workspace switched to ${workspaceId}` });
  }, [actions]);

  // --- Layout insets ---
  const leftInset = useMemo(() => (state.isLeftSidebarOpen ? 324 : 0), [state.isLeftSidebarOpen]);
  const rightInset = useMemo(() => (state.isRightSidebarOpen ? 348 : 0), [state.isRightSidebarOpen]);

  // --- PDF Editor ---
  const isEditorMode = state.currentMode === "annotate";

  const editorObjectsForCurrentPage = useMemo(() => {
    if (!activeTab || !isBackendPdfSession(activeTab)) return [];
    return editor.objectsForPage(activeTab.id, Math.max(activeTab.page - 1, 0));
  }, [activeTab, editor]);

  const markEditorDirty = useCallback(() => {
    if (activeTab && isBackendPdfSession(activeTab)) {
      actions.updateTab(activeTab.id, { dirty: true });
    }
  }, [activeTab, actions]);

  const addEditorObject = useCallback((object: EditorObject) => {
    editor.addObject(object);
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const addEditorObjects = useCallback((objects: EditorObject[]) => {
    for (const object of objects) {
      addEditorObject(object);
      audit.record("object_created", object.id, object.type, object.pageIndex, `Added ${object.type} from a document workflow`);
    }
  }, [addEditorObject, audit]);

  const updateEditorObjectRect = useCallback((objectId: string, rect: EditorRect) => {
    editor.updateObjectRect(objectId, rect);
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const patchEditorObject = useCallback((objectId: string, patch: import("./features/pdf-editor/types").EditorObjectPatch) => {
    editor.patchObject(objectId, patch);
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const deleteEditorObject = useCallback((objectId: string) => {
    editor.deleteObject(objectId);
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const acceptOcrOverlays = useCallback(async (
    result: OcrPageResult,
    overlays: OcrOverlaySpec[],
  ): Promise<boolean> => {
    if (!activeTab || !isBackendPdfSession(activeTab)) {
      actions.appendDiagnostic({ level: "WARN", source: "ui", message: "OCR acceptance requires an active native PDF session." });
      return false;
    }
    if (result.session_id !== activeTab.id || result.status !== "completed") {
      actions.appendDiagnostic({ level: "ERROR", source: "ui", message: "OCR acceptance rejected a stale, non-completed, or foreign-session result." });
      return false;
    }
    const validation = validateOcrResult(result);
    if (!validation.ok || validation.blockCount === 0) {
      actions.appendDiagnostic({ level: "ERROR", source: "ui", message: validation.ok ? "OCR acceptance rejected an empty geometry result." : `[${validation.code}] ${validation.message}` });
      return false;
    }

    const existing = Array.from(editor.objects.values()).filter((object) => object.sessionId === activeTab.id);
    const candidates = createOcrEditorObjects(result, overlays, existing.length + 1);
    const objects = candidates.filter((object) => !editor.objects.has(object.id));
    if (objects.length === 0) {
      actions.appendDiagnostic({ level: "INFO", source: "ui", message: "OCR overlays were already present; saving the canonical project state idempotently." });
    }
    for (const object of objects) {
      addEditorObject(object);
      audit.record("object_created", object.id, object.type, object.pageIndex, "Accepted validated OCR overlay", "user");
    }

    const saved = await handleSave([...existing, ...objects]);
    if (!saved.ok) {
      for (const object of objects) deleteEditorObject(object.id);
      actions.appendDiagnostic({ level: "ERROR", source: "ipc", message: `OCR overlays were rolled back because project save failed: ${saved.error ?? saved.status}` });
      return false;
    }
    actions.appendDiagnostic({ level: "INFO", source: "ui", message: `Accepted and saved ${objects.length} OCR overlay(s) through the canonical project sidecar.` });
    return true;
  }, [activeTab, actions, addEditorObject, audit, deleteEditorObject, editor.objects, handleSave]);

  const acceptGeneratedText = useCallback(async (generatedText: string): Promise<boolean> => {
    if (!activeTab || !isBackendPdfSession(activeTab)) {
      actions.appendDiagnostic({ level: "WARN", source: "ui", message: "Local generation requires an active PDF document." });
      return false;
    }
    const text = generatedText.trim().slice(0, 16_000);
    if (!text) {
      actions.appendDiagnostic({ level: "WARN", source: "ui", message: "Local generation returned no text to save." });
      return false;
    }
    const now = Date.now();
    const object: TextBoxObject = {
      id: `local-generation-${now}-${Math.random().toString(36).slice(2, 8)}`,
      sessionId: activeTab.id,
      pageIndex: Math.max(activeTab.page - 1, 0),
      type: "textBox",
      rect: { x: 72, y: 72, width: 468, height: 144 },
      rotation: 0,
      zIndex: editor.objects.size + 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: { source: "local_generation", document_context: true },
      text,
      fontSize: 11,
      fontFamily: "Helvetica",
      color: "#111",
      backgroundColor: "rgba(230,245,255,0.92)",
      borderColor: "rgba(38,116,170,0.55)",
    };
    addEditorObject(object);
    const objects = [
      ...Array.from(editor.objects.values()).filter((entry) => entry.sessionId === activeTab.id),
      object,
    ];
    const saved = await handleSave(objects);
    if (!saved.ok) {
      deleteEditorObject(object.id);
      actions.appendDiagnostic({ level: "ERROR", source: "ipc", message: `Local generation was not saved: ${saved.error ?? saved.status}` });
      return false;
    }
    audit.record("object_created", object.id, object.type, object.pageIndex, "Accepted local generation and saved PDF project state", "ai");
    actions.appendDiagnostic({ level: "INFO", source: "ui", message: "Accepted local generation and saved it to the active PDF project." });
    return true;
  }, [activeTab, actions, addEditorObject, audit, deleteEditorObject, editor.objects, handleSave]);

  const pageOrganizer = usePageOrganizer({
    activeTab,
    updateTab: actions.updateTab,
    appendDiagnostic: actions.appendDiagnostic,
    onPageDeleted: () => setRenderVersion((version) => version + 1),
    onPageInserted: () => setRenderVersion((version) => version + 1),
    onPageMoved: () => setRenderVersion((version) => version + 1),
    onPageRotated: () => setRenderVersion((version) => version + 1),
  });

  const undoEditor = useCallback(() => {
    editor.undo();
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const redoEditor = useCallback(() => {
    editor.redo();
    markEditorDirty();
  }, [editor, markEditorDirty]);

  const handleEditorCreateObject = useCallback((pdfRect: EditorRect) => {
    if (!activeTab || !isBackendPdfSession(activeTab)) return;
    const now = Date.now();
    const id = `editor-obj-${now}-${Math.random().toString(36).slice(2, 8)}`;
    const base = {
      id,
      sessionId: activeTab.id,
      pageIndex: Math.max(activeTab.page - 1, 0),
      rect: pdfRect,
      rotation: 0,
      zIndex: editorObjectsForCurrentPage.length + 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: {},
    };

    let obj: EditorObject;
    switch (editor.activeTool) {
      case "textBox":
        obj = { ...base, type: "textBox", text: "Text", fontSize: 12, fontFamily: "Helvetica", color: "#000", backgroundColor: "rgba(255,255,255,0.9)", borderColor: "rgba(116,162,255,0.5)" } as TextBoxObject;
        break;
      case "comment":
        obj = { ...base, type: "comment", contents: "", author: "User", color: "#ffba6b", status: "open", rect: { ...pdfRect, width: 24, height: 24 } } as CommentObject;
        break;
      case "highlight":
        obj = { ...base, type: "highlight", color: "rgba(255,255,0,0.4)", opacity: 0.4, contents: "" } as HighlightObject;
        break;
      case "rectangle":
        obj = { ...base, type: "rectangle", strokeColor: "#333", strokeWidth: 2, fillColor: "transparent" } as ShapeObject;
        break;
      case "redaction":
        obj = { ...base, type: "redaction", fillColor: "#000", replacementText: "", reason: "", status: "draft" } as RedactionObject;
        break;
      case "stamp":
        obj = { ...base, type: "stamp", stampText: "REVIEWED", stampType: "REVIEWED", color: "#c00" } as StampObject;
        break;
      default:
        return;
    }
    addEditorObject(obj);
    audit.record("object_created", obj.id, obj.type, base.pageIndex, `Created ${obj.type}`);
  }, [activeTab, addEditorObject, editor, editorObjectsForCurrentPage.length, audit]);

  const handleEditorDeleteSelected = useCallback(() => {
    for (const id of editor.selectedIds) {
      const obj = editor.objects.get(id);
      deleteEditorObject(id);
      if (obj) audit.record("object_deleted", id, obj.type, obj.pageIndex, `Deleted ${obj.type}`);
    }
  }, [deleteEditorObject, editor, audit]);

  // Phase 30A — Edit Content mode: load content objects + font registry for
  // the current page so the canvas hit-test layer and the inline editor
  // can use them. Refreshes when page/session/renderVersion change.
  useEffect(() => {
    if (!activeTab || !isBackendPdfSession(activeTab) || state.currentMode !== "edit-content") {
      setCurrentPageContentObjects([]);
      setCurrentPageFontRegistry([]);
      setPendingInlineEdit(null);
      setSelectedContentObjectId(null);
      return;
    }
    const sessionId = activeTab.id;
    const pageIndex = Math.max(activeTab.page - 1, 0);
    let cancelled = false;
    (async () => {
      const ipc = await import("./lib/ipc");
      const [objsRes, regRes, rotRes] = await Promise.all([
        ipc.pdfGetPageContentObjects(sessionId, pageIndex),
        ipc.pdfGetPageFontRegistry(sessionId, pageIndex),
        ipc.pdfGetPageRotation(sessionId, pageIndex),
      ]);
      if (cancelled) return;
      if (objsRes.ok) setCurrentPageContentObjects(objsRes.data);
      if (regRes.ok) setCurrentPageFontRegistry(regRes.data.fonts);
      if (rotRes.ok) setCurrentPageRotation(rotRes.data.rotation_degrees);
      else setCurrentPageRotation(0);
      // Phase 31B — clear any pending canvas inline edits when rotation
      // is non-zero so we never display a misaligned editor.
      if (rotRes.ok && rotRes.data.rotation_degrees !== 0) {
        setPendingInlineEdit(null);
        setPendingInlineBlockEdit(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [activeTab, state.currentMode, renderVersion]);

  // Phase 30A — strategy resolution for the canvas inline editor.
  const inlineEditFontInfo = useMemo<import("./lib/ipc").FontResourceInfo | null>(() => {
    if (!pendingInlineEdit) return null;
    const name = pendingInlineEdit.contentObject.text_info?.font_name;
    if (!name) return null;
    return currentPageFontRegistry.find((f) => f.resource_name === name) ?? null;
  }, [pendingInlineEdit, currentPageFontRegistry]);

  const inlineEditMethodPreview = useMemo<import("./features/content-edit/InlineTextEditor").InlineMethodPreview>(() => {
    // Default to read-only when no pending; the prop is only used when
    // pendingInlineEdit is set so this is a sentinel.
    if (!pendingInlineEdit) {
      return { strategy: "read_only", fontPreserved: false, reasons: [] };
    }
    // Use the same characters-based heuristic as the side-panel strategy
    // hook. The replacement is the original text initially (apply is
    // disabled-when-unchanged), so we evaluate against the existing text
    // until the user types. This gives a stable initial badge.
    const obj = pendingInlineEdit.contentObject;
    const replacement = obj.text_info?.decoded_text ?? "";
    const fi = inlineEditFontInfo;
    if (obj.editable_level === "read_only") {
      return { strategy: "read_only", fontPreserved: false, reasons: ["Read-only span."] };
    }
    const hasNonAscii = [...replacement].some((c) => c.charCodeAt(0) > 0x7f);
    const outsideLatin1 = [...replacement].some((c) => c.charCodeAt(0) > 0xff);
    const hasCjk = [...replacement].some((c) => {
      const cp = c.charCodeAt(0);
      return (cp >= 0x4e00 && cp <= 0x9fff) || (cp >= 0x3040 && cp <= 0x30ff) || (cp >= 0xac00 && cp <= 0xd7af);
    });
    const hasArabic = [...replacement].some((c) => {
      const cp = c.charCodeAt(0);
      return (cp >= 0x0600 && cp <= 0x06ff) || (cp >= 0xfb50 && cp <= 0xfdff);
    });
    if (fi) {
      if (fi.is_type3) return { strategy: "read_only", fontPreserved: false, reasons: ["Type3 font."] };
      if (hasCjk) return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["CJK shaping not supported natively."] };
      if (hasArabic) return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Arabic shaping not supported natively."] };
      if (fi.is_subset) return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Subset font."] };
      if (fi.is_type0 || fi.encoding_kind === "identity_h" || fi.encoding_kind === "identity_v") {
        return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Type0 / Identity-H font."] };
      }
      if (!hasNonAscii && fi.can_native_edit_ascii) return { strategy: "native_in_place", fontPreserved: true, reasons: [] };
      if (hasNonAscii && !outsideLatin1 && fi.can_native_edit_latin1) {
        return { strategy: "native_in_place", fontPreserved: true, reasons: ["Latin-1 via WinAnsi/MacRoman."] };
      }
      return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Font cannot encode replacement; using visual replacement."] };
    }
    // Fallback heuristic.
    if (hasCjk || hasArabic) return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Shaping required — visual."] };
    if (!hasNonAscii) return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["No font registry entry; native edit cannot be verified."] };
    return { strategy: "safe_visual_replacement", fontPreserved: false, reasons: ["Non-ASCII without registry — visual."] };
  }, [pendingInlineEdit, inlineEditFontInfo]);

  // Phase 30E — apply handler called by the canvas inline block editor.
  const handleCanvasBlockApply = useCallback(
    async (
      replacement: string,
      strategy: import("./lib/ipc").BlockEditStrategy,
      overflowPolicy: import("./lib/ipc").BlockOverflowPolicy = "reject",
    ) => {
      if (!pendingInlineBlockEdit) return;
      const sessionId = pendingInlineBlockEdit.block.session_id;
      const pageIndex = pendingInlineBlockEdit.pageIndex;
      const ipc = await import("./lib/ipc");
      const result = await ipc.pdfApplyTextBlockEdit({
        session_id: sessionId,
        page_index: pageIndex,
        block_id: pendingInlineBlockEdit.block.block_id,
        replacement_text: replacement,
        strategy,
        overflow_policy: overflowPolicy,
      });
      if (result.ok) {
        setRenderVersion((v) => v + 1);
        setPendingInlineBlockEdit(null);
        if (result.data.success) {
          setRagIndexStale(true);
          actions.appendDiagnostic({
            level: "INFO",
            source: "ui",
            message: `Canvas block edit on page ${pageIndex + 1}: method=${result.data.method}.`,
          });
        } else {
          actions.appendDiagnostic({
            level: "WARN",
            source: "ui",
            message: `Block edit was not applied: ${result.data.warnings.join("; ")}`,
          });
        }
      } else {
        actions.appendDiagnostic({
          level: "ERROR",
          source: "ipc",
          message: `Block edit IPC error: ${result.error.message}`,
        });
      }
    },
    [pendingInlineBlockEdit, actions],
  );

  // Phase 30A — apply handler called by the canvas inline editor.
  const handleCanvasInlineApply = useCallback(
    async (replacement: string) => {
      if (!pendingInlineEdit) return;
      const sessionId = pendingInlineEdit.contentObject.session_id;
      const pageIndex = pendingInlineEdit.pageIndex;
      const ipc = await import("./lib/ipc");
      const result = await ipc.pdfApplyNativeTextEdit({
        session_id: sessionId,
        page_index: pageIndex,
        content_object_id: pendingInlineEdit.contentObject.id,
        replacement_text: replacement,
        preserve_style: true,
      });
      if (result.ok) {
        setRenderVersion((v) => v + 1);
        setPendingInlineEdit(null);
        if (result.data.success) {
          // Invalidate the RAG index after native text edits.
          setRagIndexStale(true);
          actions.appendDiagnostic({
            level: "INFO",
            source: "ui",
            message: `Canvas inline edit on page ${pageIndex + 1}: method=${result.data.method}. RAG index marked stale.`,
          });
        } else {
          actions.appendDiagnostic({
            level: "WARN",
            source: "ui",
            message: `Canvas inline edit was not applied: ${result.data.warnings.join("; ")}`,
          });
        }
      } else {
        actions.appendDiagnostic({
          level: "ERROR",
          source: "ipc",
          message: `Canvas inline edit IPC error: ${result.error.message}`,
        });
      }
    },
    [pendingInlineEdit, actions],
  );

  const handleCanvasImageMoveApply = useCallback(
    async (
      obj: import("./lib/ipc").ContentObject,
      bbox: [number, number, number, number],
    ) => {
      if (!activeTab || !isBackendPdfSession(activeTab)) return;
      const ipc = await import("./lib/ipc");
      const pageIndex = Math.max(activeTab.page - 1, 0);
      const result = await ipc.pdfMoveNativeImage({
        session_id: activeTab.id,
        page_index: pageIndex,
        content_object_id: obj.id,
        new_rect: bbox,
      });
      if (result.ok && result.data.success) {
        setRenderVersion((v) => v + 1);
        actions.appendDiagnostic({
          level: "INFO",
          source: "ui",
          message: `Image moved/resized on page ${pageIndex + 1}: method=${result.data.method}.`,
        });
      } else {
        actions.appendDiagnostic({
          level: result.ok ? "WARN" : "ERROR",
          source: result.ok ? "ui" : "ipc",
          message: result.ok
            ? `Image move/resize was not applied: ${result.data.warnings.join("; ")}`
            : `Image move/resize IPC error: ${result.error.message}`,
        });
      }
    },
    [activeTab, actions],
  );

  const handleCanvasImageDelete = useCallback(
    async (obj: import("./lib/ipc").ContentObject) => {
      if (!activeTab || !isBackendPdfSession(activeTab)) return;
      const confirmed = window.confirm("Delete this image from the exported PDF using a visual cover?");
      if (!confirmed) return;
      const ipc = await import("./lib/ipc");
      const pageIndex = Math.max(activeTab.page - 1, 0);
      const result = await ipc.pdfDeleteNativeImage({
        session_id: activeTab.id,
        page_index: pageIndex,
        content_object_id: obj.id,
      });
      if (result.ok && result.data.success) {
        setRenderVersion((v) => v + 1);
        setSelectedContentObjectId(null);
        actions.appendDiagnostic({
          level: "INFO",
          source: "ui",
          message: `Image deleted on page ${pageIndex + 1}: method=${result.data.method}.`,
        });
      } else {
        actions.appendDiagnostic({
          level: result.ok ? "WARN" : "ERROR",
          source: result.ok ? "ui" : "ipc",
          message: result.ok
            ? `Image delete was not applied: ${result.data.warnings.join("; ")}`
            : `Image delete IPC error: ${result.error.message}`,
        });
      }
    },
    [activeTab, actions],
  );

  const handleStageImageReplace = useCallback((draft: StagedImageReplacement) => {
    setSelectedContentObjectId(draft.contentObjectId);
    setStagedImageReplace(draft);
    actions.appendDiagnostic({
      level: "INFO",
      source: "ui",
      message: `Image replacement staged on page ${draft.pageIndex + 1}; session bytes not mutated until Apply.`,
    });
  }, [actions]);

  const handleUpdateStagedImageReplace = useCallback((patch: Partial<Pick<StagedImageReplacement, "layoutMode" | "preserveAspect">>) => {
    setStagedImageReplace((current) => current ? { ...current, ...patch } : current);
  }, []);

  const handleCancelStagedImageReplace = useCallback(() => {
    setStagedImageReplace(null);
    actions.appendDiagnostic({
      level: "INFO",
      source: "ui",
      message: "Staged image replacement canceled; session bytes were not mutated.",
    });
  }, [actions]);

  const handleApplyStagedImageReplace = useCallback(async () => {
    if (!stagedImageReplace || !activeTab || !isBackendPdfSession(activeTab)) return;
    const ipc = await import("./lib/ipc");
    const result = await ipc.pdfReplaceNativeImage({
      session_id: stagedImageReplace.sessionId,
      page_index: stagedImageReplace.pageIndex,
      content_object_id: stagedImageReplace.contentObjectId,
      image_bytes_base64: stagedImageReplace.imageBytesBase64,
      layout_mode: stagedImageReplace.layoutMode,
    });
    if (result.ok && result.data.success) {
      setRenderVersion((v) => v + 1);
      setStagedImageReplace(null);
      actions.appendDiagnostic({
        level: "INFO",
        source: "ui",
        message: `Image replacement applied on page ${stagedImageReplace.pageIndex + 1}: layout=${stagedImageReplace.layoutMode}, method=${result.data.method}.`,
      });
    } else {
      actions.appendDiagnostic({
        level: result.ok ? "WARN" : "ERROR",
        source: result.ok ? "ui" : "ipc",
        message: result.ok
          ? `Image replacement was not applied: ${result.data.warnings.join("; ")}`
          : `Image replacement IPC error: ${result.error.message}`,
      });
    }
  }, [activeTab, actions, stagedImageReplace]);

  const handleCanvasImageCropApply = useCallback(
    async (
      obj: import("./lib/ipc").ContentObject,
      cropRect: [number, number, number, number],
    ) => {
      if (!activeTab || !isBackendPdfSession(activeTab)) return;
      const ipc = await import("./lib/ipc");
      const pageIndex = Math.max(activeTab.page - 1, 0);
      const result = await ipc.pdfCropNativeImage({
        session_id: activeTab.id,
        page_index: pageIndex,
        content_object_id: obj.id,
        crop_rect: cropRect,
        target_rect: obj.bbox,
      });
      if (result.ok && result.data.success) {
        setRenderVersion((v) => v + 1);
        actions.appendDiagnostic({
          level: "INFO",
          source: "ui",
          message: `Image cropped on page ${pageIndex + 1}: method=${result.data.method}.`,
        });
      } else {
        actions.appendDiagnostic({
          level: result.ok ? "WARN" : "ERROR",
          source: result.ok ? "ui" : "ipc",
          message: result.ok
            ? `Image crop was not applied: ${result.data.warnings.join("; ")}`
            : `Image crop IPC error: ${result.error.message}`,
        });
      }
    },
    [activeTab, actions],
  );

  const handleCanvasImageRotate = useCallback(
    async (obj: import("./lib/ipc").ContentObject, degrees: -90 | 90 | 180 | 270) => {
      if (!activeTab || !isBackendPdfSession(activeTab)) return;
      const ipc = await import("./lib/ipc");
      const pageIndex = Math.max(activeTab.page - 1, 0);
      const result = await ipc.pdfRotateNativeImage({
        session_id: activeTab.id,
        page_index: pageIndex,
        content_object_id: obj.id,
        degrees,
      });
      if (result.ok && result.data.success) {
        setRenderVersion((v) => v + 1);
        actions.appendDiagnostic({
          level: "INFO",
          source: "ui",
          message: `Image rotated ${degrees}° on page ${pageIndex + 1}: method=${result.data.method}.`,
        });
      } else {
        actions.appendDiagnostic({
          level: result.ok ? "WARN" : "ERROR",
          source: result.ok ? "ui" : "ipc",
          message: result.ok
            ? `Image rotation was not applied: ${result.data.warnings.join("; ")}`
            : `Image rotation IPC error: ${result.error.message}`,
        });
      }
    },
    [activeTab, actions],
  );

  useEffect(() => {
    if (state.currentMode !== "edit-content") return;
    const onEditContentKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPendingInlineEdit(null);
        setPendingInlineBlockEdit(null);
        setSelectedContentObjectId(null);
        return;
      }
      if (e.key !== "Delete" && e.key !== "Backspace") return;
      if (!selectedContentObjectId) return;
      const obj = currentPageContentObjects.find((candidate) => candidate.id === selectedContentObjectId);
      if (!obj || obj.object_type !== "image_xobject") return;
      e.preventDefault();
      void handleCanvasImageDelete(obj);
    };
    window.addEventListener("keydown", onEditContentKey);
    return () => window.removeEventListener("keydown", onEditContentKey);
  }, [
    state.currentMode,
    selectedContentObjectId,
    currentPageContentObjects,
    handleCanvasImageDelete,
  ]);

  useEditorKeyboard({
    deleteSelected: handleEditorDeleteSelected,
    clearSelection: editor.clearSelection,
    undo: undoEditor,
    redo: redoEditor,
    hasSelection: editor.selectedIds.length > 0,
    editorActive: isEditorMode,
  });


  return (
    <main className="app-root">
      <div className="workstation-grid density-compact">
        <MenuBar
          onOpenFile={handleOpenFileDialog}
          onOpenRecent={openMostRecentFile}
            onOpenBentoPdf={handleOpenBentoPdfTools}
          onSave={handleSave}
          onSaveAs={handleSaveAs}
          onExport={menu.handleExport}
          canExport={Boolean(activeTab && isBackendPdfSession(activeTab))}
          onCloseDocument={menu.handleCloseDocument}
          onExit={menu.handleExit}
          onUndo={menu.handleUndo}
          onRedo={menu.handleRedo}
          onCut={menu.handleCut}
          onCopy={menu.handleCopy}
          onPaste={menu.handlePaste}
          onFind={() => { void search.runSearch(); }}
          onPreferences={menu.handlePreferences}
          onZoomIn={() => { void stepZoom(10); }}
          onZoomOut={() => { void stepZoom(-10); }}
          onFitPage={handleFitPage}
          onFitWidth={handleFitWidth}
          onToggleLeftSidebar={menu.handleToggleLeftSidebar}
          onToggleRightSidebar={menu.handleToggleRightSidebar}
          onFullScreen={menu.handleFullScreen}
        />

        <TopBar
          state={state}
          activeTab={activeTab}
          searchQuery={search.searchQuery}
          fitMode={fitMode}
          onSearchQueryChange={search.setSearchQuery}
          onSearchSubmit={() => { void search.runSearch(); }}
          onOpenFile={handleOpenFileDialog}
          onPreviousPage={() => { void stepPage(-1); }}
          onNextPage={() => { void stepPage(1); }}
          onGoToPage={(page) => { void jumpToPage(page); }}
          onZoomOut={() => { void stepZoom(-10); }}
          onZoomIn={() => { void stepZoom(10); }}
          onSetZoomPreset={(zoom) => { void setZoomPreset(zoom); }}
          onSetFitMode={(mode) => { if (mode === "page") handleFitPage(); else handleFitWidth(); }}
          onSetMode={actions.setCurrentMode}
          onToggleLeftSidebar={() => actions.setIsLeftSidebarOpen(!state.isLeftSidebarOpen)}
          onToggleThumbnails={() => setShowThumbnails((v) => !v)}
          onToggleRightSidebar={() => actions.setIsRightSidebarOpen(!state.isRightSidebarOpen)}
          onShowDocumentProperties={() => setShowDocProps(true)}
          onShowExport={() => requestInspectorTab("export")}
          showThumbnails={showThumbnails}
        />

        <section className="document-shell">
          {isEditorMode && (
            <EditorToolbar
              activeTool={editor.activeTool}
              canUndo={editor.canUndo}
              canRedo={editor.canRedo}
              onSetTool={editor.setActiveTool}
              onUndo={undoEditor}
              onRedo={redoEditor}
            />
          )}
          {showThumbnails && activeTab && isBackendPdfSession(activeTab) && (
            <div className="sidebar-overlay sidebar-overlay--thumbnails">
              <ThumbnailStrip sessionId={activeTab.id} totalPages={activeTab.totalPages ?? 0} currentPage={activeTab.page} onNavigate={(pageIndex) => { void jumpToPage(pageIndex + 1); }} />
            </div>
          )}

          <CenterWorkspace
            activeTab={activeTab}
            leftInset={leftInset}
            rightInset={rightInset}
            onRenderError={onRenderError}
            onRenderSuccess={onRenderSuccess}
            searchMatches={search.searchResults}
            activeMatchIndex={search.activeIndex}
            renderVersion={renderVersion}
            onReopenFile={() => { void (async () => { const f = await open({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] }); if (f) await openPdfPath(f); })(); }}
            onOpenBentoPdf={handleOpenBentoPdfTools}
            onShowInspectorTab={requestInspectorTab}
            onPickWorkflow={(tab) => setPendingWorkflow(tab)}
            editor={isEditorMode ? {
              objects: editorObjectsForCurrentPage,
              selectedIds: editor.selectedIds,
              activeTool: editor.activeTool,
              onSelectObject: (id) => editor.selectObject(id, "replace"),
              onClearSelection: editor.clearSelection,
              onMoveObject: (id, rect) => { updateEditorObjectRect(id, rect); audit.record("object_moved", id, "", 0, "Moved"); },
              onResizeObject: (id, rect) => { updateEditorObjectRect(id, rect); audit.record("object_resized", id, "", 0, "Resized"); },
              onCreateObject: handleEditorCreateObject,
              onPatchObject: (id, patch) => { patchEditorObject(id, patch); if (patch.text !== undefined) audit.record("object_text_changed", id, "textBox", 0, "Text edited"); },
            } : undefined}
            inlineEdit={
              currentPageRotation === 0 &&
              pendingInlineEdit && activeTab && isBackendPdfSession(activeTab) &&
              pendingInlineEdit.contentObject.session_id === activeTab.id &&
              pendingInlineEdit.pageIndex === Math.max(activeTab.page - 1, 0)
                ? {
                    contentObject: pendingInlineEdit.contentObject,
                    pageHeightPts: pendingInlineEdit.pageHeightPts,
                    methodPreview: inlineEditMethodPreview,
                    fontInfo: inlineEditFontInfo,
                    onApply: handleCanvasInlineApply,
                    onCancel: () => setPendingInlineEdit(null),
                  }
                : null
            }
            rotatedPageNotice={currentPageRotation !== 0 && state.currentMode === "edit-content"}
            inlineBlockEdit={
              currentPageRotation === 0 &&
              pendingInlineBlockEdit && activeTab && isBackendPdfSession(activeTab) &&
              pendingInlineBlockEdit.block.session_id === activeTab.id &&
              pendingInlineBlockEdit.pageIndex === Math.max(activeTab.page - 1, 0)
                ? {
                    block: pendingInlineBlockEdit.block,
                    pageHeightPts: pendingInlineBlockEdit.pageHeightPts,
                    onApply: handleCanvasBlockApply,
                    onCancel: () => setPendingInlineBlockEdit(null),
                  }
                : null
            }
            textSpanHitTest={
              currentPageRotation === 0 &&
              state.currentMode === "edit-content" && activeTab && isBackendPdfSession(activeTab) &&
              lastRenderedPage && lastRenderedPage.pageIndex === Math.max(activeTab.page - 1, 0)
                ? {
                    contentObjects: currentPageContentObjects,
                    pageHeightPts: lastRenderedPage.heightPts,
                    selectedObjectId: selectedContentObjectId,
                    onTextSpanSelect: (obj) => setSelectedContentObjectId(obj.id),
                    onTextSpanDoubleClick: (obj) => {
                      setPendingInlineEdit({
                        contentObject: obj,
                        pageHeightPts: lastRenderedPage.heightPts,
                        pageIndex: Math.max(activeTab.page - 1, 0),
                      });
                    },
                  }
                : null
            }
            imageHitTest={
              currentPageRotation === 0 &&
              state.currentMode === "edit-content" && activeTab && isBackendPdfSession(activeTab) &&
              lastRenderedPage && lastRenderedPage.pageIndex === Math.max(activeTab.page - 1, 0)
                ? {
                    contentObjects: currentPageContentObjects,
                    pageHeightPts: lastRenderedPage.heightPts,
                    selectedObjectId: selectedContentObjectId,
                    onImageSelect: (obj) => setSelectedContentObjectId(obj.id),
                    onImageEditApply: handleCanvasImageMoveApply,
                    onImageCropApply: handleCanvasImageCropApply,
                    onImageRotate: handleCanvasImageRotate,
                    onImageReplace: (obj) => {
                      setSelectedContentObjectId(obj.id);
                      requestInspectorTab("edit");
                    },
                    replacementDraft: stagedImageReplace,
                    onImageReplaceApply: handleApplyStagedImageReplace,
                    onImageReplaceCancel: handleCancelStagedImageReplace,
                    onImageDelete: handleCanvasImageDelete,
                  }
                : null
            }
          />

          {state.isLeftSidebarOpen && (
            <div className="sidebar-overlay sidebar-overlay--left">
              <LeftPanel state={state} onOpenTab={openTab} onOpenFilePath={(path) => { void openPdfPath(path); }} onSetWorkspace={setWorkspace} onClose={() => actions.setIsLeftSidebarOpen(false)} />
            </div>
          )}

          {state.isRightSidebarOpen && (
            <div className="sidebar-overlay sidebar-overlay--right">
              <RightInspector
                activeTab={activeTab}
                currentMode={state.currentMode}
                preferences={state.preferences}
                editorObjects={activeTab && isBackendPdfSession(activeTab) ? Array.from(editor.objects.values()).filter((o) => o.sessionId === activeTab.id) : []}
                searchResults={search.searchResults}
                searchBusy={search.searchBusy}
                searchMessage={search.searchMessage}
                searchMatchIndex={search.activeIndex}
                searchCaseSensitive={search.searchCaseSensitive}
                searchWholeWords={search.searchWholeWords}
                searchUseRegex={search.searchUseRegex}
                onSearchCaseSensitiveChange={search.setSearchCaseSensitive}
                onSearchWholeWordsChange={search.setSearchWholeWords}
                onSearchUseRegexChange={search.setSearchUseRegex}
                onJumpToSearchMatch={(match) => { void search.jumpToSearchMatch(match); }}
                onNavigateToPage={(pageIndex) => { void jumpToPage(pageIndex + 1); }}
                onApplyAiActions={(aiActions) => {
                  if (!activeTab || !isBackendPdfSession(activeTab)) return;
                  const { applied, skipped } = applyAiActionsToEditorObjects(aiActions, activeTab.id, activeTab.totalPages ?? 0);
                  for (const obj of applied) {
                    addEditorObject(obj);
                    audit.record("object_created", obj.id, obj.type, obj.pageIndex, `AI action applied: ${obj.type}`, "ai");
                  }
                  for (const skip of skipped) {
                    actions.appendDiagnostic({ level: "WARN", source: "ipc", message: `AI action skipped (${skip.actionId}): ${skip.reason}` });
                  }
                  if (applied.length > 0) {
                    actions.appendDiagnostic({ level: "INFO", source: "ui", message: `Applied ${applied.length} AI actions to editor overlay` });
                  }
                }}
                onAddOverlay={addEditorObject}
                onAddEditorObjects={addEditorObjects}
                onAcceptOcrOverlays={acceptOcrOverlays}
                onAcceptGeneratedText={acceptGeneratedText}
                onLibraryUpdated={actions.setLibrarySnapshot}
                onReviewCompleted={async (sourcePath, reviewId) => {
                  const library = await libraryRecordReview({ source_path: sourcePath, review_id: reviewId, status: "in-review" });
                  if (library.ok) {
                    actions.setLibrarySnapshot(library.data);
                  } else {
                    actions.appendDiagnostic({ level: "ERROR", source: "state", message: `Review history update failed after completed review: ${library.error.message}` });
                  }
                }}
                pageOrganizer={pageOrganizer}
                onContentEdited={() => setRenderVersion((v) => v + 1)}
                onTextEdited={(info) => {
                  // Phase 28F — invalidate the RAG index after any text
                  // edit so the user knows their question results are
                  // out-of-sync until rebuild.
                  setRagIndexStale(true);
                  actions.appendDiagnostic({
                    level: "INFO",
                    source: "ui",
                    message: `Text edited on page ${info.pageIndex + 1} via ${info.method}; RAG index marked stale.`,
                  });
                }}
                indexStale={ragIndexStale}
                onIndexRefreshed={() => setRagIndexStale(false)}
                enableExperimentalToUnicode={enableExperimentalToUnicode}
                onToggleExperimentalToUnicode={setEnableExperimentalToUnicode}
                onRequestCanvasBlockEdit={(block) => {
                  if (!activeTab || !isBackendPdfSession(activeTab) || !lastRenderedPage) return;
                  setPendingInlineBlockEdit({
                    block,
                    pageHeightPts: lastRenderedPage.heightPts,
                    pageIndex: Math.max(activeTab.page - 1, 0),
                  });
                }}
                onStageImageReplace={handleStageImageReplace}
                stagedImageReplace={stagedImageReplace}
                onUpdateStagedImageReplace={handleUpdateStagedImageReplace}
                onApplyStagedImageReplace={handleApplyStagedImageReplace}
                onCancelStagedImageReplace={handleCancelStagedImageReplace}
                onSelectOverlay={(id) => editor.selectObject(id, "replace")}
                onPatchOverlay={(id, patch) => {
                  patchEditorObject(id, patch);
                  const obj = editor.objects.get(id);
                  if (obj) {
                    if (patch.hidden !== undefined) {
                      audit.record("object_text_changed", id, obj.type, obj.pageIndex,
                        patch.hidden ? "Hidden via Objects panel" : "Shown via Objects panel");
                    }
                    if (patch.locked !== undefined) {
                      audit.record("object_text_changed", id, obj.type, obj.pageIndex,
                        patch.locked ? "Locked via Objects panel" : "Unlocked via Objects panel");
                    }
                  }
                }}
                onDeleteOverlay={(id) => {
                  const obj = editor.objects.get(id);
                  deleteEditorObject(id);
                  if (obj) {
                    audit.record("object_deleted", id, obj.type, obj.pageIndex, "Deleted via Objects panel");
                  }
                }}
                appendDiagnostic={actions.appendDiagnostic}
                requestedTab={requestedInspectorTab}
                onTabChange={() => {
                  // Clear the one-shot request so the next external trigger
                  // re-fires the effect inside RightInspector.
                  if (requestedInspectorTab !== null) setRequestedInspectorTab(null);
                }}
                onClose={() => actions.setIsRightSidebarOpen(false)}
              />
            </div>
          )}
        </section>

        <StatusBar state={state} activeTab={activeTab} lastRenderMs={lastRenderMs} licenseStatus={licenseStatus} onToggleDiagnostics={() => actions.setDiagnosticsOpen(!state.diagnosticsOpen)} />
      </div>

      <CommandPalette open={state.commandPaletteOpen} commands={state.commands} onClose={() => actions.setCommandPaletteOpen(false)} onExecute={menu.executeCommand} isCommandAvailable={menu.isCommandAvailable} />
      <PreferencesPanel open={state.preferencesOpen} value={state.preferences} onClose={() => actions.setPreferencesOpen(false)} onChange={actions.setPreferences} />
      <DiagnosticsPanel open={state.diagnosticsOpen} logs={state.diagnostics} onClose={() => actions.setDiagnosticsOpen(false)} />
      {showDocProps && activeTab && <DocumentPropertiesModal tab={activeTab} onClose={() => setShowDocProps(false)} />}
    </main>
  );
}

export default App;
