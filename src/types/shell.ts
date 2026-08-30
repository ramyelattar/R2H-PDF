export type NavSection =
  | "library"
  | "workspaces"
  | "search"
  | "review"
  | "handoff"
  | "export"
  | "diagnostics";

export type TabKind = "pdf" | "review" | "handoff" | "export";
export type TabLoadState = "empty" | "loading" | "ready" | "error";
export type ReaderMode = "read" | "review" | "annotate" | "edit-content";

export interface WorkspaceInfo {
  id: string;
  name: string;
  path: string;
  createdAtEpochMs: number;
  modifiedAtEpochMs: number;
  documentCount: number;
  lastOpenedAtEpochMs: number | null;
}

export interface DocumentTab {
  id: string;
  title: string;
  kind: TabKind;
  workspaceId: string;
  sourcePath: string;
  pinned: boolean;
  dirty: boolean;
  page: number;
  totalPages: number;
  zoom: number;
  rotation?: number;
  isScanned?: boolean;
  lastSavedAt?: number | null;
  permissions?: {
    canPrint: boolean;
    canCopy: boolean;
    canEdit: boolean;
    canAnnotate: boolean;
  };
  loadState: TabLoadState;
  errorMessage?: string;
}

export interface FileRecord {
  id: string;
  title: string;
  path: string;
  workspaceId: string;
  modifiedAtEpochMs: number;
  lastOpenedAtEpochMs: number;
  pages: number;
  pinned?: boolean;
  available: boolean;
}

export interface SavedSearch {
  id: string;
  name: string;
  query: string;
  scope: "active-workspace" | "all-workspaces";
  filters: Record<string, unknown>;
  createdAtEpochMs: number;
  updatedAtEpochMs: number;
}

export interface ReviewSession {
  id: string;
  projectId: string;
  documentId: string;
  status: "draft" | "in-review" | "approved";
  createdAtEpochMs: number;
  updatedAtEpochMs: number;
}

export interface HandoffPackage {
  id: string;
  projectId: string;
  destinationPath: string;
  createdAtEpochMs: number;
  includedDocuments: string[];
  outputSha256: string | null;
  status: "draft" | "ready" | "sent";
}

export interface ExportBundle {
  id: string;
  projectId: string;
  documentId: string;
  destination: string;
  exportType: "pdf" | "zip" | "package";
  timestampEpochMs: number;
  sizeBytes: number;
  sha256: string;
  pageCount: number;
  includedOverlays: number;
  warnings: string[];
  status: "completed" | "failed" | "cancelled";
}

export interface CommandItem {
  id: string;
  title: string;
  description: string;
  category: "Navigation" | "Documents" | "Panels" | "System";
  shortcut: string;
  requiresDocument?: boolean;
}

/** Backend-owned application-data records used by the production shell. */
export interface LibrarySnapshot {
  schemaVersion: number;
  workspaces: WorkspaceInfo[];
  recentFiles: FileRecord[];
  savedSearches: SavedSearch[];
  reviewSessions: ReviewSession[];
  handoffPackages: HandoffPackage[];
  exportBundles: ExportBundle[];
}

export interface DiagnosticLog {
  id: string;
  level: "INFO" | "WARN" | "ERROR";
  source: "ui" | "session" | "state" | "ipc" | "edit";
  message: string;
  at: string;
}

export interface UiPreferences {
  compactDensity: boolean;
  showRightInspector: boolean;
  showLeftPanel: boolean;
  restoreLastSession: boolean;
  telemetryMode: "minimal" | "standard" | "verbose";
  aiEnabled: boolean;
  aiEndpoint: string;
  aiModel: string;
  aiSummarizeEnabled: boolean;
  aiQaEnabled: boolean;
  aiAnnotationSuggestEnabled: boolean;
  aiEntityExtractEnabled: boolean;
}

export interface ShellSessionSnapshot {
  activeTabId: string | null;
  openTabs: DocumentTab[];
  activeWorkspaceId: string;
  isLeftSidebarOpen: boolean;
  isRightSidebarOpen: boolean;
  currentMode: ReaderMode;
  diagnosticsOpen: boolean;
  preferences: UiPreferences;
}

export interface DocumentSummary {
  page_count: number;
  object_count: number;
  title: string | null;
  author: string | null;
  producer: string | null;
}

export interface OpenDocumentResponse {
  session_id: string;
  source_path: string;
  recovered: boolean;
  summary: DocumentSummary;
}
