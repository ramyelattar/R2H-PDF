import { useEffect, useMemo, useState } from "react";
import { commandCatalog } from "./shellCatalog";
import { libraryLoad } from "../lib/ipc";
import type {
  DiagnosticLog,
  DocumentTab,
  ExportBundle,
  FileRecord,
  HandoffPackage,
  LibrarySnapshot,
  ReaderMode,
  ReviewSession,
  SavedSearch,
  ShellSessionSnapshot,
  UiPreferences,
  WorkspaceInfo,
} from "../types/shell";

export interface WorkstationState {
  activeWorkspaceId: string;
  tabs: DocumentTab[];
  activeTabId: string | null;
  currentMode: ReaderMode;
  workspaces: WorkspaceInfo[];
  recentFiles: FileRecord[];
  savedSearches: SavedSearch[];
  reviewSessions: ReviewSession[];
  handoffPackages: HandoffPackage[];
  exportBundles: ExportBundle[];
  commands: typeof commandCatalog;
  diagnostics: DiagnosticLog[];
  isLeftSidebarOpen: boolean;
  isRightSidebarOpen: boolean;
  commandPaletteOpen: boolean;
  preferencesOpen: boolean;
  diagnosticsOpen: boolean;
  preferences: UiPreferences;
}

const defaultPreferences: UiPreferences = {
  compactDensity: true,
  showRightInspector: true,
  showLeftPanel: true,
  restoreLastSession: true,
  telemetryMode: "standard",
  aiEnabled: true,
  aiEndpoint: "http://localhost:11434",
  aiModel: "",
  aiSummarizeEnabled: false,
  aiQaEnabled: false,
  aiAnnotationSuggestEnabled: false,
  aiEntityExtractEnabled: false,
};

