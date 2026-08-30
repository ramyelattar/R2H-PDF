import type {
  CommentObject,
  EditorObject,
  HighlightObject,
  ShapeObject,
  StrikethroughObject,
  UnderlineObject,
} from "../pdf-editor/types";
import type { CompareTextChange, CompareVisualChange } from "../../lib/ipc";

const DEFAULT_RECT = { x: 36, y: 720, width: 240, height: 24 };

/**
 * Compare → editor-object kinds:
 * - `comment`: yellow sticky note at the change site
 * - `highlight`: colored overlay covering the change bbox
 * - `redline`: native PDF Strikeout (removed/modified base text) +
 *              Underline (added text); paired comment with full change detail
 */
export type CompareAdaptKind = "comment" | "highlight" | "redline";

let counter = 0;
const nextId = () => `cmp-obj-${Date.now()}-${++counter}-${Math.floor(Math.random() * 1000)}`;

function bboxToRect(bbox: [number, number, number, number] | null | undefined) {
  if (!bbox || bbox.length !== 4) return null;
  const [x0, y0, x1, y1] = bbox;
  const x = Math.min(x0, x1);
  const y = Math.min(y0, y1);
  const width = Math.abs(x1 - x0);
  const height = Math.abs(y1 - y0);
  if (width <= 0 || height <= 0) return null;
  return { x, y, width, height };
}

function changeSummaryLines(change: CompareTextChange): string {
  const lines: string[] = [];
  lines.push(`[Compare] ${change.change_type.toUpperCase()} (${change.citation})`);
  if (change.old_text) lines.push(`− ${change.old_text}`);
  if (change.new_text) lines.push(`+ ${change.new_text}`);
  return lines.join("\n");
}

/**
 * Convert a single text change into an editor overlay object.
 *
 * If the change carries a bbox, callers can choose `comment` (a sticky
 * note at the bbox top-left) or `highlight` (a colored highlight covering the
 * bbox). When no bbox is available, a comment is placed at the top of
 * the page with a message that says so.
 */
export function textChangeToEditorObject(
  sessionId: string,
  change: CompareTextChange,
  kind: CompareAdaptKind,
): EditorObject {
  const now = Date.now();
  const rectFromBbox = bboxToRect(change.bbox ?? null);
  const rect = rectFromBbox ?? DEFAULT_RECT;
  const baseMeta = {
    source: "compare",
    change_type: change.change_type,
    citation: change.citation,
  };

  if (kind === "highlight" && rectFromBbox) {
    const obj: HighlightObject = {
      id: nextId(),
      sessionId,
      pageIndex: change.page_index,
      type: "highlight",
      rect,
      rotation: 0,
      zIndex: 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: {
        ...baseMeta,
        compare_adapt_kind: kind,
      },
      color:
        change.change_type === "removed"
          ? "rgba(255,140,140,0.45)"
          : change.change_type === "added"
            ? "rgba(150,255,150,0.45)"
            : "rgba(255,210,120,0.45)",
      opacity: 0.45,
      contents: changeSummaryLines(change),
    };
    return obj;
  }

  // Phase 25A: redline kind produces native PDF Strikeout / Underline so
  // any reader (Acrobat, Foxit, PDF.js, etc.) sees Adobe-style change
  // markup, not just a paint-on-top highlight. Removed lines / the base
  // side of modified lines become strikethrough; added lines become
  // underline. Bbox-less changes degrade to the comment fallback below.
  if (kind === "redline" && rectFromBbox) {
    // Phase 27C — when the backend supplies a real bbox for the change,
    // record it in `metadata.line_bboxes` so EditableOverlay can render
    // an exact-width line per row instead of relying on the 14pt heuristic.
    // The backend currently emits one bbox per change; multi-line changes
    // arrive as multiple CompareTextChange rows so a single-entry array
    // is correct here.
    const redlineMeta = {
      ...baseMeta,
      compare_adapt_kind: "redline",
      line_bboxes: change.bbox ? [change.bbox as [number, number, number, number]] : undefined,
    };
    const wantStrike = change.change_type === "removed" || change.change_type === "modified";
    if (wantStrike) {
      const obj: StrikethroughObject = {
        id: nextId(),
        sessionId,
        pageIndex: change.page_index,
        type: "strikethrough",
        rect,
        rotation: 0,
        zIndex: 1,
        locked: false,
        hidden: false,
        createdAt: now,
        updatedAt: now,
        metadata: redlineMeta,
        color: "#c83232",
        contents: changeSummaryLines(change),
      };
      return obj;
    }
    // Added
    const obj: UnderlineObject = {
      id: nextId(),
      sessionId,
      pageIndex: change.page_index,
      type: "underline",
      rect,
      rotation: 0,
      zIndex: 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: redlineMeta,
      color: "#3aa888",
      contents: changeSummaryLines(change),
    };
    return obj;
  }

  // For bbox-less changes we always fall back to a comment at the page
  // top-left and prefix the contents with a clear note. This honours
  // 24A's "missing bbox creates page-level comment" rule.
  const contents = rectFromBbox
    ? changeSummaryLines(change)
    : `${changeSummaryLines(change)}\n(note: no bounding box on this change — placed at top of page)`;

  const comment: CommentObject = {
    id: nextId(),
    sessionId,
    pageIndex: change.page_index,
    type: "comment",
    rect: rectFromBbox ?? { ...DEFAULT_RECT, width: 24, height: 24 },
    rotation: 0,
    zIndex: 1,
    locked: false,
    hidden: false,
    createdAt: now,
    updatedAt: now,
    metadata: baseMeta,
    contents,
    author: "Compare",
    color:
      change.change_type === "added"
        ? "#5fb96b"
        : change.change_type === "removed"
          ? "#c66"
          : change.change_type === "modified"
            ? "#d99537"
            : "#7aa",
    status: "open",
  };
  return comment;
}

/** Convert a visual diff bbox into a translucent rectangle overlay. */
export function visualChangeToEditorObject(
  sessionId: string,
  change: CompareVisualChange,
): EditorObject {
  const now = Date.now();
  const rect = bboxToRect(change.bbox) ?? DEFAULT_RECT;
  const obj: ShapeObject = {
    id: nextId(),
    sessionId,
    pageIndex: change.page_index,
    type: "rectangle",
    rect,
    rotation: 0,
    zIndex: 1,
    locked: false,
    hidden: false,
    createdAt: now,
    updatedAt: now,
    metadata: {
      source: "compare_visual",
      confidence: change.confidence,
    },
    strokeColor: "#3060ff",
    strokeWidth: 2,
    fillColor: "rgba(120,160,255,0.15)",
  };
  return obj;
}

export function bulkTextChangesToEditorObjects(
  sessionId: string,
  changes: CompareTextChange[],
  kind: CompareAdaptKind,
): EditorObject[] {
  return changes.map((change) => textChangeToEditorObject(sessionId, change, kind));
}