import { useCallback, useEffect, useMemo, useState } from "react";
import {
  docListFormFields,
  pdfGetPageContentObjects,
  pdfListContentEdits,
  type ContentObject,
  type ContentEditRecord,
  type FormField,
} from "../../lib/ipc";
import type { EditorObject } from "../pdf-editor/types";
import {
  DEFAULT_FILTER,
  type ObjectKind,
  type ObjectRow,
  type ObjectSource,
  type ObjectsActionsAvailability,
  type ObjectsFilter,
  type ZOrderOp,
} from "./types";

interface UseObjectsPanelOptions {
  sessionId: string;
  /** Snapshot of editor overlay objects from `usePdfEditorState`. */
  overlayObjects: EditorObject[];
  /** Currently active page (0-based). Drives the native-content row set. */
  activePageIndex: number;
}

/**
 * Aggregates objects from every source the app knows about into a single,
 * filterable, searchable list.
 *
 * `refresh()` re-pulls backend data (form fields, content-edit history,
 * native content objects on the active page).
 */
export function useObjectsPanel({
  sessionId,
  overlayObjects,
  activePageIndex,
}: UseObjectsPanelOptions) {
  const [filter, setFilter] = useState<ObjectsFilter>(DEFAULT_FILTER);
  const [formFields, setFormFields] = useState<FormField[]>([]);
  const [contentEdits, setContentEdits] = useState<ContentEditRecord[]>([]);
  const [nativeObjects, setNativeObjects] = useState<ContentObject[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!sessionId) return;
    setLoading(true);
    setError(null);
    const [ff, ce, nc] = await Promise.all([
      docListFormFields(sessionId),
      pdfListContentEdits(sessionId),
      pdfGetPageContentObjects(sessionId, activePageIndex),
    ]);
    if (ff.ok) setFormFields(ff.data); else setError(ff.error.message);
    if (ce.ok) setContentEdits(ce.data);
    if (nc.ok) setNativeObjects(nc.data);
    setLoading(false);
  }, [sessionId, activePageIndex]);

  useEffect(() => { void refresh(); }, [refresh]);

  // ----- aggregate into a single ObjectRow[] -----
  const allRows: ObjectRow[] = useMemo(() => {
    const rows: ObjectRow[] = [];

    for (const obj of overlayObjects) {
      const source = inferOverlaySource(obj);
      rows.push({
        id: `overlay:${obj.id}`,
        sourceId: obj.id,
        source,
        kind: overlayKind(obj),
        label: overlayLabel(obj),
        pageIndex: obj.pageIndex,
        bbox: [
          obj.rect.x,
          obj.rect.y,
          obj.rect.x + obj.rect.width,
          obj.rect.y + obj.rect.height,
        ],
        hidden: obj.hidden,
        locked: obj.locked,
        status: overlayStatus(obj),
        zIndex: obj.zIndex,
      });
    }

    for (const field of formFields) {
      rows.push({
        id: `form:${field.name}`,
        sourceId: field.name,
        source: "form",
        kind: "form_field",
        label: `${field.name} (${field.field_type})`,
        pageIndex: field.page_index,
        bbox: field.rect,
        status: field.value || "(empty)",
      });
    }

    for (const edit of contentEdits) {
      rows.push({
        id: `cedit:${edit.edit_id}`,
        sourceId: edit.edit_id,
        source: "content_edit",
        kind: "content_edit",
        label: `${formatContentEditType(edit.edit_type)}: ${edit.after_summary || edit.before_summary || edit.edit_id}`,
        pageIndex: edit.page_index,
        status: `${formatEditMethod(edit.method)}${edit.reversible ? " · reversible" : ""}`,
        warnings: edit.warnings && edit.warnings.length > 0 ? edit.warnings : undefined,
      });
    }

    for (const obj of nativeObjects) {
      rows.push({
        id: `native:${obj.id}`,
        sourceId: obj.id,
        source: "native_content",
        kind: nativeKindFor(obj),
        label: nativeLabelFor(obj),
        pageIndex: obj.page_index,
        bbox: obj.bbox,
        status: obj.editable_level,
        warnings: obj.diagnostics && obj.diagnostics.length > 0 ? obj.diagnostics : undefined,
      });
    }

    return rows;
  }, [overlayObjects, formFields, contentEdits, nativeObjects]);

  // ----- filter -----
  const filteredRows: ObjectRow[] = useMemo(() => {
    const q = filter.search.trim().toLowerCase();
    return allRows.filter((row) => {
      if (filter.source !== "all" && row.source !== filter.source) return false;
      if (filter.kind !== "all" && row.kind !== filter.kind) return false;
      if (filter.page !== "all" && row.pageIndex !== filter.page) return false;
      if (q) {
        const hay = `${row.label} ${row.status ?? ""} ${row.kind} ${row.source}`.toLowerCase();
        if (!hay.includes(q)) return false;
      }
      return true;
    });
  }, [allRows, filter]);

  // ----- per-row action availability -----
  const actionsFor = useCallback((row: ObjectRow): ObjectsActionsAvailability => {
    // Overlay objects support the full set.
    if (row.id.startsWith("overlay:")) {
      const locked = row.locked === true;
      return {
        canSelect: true,
        canJump: true,
        canHide: true,
        canLock: true,
        canDelete: !locked,
        canReorder: !locked,
        disabledReason: locked ? "Object is locked. Unlock first." : undefined,
        reorderDisabledReason: locked ? "Object is locked. Unlock first." : undefined,
      };
    }
    // Form fields: jump only — edits route to Forms panel.
    if (row.id.startsWith("form:")) {
      return {
        canSelect: false,
        canJump: true,
        canHide: false,
        canLock: false,
        canDelete: false,
        canReorder: false,
        disabledReason: "Form field — edit via the Forms tab.",
        reorderDisabledReason: "Form fields cannot be reordered from this panel.",
      };
    }
    // Content edits: jump + (future) revert; no overlay actions.
    if (row.id.startsWith("cedit:")) {
      return {
        canSelect: false,
        canJump: true,
        canHide: false,
        canLock: false,
        canDelete: false,
        canReorder: false,
        disabledReason: "Content edit — revert from the Edit Content panel.",
        reorderDisabledReason: "Content edit entries are history, not editable objects.",
      };
    }
    // Native content: read-only here.
    if (row.id.startsWith("native:")) {
      return {
        canSelect: false,
        canJump: true,
        canHide: false,
        canLock: false,
        canDelete: false,
        canReorder: false,
        disabledReason:
          "Native PDF content cannot be deleted from the Objects panel. Use the Edit Content panel.",
        reorderDisabledReason:
          "Native PDF content cannot be reordered from this panel — z-order is fixed by the source stream.",
      };
    }
    return {
      canSelect: false, canJump: false, canHide: false,
      canLock: false, canDelete: false, canReorder: false,
      disabledReason: "Unknown object source.",
    };
  }, []);

  /**
   * Phase 27B — compute the new zIndex for a reorder op against the
   * current overlay snapshot. Returns null when the op would be a no-op
   * (already at the extreme) so callers can skip dispatch.
   *
   * Rules:
   *   - "forward": +1 (clamped to current max + 1)
   *   - "backward": -1 (clamped to 0)
   *   - "front":   max(allZ) + 1
   *   - "back":    min(allZ) - 1, or 0 if that would go negative
   */
  const computeReorderZIndex = useCallback((sourceId: string, op: ZOrderOp): number | null => {
    const obj = overlayObjects.find((o) => o.id === sourceId);
    if (!obj || obj.locked) return null;
    // Consider only overlay objects on the same page for z-order math.
    const peers = overlayObjects.filter((o) => o.pageIndex === obj.pageIndex);
    const allZ = peers.map((o) => o.zIndex);
    const minZ = Math.min(...allZ);
    const maxZ = Math.max(...allZ);
    switch (op) {
      case "forward": {
        if (obj.zIndex >= maxZ) return null;
        return obj.zIndex + 1;
      }
      case "backward": {
        if (obj.zIndex <= 0 && obj.zIndex <= minZ) return null;
        return Math.max(0, obj.zIndex - 1);
      }
      case "front": {
        if (peers.length <= 1) return null;
        if (obj.zIndex >= maxZ && obj.zIndex > 0) return null;
        return maxZ + 1;
      }
      case "back": {
        if (peers.length <= 1) return null;
        // Already at back (the only one with min z) → no-op.
        const atBack = obj.zIndex === minZ && peers.filter((p) => p.zIndex === minZ).length === 1;
        if (atBack && obj.zIndex === 0) return null;
        return Math.max(0, minZ - 1);
      }
      default:
        return null;
    }
  }, [overlayObjects]);

  return {
    loading,
    error,
    filter,
    setFilter,
    allRows,
    filteredRows,
    actionsFor,
    refresh,
    computeReorderZIndex,
  };
}