export const useWorkstationState = () => {
  const [workspaces, setWorkspaces] = useState<WorkspaceInfo[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState("");
  const [tabs, setTabs] = useState<DocumentTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [currentMode, setCurrentMode] = useState<ReaderMode>("read");
  const [recentFiles, setRecentFiles] = useState<FileRecord[]>([]);
  const [savedSearches, setSavedSearches] = useState<SavedSearch[]>([]);
  const [reviewSessions, setReviewSessions] = useState<ReviewSession[]>([]);
  const [handoffPackages, setHandoffPackages] = useState<HandoffPackage[]>([]);
  const [exportBundles, setExportBundles] = useState<ExportBundle[]>([]);
  const [commands] = useState(commandCatalog);
  const [diagnostics, setDiagnostics] = useState<DiagnosticLog[]>([]);
  const [isLeftSidebarOpen, setIsLeftSidebarOpen] = useState(false);
  const [isRightSidebarOpen, setIsRightSidebarOpen] = useState(true);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [preferencesOpen, setPreferencesOpen] = useState(false);
  const [diagnosticsOpen, setDiagnosticsOpen] = useState(false);
  const [preferences, setPreferences] = useState<UiPreferences>(defaultPreferences);

  useEffect(() => {
    let cancelled = false;
    void libraryLoad().then((result) => {
      if (cancelled) return;
      if (result.ok) {
        setWorkspaces(result.data.workspaces);
        setRecentFiles(result.data.recentFiles);
        setSavedSearches(result.data.savedSearches);
        setReviewSessions(result.data.reviewSessions);
        setHandoffPackages(result.data.handoffPackages);
        setExportBundles(result.data.exportBundles);
        setActiveWorkspaceId(result.data.workspaces[0]?.id ?? "");
      } else {
        setDiagnostics((current) => [
          ...current,
          {
            id: crypto.randomUUID(),
            at: new Date().toLocaleTimeString("en-US", { hour12: false }),
            level: "ERROR",
            source: "state",
            message: `Library registry load failed: ${result.error.message}`,
          },
        ]);
      }
    });
    return () => { cancelled = true; };
  }, []);

  const activeTab = useMemo(
    () => tabs.find((tab) => tab.id === activeTabId) ?? null,
    [tabs, activeTabId],
  );

  const selectTab = (tabId: string) => {
    setActiveTabId(tabId);
  };

  const nextTab = () => {
    if (!tabs.length || !activeTabId) return;
    const currentIndex = tabs.findIndex((tab) => tab.id === activeTabId);
    const nextIndex = (currentIndex + 1) % tabs.length;
    setActiveTabId(tabs[nextIndex].id);
  };

  const previousTab = () => {
    if (!tabs.length || !activeTabId) return;
    const currentIndex = tabs.findIndex((tab) => tab.id === activeTabId);
    const previousIndex = (currentIndex - 1 + tabs.length) % tabs.length;
    setActiveTabId(tabs[previousIndex].id);
  };

  const closeTab = (tabId: string) => {
    const currentIndex = tabs.findIndex((tab) => tab.id === tabId);
    const nextTabs = tabs.filter((tab) => tab.id !== tabId);
    setTabs(nextTabs);

    if (activeTabId === tabId) {
      if (!nextTabs.length) {
        setActiveTabId(null);
        return;
      }
      const fallbackIndex = Math.max(0, currentIndex - 1);
      setActiveTabId(nextTabs[fallbackIndex].id);
    }
  };

  const openTab = (tab: DocumentTab) => {
    const existing = tabs.find((entry) => entry.id === tab.id);
    if (existing) {
      setActiveTabId(existing.id);
      return;
    }
    setTabs((current) => [...current, tab]);
    setActiveTabId(tab.id);
  };

  const closeActiveTab = () => {
    if (!activeTabId) return;
    closeTab(activeTabId);
  };

  const togglePin = (tabId: string) => {
    setTabs((current) =>
      current.map((tab) =>
        tab.id === tabId
          ? {
              ...tab,
              pinned: !tab.pinned,
            }
          : tab,
      ),
    );
  };

  const setTabLoadState = (
    tabId: string,
    loadState: DocumentTab["loadState"],
    errorMessage?: string,
  ) => {
    setTabs((current) =>
      current.map((tab) =>
        tab.id === tabId
          ? {
              ...tab,
              loadState,
              errorMessage,
            }
          : tab,
      ),
    );
  };

  const updateTab = (tabId: string, patch: Partial<DocumentTab>) => {
    setTabs((current) =>
      current.map((tab) => (tab.id === tabId ? { ...tab, ...patch } : tab)),
    );
  };

  const appendDiagnostic = (entry: Omit<DiagnosticLog, "id" | "at">) => {
    const now = new Date();
    const timestamp = now.toLocaleTimeString("en-US", {
      hour12: false,
    });

    setDiagnostics((current) => [
      ...current,
      {
        ...entry,
        id: crypto.randomUUID(),
        at: timestamp,
      },
    ]);
  };

  const setLibrarySnapshot = (snapshot: LibrarySnapshot) => {
    setWorkspaces(snapshot.workspaces);
    setRecentFiles(snapshot.recentFiles);
    setSavedSearches(snapshot.savedSearches);
    setReviewSessions(snapshot.reviewSessions);
    setHandoffPackages(snapshot.handoffPackages);
    setExportBundles(snapshot.exportBundles);
    setActiveWorkspaceId((current) => {
      if (current && snapshot.workspaces.some((workspace) => workspace.id === current)) {
        return current;
      }
      return snapshot.workspaces[0]?.id ?? "";
    });
  };

  const restoreSession = (snapshot: ShellSessionSnapshot) => {
    setActiveWorkspaceId(snapshot.activeWorkspaceId);
    setTabs(snapshot.openTabs);
    setActiveTabId(snapshot.activeTabId);
    setCurrentMode(snapshot.currentMode ?? "read");
    setIsLeftSidebarOpen(
      snapshot.isLeftSidebarOpen ?? (snapshot as unknown as { leftPanelOpen?: boolean }).leftPanelOpen ?? false,
    );
    setIsRightSidebarOpen(
      snapshot.isRightSidebarOpen ??
        (snapshot as unknown as { rightPanelOpen?: boolean }).rightPanelOpen ??
        false,
    );
    setDiagnosticsOpen(snapshot.diagnosticsOpen);
    setPreferences(snapshot.preferences);

    appendDiagnostic({
      level: "INFO",
      source: "session",
      message: `Session restored with ${snapshot.openTabs.length} tabs`,
    });
  };

  const sessionSnapshot: ShellSessionSnapshot = {
    activeTabId,
    openTabs: tabs,
    activeWorkspaceId,
    currentMode,
    isLeftSidebarOpen,
    isRightSidebarOpen,
    diagnosticsOpen,
    preferences,
  };

  return {
    state: {
      activeWorkspaceId,
      tabs,
      activeTabId,
      currentMode,
      workspaces,
      recentFiles,
      savedSearches,
      reviewSessions,
      handoffPackages,
      exportBundles,
      commands,
      diagnostics,
      isLeftSidebarOpen,
      isRightSidebarOpen,
      commandPaletteOpen,
      preferencesOpen,
      diagnosticsOpen,
      preferences,
    } satisfies WorkstationState,
    derived: {
      activeTab,
      activeWorkspace: workspaces.find((ws) => ws.id === activeWorkspaceId) ?? null,
    },
    actions: {
      setActiveWorkspaceId,
      selectTab,
      nextTab,
      previousTab,
      closeTab,
      openTab,
      closeActiveTab,
      togglePin,
      setTabLoadState,
      updateTab,
      appendDiagnostic,
      setCurrentMode,
      setIsLeftSidebarOpen,
      setIsRightSidebarOpen,
      setCommandPaletteOpen,
      setPreferencesOpen,
      setDiagnosticsOpen,
      setPreferences,
      setLibrarySnapshot,
      restoreSession,
    },
    sessionSnapshot,
  };
};
