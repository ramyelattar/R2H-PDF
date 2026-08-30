import { useEffect, useRef } from "react";
import { sessionStorageKey } from "../lib/shortcuts";
import type { ShellSessionSnapshot } from "../types/shell";

const isSnapshot = (value: unknown): value is ShellSessionSnapshot => {
  if (!value || typeof value !== "object") {
    return false;
  }

  const candidate = value as Partial<ShellSessionSnapshot>;
  return (
    typeof candidate.activeWorkspaceId === "string" &&
    Array.isArray(candidate.openTabs) &&
    typeof candidate.preferences === "object"
  );
};

interface SessionPersistenceProps {
  sessionSnapshot: ShellSessionSnapshot;
  restoreLastSession: boolean;
  onRestore: (snapshot: ShellSessionSnapshot) => void;
}

export const useSessionPersistence = ({
  sessionSnapshot,
  restoreLastSession,
  onRestore,
}: SessionPersistenceProps) => {
  const restoreRef = useRef(onRestore);

  useEffect(() => {
    restoreRef.current = onRestore;
  }, [onRestore]);

  useEffect(() => {
    if (!restoreLastSession) {
      return;
    }

    try {
      const stored = localStorage.getItem(sessionStorageKey);
      if (!stored) {
        return;
      }

      const parsed = JSON.parse(stored);
      if (isSnapshot(parsed)) {
        restoreRef.current(parsed);
      }
    } catch {
      // Ignore broken local session data.
    }
  }, [restoreLastSession]);

  useEffect(() => {
    try {
      localStorage.setItem(sessionStorageKey, JSON.stringify(sessionSnapshot));
    } catch {
      // Ignore storage failures in constrained environments.
    }
  }, [sessionSnapshot]);
};
