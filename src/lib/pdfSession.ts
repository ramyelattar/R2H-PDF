/**
 * Shared utilities for PDF session management across hooks.
 */

import type { DocumentTab } from "../types/shell";

export type BackendPdfTab = DocumentTab & { kind: "pdf" };

/** Type guard: is this tab a live backend PDF session? */
export function isBackendPdfSession(tab: DocumentTab | null): tab is BackendPdfTab {
  return Boolean(tab && tab.kind === "pdf" && tab.id.startsWith("doc-session-"));
}

/** Map backend session state response fields to frontend tab patch. */
export function sessionStateToTabPatch(session: {
  title: string;
  source_path: string;
  current_page: number;
  page_count: number;
  zoom: number;
  rotation: number;
  is_scanned: boolean;
  permissions: {
    can_print: boolean;
    can_copy: boolean;
    can_edit: boolean;
    can_annotate: boolean;
  };
  last_saved_at: number | null;
  dirty: boolean;
}): Partial<DocumentTab> {
  return {
    title: session.title,
    sourcePath: session.source_path,
    page: session.current_page + 1,
    totalPages: session.page_count,
    zoom: Math.round(session.zoom * 100),
    rotation: session.rotation,
    isScanned: session.is_scanned,
    lastSavedAt: session.last_saved_at,
    permissions: {
      canPrint: session.permissions.can_print,
      canCopy: session.permissions.can_copy,
      canEdit: session.permissions.can_edit,
      canAnnotate: session.permissions.can_annotate,
    },
    dirty: session.dirty,
    loadState: "ready",
    errorMessage: undefined,
  };
}
