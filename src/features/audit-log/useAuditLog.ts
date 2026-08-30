import { useCallback, useRef, useState } from "react";
import type { AuditAction, AuditLogEntry } from "./types";

const MAX_ENTRIES = 500;
const STORAGE_KEY = "r2h-audit-log";

function generateId(): string {
  return `audit-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

export function useAuditLog() {
  const [entries, setEntries] = useState<AuditLogEntry[]>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) return JSON.parse(stored) as AuditLogEntry[];
    } catch { /* ignore */ }
    return [];
  });

  const entriesRef = useRef(entries);
  entriesRef.current = entries;

  const persist = useCallback((updated: AuditLogEntry[]) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(updated.slice(-MAX_ENTRIES)));
    } catch { /* ignore quota */ }
  }, []);

  const record = useCallback((
    action: AuditAction,
    objectId: string,
    objectType: string,
    pageIndex: number,
    detail: string,
    actor: "user" | "ai" | "system" | "compare" = "user",
  ) => {
    const entry: AuditLogEntry = {
      id: generateId(),
      timestamp: Date.now(),
      action,
      objectId,
      objectType,
      pageIndex,
      detail,
      actor,
    };
    setEntries((prev) => {
      const updated = [...prev, entry].slice(-MAX_ENTRIES);
      persist(updated);
      return updated;
    });
  }, [persist]);

  const clear = useCallback(() => {
    setEntries([]);
    try { localStorage.removeItem(STORAGE_KEY); } catch { /* ignore */ }
  }, []);

  return { entries, record, clear };
}
