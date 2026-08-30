/**
 * Phase 25A — unified Objects/Layers panel types.
 *
 * Objects from many different subsystems are merged into a single list:
 *   - Overlay editor objects (text boxes, comments, highlights, shapes,
 *     stamps, redactions, images, strikethroughs, underlines).
 *   - Form fields (AcroForm) — read from the backend.
 *   - Content edit history entries — read from the backend.
 *   - Native content objects on the current page (text spans, image
 *     XObjects, paths) — read from the backend Edit Content analysis.
 *
 * The panel is intentionally read-mostly: actions are limited to those
 * each source supports, and unsupported actions are presented disabled
 * with an explicit reason so the user is never staring at a dead button.
 */

export type ObjectSource =
  | "manual"
  | "ai"
  | "compare"
  | "compare_visual"
  | "ai_review"
  | "form"
  | "content_edit"
  | "signature"
  | "ocr"
  | "native_content";

export type ObjectKind =
  // Overlay editor object types
  | "textBox"
  | "comment"
  | "highlight"
  | "rectangle"
  | "redaction"
  | "stamp"
  | "image"
  | "strikethrough"
  | "underline"
  // Backend object kinds
  | "form_field"
  | "content_edit"
  | "native_text"
  | "native_image"
  | "native_path";

export interface ObjectRow {
  /** Unique id within the panel; collisions are forbidden. */
  id: string;
  /** Original id from the underlying source (overlay id, field name, edit id...). */
  sourceId: string;
  source: ObjectSource;
  kind: ObjectKind;
  label: string;
  pageIndex: number;
  /** Optional bbox in PDF point space [x0, y0, x1, y1]. */
  bbox?: [number, number, number, number];
  /** Overlay objects expose hidden/locked flags. Backend rows leave them undefined. */
  hidden?: boolean;
  locked?: boolean;
  /** Free-form status string (e.g. form value, edit method, OCR confidence). */
  status?: string;
  warnings?: string[];
  /** Phase 27B — overlay zIndex, surfaced so the panel can show it and the
   * reorder buttons can compute deltas. Undefined for non-overlay rows. */
  zIndex?: number;
}

export type ObjectFilterSource = "all" | ObjectSource;
export type ObjectFilterKind = "all" | ObjectKind;

export interface ObjectsFilter {
  search: string;
  page: "all" | number; // "all" or a 0-based page index
  source: ObjectFilterSource;
  kind: ObjectFilterKind;
}

export interface ObjectsActionsAvailability {
  canSelect: boolean;
  canJump: boolean;
  canHide: boolean;
  canLock: boolean;
  canDelete: boolean;
  /** Phase 27B — z-order reordering availability. */
  canReorder: boolean;
  /** Reason shown when an action is disabled (UX honesty). */
  disabledReason?: string;
  /** Phase 27B — specific reason why reorder is unavailable, if it is. */
  reorderDisabledReason?: string;
}

/** Phase 27B — z-order operations available on overlay rows. */
export type ZOrderOp = "forward" | "backward" | "front" | "back";

export const DEFAULT_FILTER: ObjectsFilter = {
  search: "",
  page: "all",
  source: "all",
  kind: "all",
};