// ---------- helpers ----------

function inferOverlaySource(obj: EditorObject): ObjectSource {
  const tag = typeof obj.metadata?.source === "string" ? (obj.metadata.source as string) : "";
  if (tag === "compare") return "compare";
  if (tag === "compare_visual") return "compare_visual";
  if (tag === "ai_review") return "ai_review";
  if (tag === "ai") return "ai";
  if (tag === "signature") return "signature";
  if (tag === "ocr") return "ocr";
  return "manual";
}

/** Editor types and panel `ObjectKind` align 1:1 for overlay objects. */
function overlayKind(obj: EditorObject): ObjectKind {
  return obj.type;
}

function overlayLabel(obj: EditorObject): string {
  switch (obj.type) {
    case "textBox":      return obj.text || "Text box";
    case "comment":      return obj.contents ? truncate(obj.contents, 60) : "Comment";
    case "highlight":    return obj.contents ? truncate(obj.contents, 60) : "Highlight";
    case "rectangle":    return "Rectangle";
    case "redaction":    return obj.reason ? `Redaction — ${obj.reason}` : "Redaction";
    case "stamp":        return obj.stampText || obj.stampType || "Stamp";
    case "image":        return `Image (${obj.originalWidth}×${obj.originalHeight})`;
    case "strikethrough":return obj.contents ? `Strikethrough — ${truncate(obj.contents, 40)}` : "Strikethrough";
    case "underline":    return obj.contents ? `Underline — ${truncate(obj.contents, 40)}` : "Underline";
    default:             return "Object";
  }
}

