/**
 * Editor Object Model for the PDF editor overlay.
 *
 * ## Coordinate System
 *
 * All editor object rects are stored in **PDF page coordinates** (points).
 * - Origin: bottom-left of the page (per PDF spec).
 * - Units: 1 point = 1/72 inch.
 * - x increases rightward, y increases upward.
 *
 * Conversion to screen coordinates happens only at render time using:
 *   screenX = pdfX * zoom * dpr
 *   screenY = (pageHeight - pdfY - height) * zoom * dpr  (Y-flip for top-left origin)
 *
 * This keeps the object model resolution-independent and aligned with
 * the backend's MuPDF coordinate system for future annotation write-back.
 */

export type EditorObjectId = string;

export type EditorObjectType =
  | "textBox"
  | "comment"
  | "highlight"
  | "rectangle"
  | "redaction"
  | "stamp"
  | "image"
  | "strikethrough"
  | "underline";

export type EditorTool =
  | "select"
  | "textBox"
  | "comment"
  | "highlight"
  | "rectangle"
  | "redaction"
  | "stamp";

export interface EditorRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface EditorObjectBase {
  id: EditorObjectId;
  sessionId: string;
  pageIndex: number;
  type: EditorObjectType;
  rect: EditorRect;
  rotation: number;
  zIndex: number;
  locked: boolean;
  hidden: boolean;
  createdAt: number;
  updatedAt: number;
  metadata: Record<string, unknown>;
}

export interface TextBoxObject extends EditorObjectBase {
  type: "textBox";
  text: string;
  fontSize: number;
  fontFamily: string;
  color: string;
  backgroundColor: string;
  borderColor: string;
}

export interface CommentObject extends EditorObjectBase {
  type: "comment";
  contents: string;
  author: string;
  color: string;
  status: "open" | "resolved";
}

export interface HighlightObject extends EditorObjectBase {
  type: "highlight";
  color: string;
  opacity: number;
  contents: string;
}

export interface ShapeObject extends EditorObjectBase {
  type: "rectangle";
  strokeColor: string;
  strokeWidth: number;
  fillColor: string;
}

export type RedactionStatus = "draft" | "applied";

export interface RedactionObject extends EditorObjectBase {
  type: "redaction";
  fillColor: string;
  replacementText: string;
  reason: string;
  status: RedactionStatus;
}

/**
 * Phase 25C: stamp presets cover both review and engineering workflows.
 * Adobe ships these as standard rubber-stamps; we render them as Stamp
 * annotations with the preset name embedded in the appearance.
 */
export type StampType =
  | "APPROVED"
  | "APPROVED_AS_NOTED"
  | "REJECTED"
  | "REVIEWED"
  | "DRAFT"
  | "FOR_CONSTRUCTION"
  | "AS_BUILT"
  | "REVISE_AND_RESUBMIT"
  | "CONFIDENTIAL"
  | "VOID"
  | "CUSTOM";

export interface StampObject extends EditorObjectBase {
  type: "stamp";
  stampText: string;
  stampType: StampType;
  color: string;
}

export interface ImageObject extends EditorObjectBase {
  type: "image";
  imageRef: string;
  originalWidth: number;
  originalHeight: number;
}

/**
 * Phase 25A: native redline appearance for compare-driven annotations.
 * Backend maps `strikethrough` to `AnnotationType::Strikeout` and
 * `underline` to `AnnotationType::Underline` so reviewers see proper
 * markup in any PDF viewer.
 */
export interface StrikethroughObject extends EditorObjectBase {
  type: "strikethrough";
  color: string;
  contents: string;
}

export interface UnderlineObject extends EditorObjectBase {
  type: "underline";
  color: string;
  contents: string;
}

export type EditorObject =
  | TextBoxObject
  | CommentObject
  | HighlightObject
  | ShapeObject
  | RedactionObject
  | StampObject
  | ImageObject
  | StrikethroughObject
  | UnderlineObject;

export interface PageEditState {
  sessionId: string;
  pageIndex: number;
  objects: EditorObject[];
  dirty: boolean;
}

export interface EditorSelectionState {
  selectedIds: EditorObjectId[];
  primaryId: EditorObjectId | null;
}

export interface EditorObjectPatch {
  rect?: EditorRect;
  rotation?: number;
  zIndex?: number;
  locked?: boolean;
  hidden?: boolean;
  metadata?: Record<string, unknown>;
  // TextBox
  text?: string;
  fontSize?: number;
  fontFamily?: string;
  color?: string;
  backgroundColor?: string;
  borderColor?: string;
  // Comment
  contents?: string;
  author?: string;
  status?: "open" | "resolved" | RedactionStatus;
  // Highlight
  opacity?: number;
  // Shape
  strokeColor?: string;
  strokeWidth?: number;
  fillColor?: string;
  // Redaction
  replacementText?: string;
  reason?: string;
  // Stamp
  stampText?: string;
  stampType?: StampType;
}

// --- Undo/Redo ---

export type EditorActionType = "add" | "delete" | "move" | "resize" | "patch";

export interface EditorUndoEntry {
  actionType: EditorActionType;
  objectId: EditorObjectId;
  before: EditorObject | null;
  after: EditorObject | null;
}
