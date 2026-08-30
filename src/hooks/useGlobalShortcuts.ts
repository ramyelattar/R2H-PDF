import { useEffect, useRef } from "react";
import { formatShortcut, navShortcuts } from "../lib/shortcuts";
import type { ReaderMode } from "../types/shell";

interface ShortcutActions {
  openCommandPalette: () => void;
  openPreferences: () => void;
  toggleDiagnostics: () => void;
  closeActiveTab: () => void;
  nextTab: () => void;
  previousTab: () => void;
  nextPage: () => void;
  previousPage: () => void;
  zoomIn: () => void;
  zoomOut: () => void;
  setMode: (mode: ReaderMode) => void;
  onNextMatch?: () => void;
  onPreviousMatch?: () => void;
}

const isEditableTarget = (target: EventTarget | null) => {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tagName = target.tagName.toLowerCase();
  return (
    target.isContentEditable ||
    tagName === "input" ||
    tagName === "textarea" ||
    tagName === "select"
  );
};

export const useGlobalShortcuts = (actions: ShortcutActions) => {
  // Keep a ref so the stable keydown listener always invokes the latest
  // callbacks without needing to re-register on every render.
  const actionsRef = useRef(actions);
  useEffect(() => {
    actionsRef.current = actions;
  }, [actions]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const shortcut = formatShortcut(event);
      const a = actionsRef.current;
      const isEditingText = isEditableTarget(event.target);

      if (shortcut in navShortcuts) {
        event.preventDefault();
        a.setMode(navShortcuts[shortcut]);
        return;
      }

      switch (shortcut) {
        case "ctrl+k":
          event.preventDefault();
          a.openCommandPalette();
          break;
        case "ctrl+,":
          event.preventDefault();
          a.openPreferences();
          break;
        case "ctrl+shift+d":
          event.preventDefault();
          a.toggleDiagnostics();
          break;
        case "ctrl+w":
          event.preventDefault();
          a.closeActiveTab();
          break;
        case "ctrl+tab":
          event.preventDefault();
          a.nextTab();
          break;
        case "ctrl+shift+tab":
          event.preventDefault();
          a.previousTab();
          break;
        case "arrowleft":
          if (!isEditingText) {
            event.preventDefault();
            a.previousPage();
          }
          break;
        case "arrowright":
          if (!isEditingText) {
            event.preventDefault();
            a.nextPage();
          }
          break;
        case "+":
        case "ctrl++":
        case "ctrl+=":
        case "shift+=":
          if (!isEditingText) {
            event.preventDefault();
            a.zoomIn();
          }
          break;
        case "-":
        case "ctrl+-":
          if (!isEditingText) {
            event.preventDefault();
            a.zoomOut();
          }
          break;
        case "f3":
          event.preventDefault();
          a.onNextMatch?.();
          break;
        case "shift+f3":
          event.preventDefault();
          a.onPreviousMatch?.();
          break;
        case "ctrl+g":
          event.preventDefault();
          a.onNextMatch?.();
          break;
        case "ctrl+shift+g":
          event.preventDefault();
          a.onPreviousMatch?.();
          break;
        default:
          break;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
    };
  }, []); // Mount/unmount only — actionsRef keeps callbacks fresh.
};
