import { useRef } from "react";
import { useSessionPersistence } from "./useSessionPersistence";
import { prepareRestoredSession } from "../lib/sessionRestore";
import type { ShellSessionSnapshot, DocumentTab } from "../types/shell";

interface UseSessionRestoreFlowDeps {
  sessionSnapshot: ShellSessionSnapshot;
  restoreLastSession: boolean;
  restoreSession: (snapshot: ShellSessionSnapshot) => void;
  closeTab: (tabId: string) => void;
  setTabLoadState: (tabId: string, loadState: DocumentTab["loadState"], errorMessage?: string) => void;
  openPdfPath: (path: string) => Promise<string | null>;
}

export function useSessionRestoreFlow(deps: UseSessionRestoreFlowDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  useSessionPersistence({
    sessionSnapshot: deps.sessionSnapshot,
    restoreLastSession: deps.restoreLastSession,
    onRestore: (snapshot) => {
      const { restoreSession, closeTab, setTabLoadState, openPdfPath } = depsRef.current;
      const restored = prepareRestoredSession(snapshot);
      restoreSession(restored.snapshot);

      void (async () => {
        for (const { tabId, path } of restored.pdfTabsToReopen) {
          const sessionId = await openPdfPath(path);
          if (sessionId) {
            closeTab(tabId);
          } else {
            setTabLoadState(tabId, "error", `Failed to open ${path}`);
          }
        }
      })();
    },
  });
}