function overlayStatus(obj: EditorObject): string {
  const bits: string[] = [];
  if (obj.locked) bits.push("locked");
  if (obj.hidden) bits.push("hidden");
  return bits.join(" · ");
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + "…";
}

function nativeKindFor(obj: ContentObject): ObjectKind {
  switch (obj.object_type) {
    case "text_span":
    case "text_block":    return "native_text";
    case "image_xobject": return "native_image";
    case "path":          return "native_path";
    default:              return "native_text";
  }
}

function nativeLabelFor(obj: ContentObject): string {
  if (obj.text_info) {
    return `Text "${truncate(obj.text_info.decoded_text || obj.text_info.raw_text, 60)}"`;
  }
  if (obj.image_info) {
    return `Image XObject /${obj.image_info.xobject_name} (${obj.image_info.width}×${obj.image_info.height})`;
  }
  return `${obj.object_type} object`;
}

function formatContentEditType(type: string): string {
  switch (type) {
    case "NativeMultiOperator": return "Native block edit";
    case "SafeVisualReplacement": return "Visual replacement";
    case "XObjectSwap": return "Replace image";
    default: return type.replace(/_/g, " ");
  }
}

function formatEditMethod(method: string): string {
  switch (method) {
    case "native_multi_operator": return "Native block edit";
    case "safe_visual_replacement": return "Visual replacement";
    case "xobject_swap": return "Replace image";
    default: return method.replace(/_/g, " ");
  }
}
