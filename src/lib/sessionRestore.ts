import type { ShellSessionSnapshot } from "../types/shell";

export interface PdfTabToReopen {
  tabId: string;
  path: string;
}

export interface PreparedRestoredSession {
  snapshot: ShellSessionSnapshot;
  pdfPathsToReopen: string[];
  pdfTabsToReopen: PdfTabToReopen[];
}

const isReopenablePdfTab = (tab: ShellSessionSnapshot["openTabs"][number]) =>
  tab.kind === "pdf" && !tab.sourcePath.startsWith("session://");

/**
 * Identify a tab id as a placeholder for a PDF that needs to be reopened from
 * disk. Such ids must NOT pass `isBackendPdfSession()` (which checks for the
 * `doc-session-` prefix), so the viewer never attempts an IPC render against a
 * dead backend session.
 */
export const PENDING_REOPEN_PREFIX = "pending-reopen-";

export const isPendingReopenTabId = (tabId: string): boolean =>
  tabId.startsWith(PENDING_REOPEN_PREFIX);

let pendingReopenSeq = 0;
const allocatePendingReopenId = (): string => {
  pendingReopenSeq += 1;
  return `${PENDING_REOPEN_PREFIX}${Date.now()}-${pendingReopenSeq}`;
};

export const prepareRestoredSession = (
  snapshot: ShellSessionSnapshot,
): PreparedRestoredSession => {
  // Older builds persisted synthetic review, handoff, and export shells.
  // They are not documents and must not be revived into the production tab
  // graph now that these workflows are either document-gated panels or have
  // no implemented durable record flow.
  const withoutSyntheticShellTabs = snapshot.openTabs.filter(
    (tab) => !(
      tab.sourcePath.startsWith("session://review") ||
      tab.sourcePath.startsWith("session://handoff") ||
      tab.sourcePath.startsWith("session://export")
    ),
  );
  // Re-id every reopenable PDF tab so the stale `doc-session-XYZ` id from the
  // persisted snapshot cannot be mistaken for a live backend session. Without
  // this step, `isBackendPdfSession()` returns true for stale ids, and any
  // code path that fires IPC against the active tab id (autosave, render,
  // search) would silently hit `SESSION_NOT_FOUND` and leave the UI stuck.
  const idMap = new Map<string, string>();
  const openTabs = withoutSyntheticShellTabs.map((tab) => {
    if (!isReopenablePdfTab(tab)) {
      return tab;
    }
    const newId = allocatePendingReopenId();
    idMap.set(tab.id, newId);
    return {
      ...tab,
      id: newId,
      loadState: "loading" as const,
      dirty: false,
      errorMessage: undefined,
    };
  });

  const pdfTabsToReopen = openTabs
    .filter(isReopenablePdfTab)
    .map((tab) => ({ tabId: tab.id, path: tab.sourcePath }));

  const pdfPathsToReopen = pdfTabsToReopen.map((entry) => entry.path);

  const activeTabId =
    snapshot.activeTabId && idMap.has(snapshot.activeTabId)
      ? (idMap.get(snapshot.activeTabId) ?? snapshot.activeTabId)
      : openTabs.some((tab) => tab.id === snapshot.activeTabId)
        ? snapshot.activeTabId
        : openTabs[0]?.id ?? null;

  return {
    snapshot: {
      ...snapshot,
      activeTabId,
      openTabs,
    },
    pdfPathsToReopen,
    pdfTabsToReopen,
  };
};
