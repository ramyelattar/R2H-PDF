import { useCallback, useRef, useState } from "react";
import type {
  EditorObject,
  EditorObjectId,
  EditorObjectPatch,
  EditorRect,
  EditorTool,
  EditorUndoEntry,
} from "./types";

interface EditorState {
  objects: Map<EditorObjectId, EditorObject>;
  selectedIds: EditorObjectId[];
  activeTool: EditorTool;
  undoStack: EditorUndoEntry[];
  redoStack: EditorUndoEntry[];
}

const initialState: EditorState = {
  objects: new Map(),
  selectedIds: [],
  activeTool: "select",
  undoStack: [],
  redoStack: [],
};

const MAX_UNDO_DEPTH = 100;

export function usePdfEditorState() {
  const [state, setState] = useState<EditorState>(initialState);
  // Use a ref for the latest state to avoid stale closures in callbacks.
  const stateRef = useRef(state);
  stateRef.current = state;

  const objectsForPage = useCallback((sessionId: string, pageIndex: number): EditorObject[] => {
    const result: EditorObject[] = [];
    for (const obj of stateRef.current.objects.values()) {
      if (obj.sessionId === sessionId && obj.pageIndex === pageIndex && !obj.hidden) {
        result.push(obj);
      }
    }
    return result.sort((a, b) => a.zIndex - b.zIndex);
  }, []);

  const pushUndo = (entry: EditorUndoEntry, current: EditorState): EditorState => {
    const undoStack = [...current.undoStack, entry].slice(-MAX_UNDO_DEPTH);
    return { ...current, undoStack, redoStack: [] };
  };

  const addObject = useCallback((object: EditorObject) => {
    setState((prev) => {
      const objects = new Map(prev.objects);
      objects.set(object.id, object);
      const entry: EditorUndoEntry = { actionType: "add", objectId: object.id, before: null, after: object };
      return pushUndo(entry, { ...prev, objects, selectedIds: [object.id] });
    });
  }, []);

  /** Replace the current document projection after a durable project load. */
  const replaceObjects = useCallback((objects: EditorObject[]) => {
    const nextObjects = new Map<EditorObjectId, EditorObject>();
    for (const object of objects) {
      if (object.id) nextObjects.set(object.id, object);
    }
    setState((prev) => ({
      ...prev,
      objects: nextObjects,
      selectedIds: [],
      undoStack: [],
      redoStack: [],
    }));
  }, []);

  const selectObject = useCallback((objectId: EditorObjectId, mode: "replace" | "toggle" = "replace") => {
    setState((prev) => {
      if (mode === "toggle") {
        const has = prev.selectedIds.includes(objectId);
        const selectedIds = has
          ? prev.selectedIds.filter((id) => id !== objectId)
          : [...prev.selectedIds, objectId];
        return { ...prev, selectedIds };
      }
      return { ...prev, selectedIds: [objectId] };
    });
  }, []);

  const clearSelection = useCallback(() => {
    setState((prev) => ({ ...prev, selectedIds: [] }));
  }, []);

  const updateObjectRect = useCallback((objectId: EditorObjectId, rect: EditorRect) => {
    setState((prev) => {
      const existing = prev.objects.get(objectId);
      if (!existing || existing.locked) return prev;

      const updated = { ...existing, rect, updatedAt: Date.now() };
      const objects = new Map(prev.objects);
      objects.set(objectId, updated);

      const entry: EditorUndoEntry = { actionType: "move", objectId, before: existing, after: updated };
      return pushUndo(entry, { ...prev, objects });
    });
  }, []);

  const patchObject = useCallback((objectId: EditorObjectId, patch: EditorObjectPatch) => {
    setState((prev) => {
      const existing = prev.objects.get(objectId);
      if (!existing) return prev;

      const updated = { ...existing, ...patch, updatedAt: Date.now() } as EditorObject;
      if (patch.rect) updated.rect = patch.rect;
      const objects = new Map(prev.objects);
      objects.set(objectId, updated);

      const entry: EditorUndoEntry = { actionType: "patch", objectId, before: existing, after: updated };
      return pushUndo(entry, { ...prev, objects });
    });
  }, []);

  const deleteObject = useCallback((objectId: EditorObjectId) => {
    setState((prev) => {
      const existing = prev.objects.get(objectId);
      if (!existing) return prev;

      const objects = new Map(prev.objects);
      objects.delete(objectId);
      const selectedIds = prev.selectedIds.filter((id) => id !== objectId);

      const entry: EditorUndoEntry = { actionType: "delete", objectId, before: existing, after: null };
      return pushUndo(entry, { ...prev, objects, selectedIds });
    });
  }, []);

  const setActiveTool = useCallback((tool: EditorTool) => {
    setState((prev) => ({ ...prev, activeTool: tool }));
  }, []);

  const undo = useCallback(() => {
    setState((prev) => {
      if (prev.undoStack.length === 0) return prev;
      const entry = prev.undoStack[prev.undoStack.length - 1];
      const undoStack = prev.undoStack.slice(0, -1);
      const redoStack = [...prev.redoStack, entry];
      const objects = new Map(prev.objects);

      if (entry.actionType === "add") {
        // Undo add = remove the object.
        objects.delete(entry.objectId);
      } else if (entry.actionType === "delete") {
        // Undo delete = restore the object.
        if (entry.before) objects.set(entry.objectId, entry.before);
      } else {
        // Undo move/resize/patch = restore before state.
        if (entry.before) objects.set(entry.objectId, entry.before);
      }

      const selectedIds = prev.selectedIds.filter((id) => objects.has(id));
      return { ...prev, objects, undoStack, redoStack, selectedIds };
    });
  }, []);

  const redo = useCallback(() => {
    setState((prev) => {
      if (prev.redoStack.length === 0) return prev;
      const entry = prev.redoStack[prev.redoStack.length - 1];
      const redoStack = prev.redoStack.slice(0, -1);
      const undoStack = [...prev.undoStack, entry];
      const objects = new Map(prev.objects);

      if (entry.actionType === "add") {
        // Redo add = re-add the object.
        if (entry.after) objects.set(entry.objectId, entry.after);
      } else if (entry.actionType === "delete") {
        // Redo delete = remove again.
        objects.delete(entry.objectId);
      } else {
        // Redo move/resize/patch = apply after state.
        if (entry.after) objects.set(entry.objectId, entry.after);
      }

      const selectedIds = prev.selectedIds.filter((id) => objects.has(id));
      return { ...prev, objects, undoStack, redoStack, selectedIds };
    });
  }, []);

  return {
    objects: state.objects,
    selectedIds: state.selectedIds,
    activeTool: state.activeTool,
    canUndo: state.undoStack.length > 0,
    canRedo: state.redoStack.length > 0,
    objectsForPage,
    addObject,
    replaceObjects,
    selectObject,
    clearSelection,
    updateObjectRect,
    patchObject,
    deleteObject,
    setActiveTool,
    undo,
    redo,
  };
}
