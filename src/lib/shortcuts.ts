import type { ReaderMode } from "../types/shell";

export const sessionStorageKey = "r2h-shell-session-v1";

export const navShortcuts: Record<string, ReaderMode> = {
  "ctrl+1": "read",
  "ctrl+2": "review",
  "ctrl+3": "annotate",
};

export const formatShortcut = (event: KeyboardEvent): string => {
  const parts: string[] = [];

  if (event.ctrlKey || event.metaKey) {
    parts.push("ctrl");
  }
  if (event.shiftKey) {
    parts.push("shift");
  }
  if (event.altKey) {
    parts.push("alt");
  }

  parts.push(event.key.toLowerCase());
  return parts.join("+");
};
