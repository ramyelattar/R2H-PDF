import { useEffect, useRef } from "react";

interface EditorKeyboardActions {
  deleteSelected: () => void;
  clearSelection: () => void;
  undo: () => void;
  redo: () => void;
  hasSelection: boolean;
  /** When true, editor keyboard shortcuts are active. */
  editorActive: boolean;
}

/**
 * Keyboard shortcuts for the editor overlay.
 * Only active when `editorActive` is true (editor mode is engaged).
 * Does not interfere with existing global shortcuts.
 */
export function useEditorKeyboard(actions: EditorKeyboardActions) {
  const actionsRef = useRef(actions);
  actionsRef.current = actions;

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const a = actionsRef.current;
      if (!a.editorActive) return;

      // Don't intercept when typing in inputs.
      const target = e.target as HTMLElement;
      if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) return;

      if ((e.key === "Delete" || e.key === "Backspace") && a.hasSelection) {
        e.preventDefault();
        a.deleteSelected();
        return;
      }

      if (e.key === "Escape" && a.hasSelection) {
        e.preventDefault();
        a.clearSelection();
        return;
      }

      // Editor undo/redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z)
      if (e.ctrlKey && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        a.undo();
        return;
      }
      if ((e.ctrlKey && e.key === "y") || (e.ctrlKey && e.shiftKey && e.key === "Z")) {
        e.preventDefault();
        a.redo();
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown, true); // capture phase
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);
}
