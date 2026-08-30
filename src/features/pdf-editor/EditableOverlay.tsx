import { useCallback, useState } from "react";
import { pdfRectToScreen, screenRectToPdf, type PageDimensions, type ViewTransform } from "./coordinates";
import { ObjectSelection } from "./ObjectSelection";
import type { EditorObject, EditorObjectId, EditorObjectPatch, EditorRect, EditorTool } from "./types";

interface EditableOverlayProps {
  objects: EditorObject[];
  selectedIds: EditorObjectId[];
  activeTool: EditorTool;
  page: PageDimensions;
  view: ViewTransform;
  onSelectObject: (id: EditorObjectId) => void;
  onClearSelection: () => void;
  onMoveObject: (id: EditorObjectId, rect: EditorRect) => void;
  onResizeObject: (id: EditorObjectId, rect: EditorRect) => void;
  onCreateObject: (pdfRect: EditorRect) => void;
  onPatchObject?: (id: EditorObjectId, patch: EditorObjectPatch) => void;
}

export const EditableOverlay = ({
  objects,
  selectedIds,
  activeTool,
  page,
  view,
  onSelectObject,
  onClearSelection,
  onMoveObject,
  onResizeObject,
  onCreateObject,
  onPatchObject,
}: EditableOverlayProps) => {
  const containerWidth = page.widthPts * view.zoom;
  const containerHeight = page.heightPts * view.zoom;
  const [editingTextId, setEditingTextId] = useState<EditorObjectId | null>(null);

  const handleBackgroundClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (e.target !== e.currentTarget) return;
      if (activeTool !== "select") {
        const rect = e.currentTarget.getBoundingClientRect();
        const screenX = e.clientX - rect.left;
        const screenY = e.clientY - rect.top;
        const defaultWidth = 120;
        const defaultHeight = 40;
        const screenRect: EditorRect = { x: screenX, y: screenY, width: defaultWidth * view.zoom, height: defaultHeight * view.zoom };
        const pdfRect = screenRectToPdf(screenRect, page, view);
        onCreateObject(pdfRect);
      } else {
        setEditingTextId(null);
        onClearSelection();
      }
    },
    [activeTool, page, view, onClearSelection, onCreateObject],
  );

  const handleObjectClick = useCallback(
    (e: React.MouseEvent, id: EditorObjectId) => {
      e.stopPropagation();
      if (editingTextId && editingTextId !== id) setEditingTextId(null);
      onSelectObject(id);
    },
    [onSelectObject, editingTextId],
  );

  const handleObjectDoubleClick = useCallback(
    (e: React.MouseEvent, obj: EditorObject) => {
      e.stopPropagation();
      if (obj.type === "textBox") {
        setEditingTextId(obj.id);
      }
    },
    [],
  );

  const handleTextCommit = useCallback(
    (id: EditorObjectId, newText: string) => {
      setEditingTextId(null);
      onPatchObject?.(id, { text: newText });
    },
    [onPatchObject],
  );

  return (
    <div
      className="editor-overlay"
      style={{ position: "absolute", top: 0, left: 0, width: containerWidth, height: containerHeight, pointerEvents: "auto", zIndex: 15 }}
      onClick={handleBackgroundClick}
    >
      {objects.map((obj) => {
        const screenRect = pdfRectToScreen(obj.rect, page, view);
        const isSelected = selectedIds.includes(obj.id);
        const isEditing = editingTextId === obj.id;

        return (
          <div key={obj.id} style={{ position: "absolute", left: screenRect.x, top: screenRect.y, width: screenRect.width, height: screenRect.height }}>
            <div
              className={`editor-object editor-object--${obj.type}`}
              style={{ width: "100%", height: "100%", ...getObjectStyle(obj, view.zoom), cursor: activeTool === "select" ? "pointer" : "crosshair" }}
              onClick={(e) => handleObjectClick(e, obj.id)}
              onDoubleClick={(e) => handleObjectDoubleClick(e, obj)}
            >
              {obj.type === "textBox" && !isEditing && (
                <span className="editor-object__text" style={{ fontSize: obj.fontSize * view.zoom, color: obj.color }}>{obj.text}</span>
              )}
              {obj.type === "textBox" && isEditing && (
                <InlineTextEditor
                  initialText={obj.text}
                  fontSize={obj.fontSize * view.zoom}
                  color={obj.color}
                  onCommit={(text) => handleTextCommit(obj.id, text)}
                  onCancel={() => setEditingTextId(null)}
                />
              )}
              {obj.type === "comment" && (
                <div className="editor-object__comment-marker" title={obj.contents}>💬</div>
              )}
              {obj.type === "highlight" && null /* transparent overlay, no content */}
              {obj.type === "redaction" && (
                <span className="editor-object__redaction-label" style={{ fontSize: 10 * view.zoom }}>
                  {obj.status === "applied" ? "REDACTED" : "DRAFT"}
                </span>
              )}
              {obj.type === "stamp" && (
                <span className="editor-object__stamp-text" style={{ fontSize: 14 * view.zoom }}>{obj.stampText}</span>
              )}
              {/* Phase 25A + 26B + 27C: native redline markup. The container
                  DIV is transparent and we draw one or more explicit line
                  elements inside it to mimic how PDF Strikeout/Underline
                  annotations render in Acrobat. When metadata.line_bboxes
                  is provided (Phase 27C — sourced from real text line
                  bboxes), we draw one line per provided bbox in PDF-rect-
                  local space. Otherwise we fall back to the 14pt-row
                  heuristic. */}
              {(obj.type === "strikethrough" || obj.type === "underline") && (
                <RedlineLines
                  variant={obj.type}
                  color={obj.color}
                  rectHeightPts={obj.rect.height}
                  zoom={view.zoom}
                  contents={obj.contents}
                  lineBboxes={extractLineBboxes(obj.metadata, obj.rect)}
                />
              )}
            </div>

            {isSelected && (
              <ObjectSelection
                rect={{ x: 0, y: 0, width: screenRect.width, height: screenRect.height }}
                locked={obj.locked}
                onMove={(newLocalRect) => {
                  const newScreenRect: EditorRect = { x: screenRect.x + newLocalRect.x, y: screenRect.y + newLocalRect.y, width: newLocalRect.width, height: newLocalRect.height };
                  onMoveObject(obj.id, screenRectToPdf(newScreenRect, page, view));
                }}
                onResize={(newLocalRect) => {
                  const newScreenRect: EditorRect = { x: screenRect.x + newLocalRect.x, y: screenRect.y + newLocalRect.y, width: newLocalRect.width, height: newLocalRect.height };
                  onResizeObject(obj.id, screenRectToPdf(newScreenRect, page, view));
                }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
};

// --- Inline Text Editor ---

function InlineTextEditor({ initialText, fontSize, color, onCommit, onCancel }: {
  initialText: string;
  fontSize: number;
  color: string;
  onCommit: (text: string) => void;
  onCancel: () => void;
}) {
  const [text, setText] = useState(initialText);

  return (
    <textarea
      className="editor-inline-text"
      value={text}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => onCommit(text)}
      onKeyDown={(e) => {
        if (e.key === "Escape") { e.preventDefault(); onCancel(); }
        if (e.ctrlKey && e.key === "Enter") { e.preventDefault(); onCommit(text); }
      }}
      autoFocus
      style={{ width: "100%", height: "100%", fontSize, color, resize: "none", border: "none", outline: "2px solid #74a2ff", background: "rgba(255,255,255,0.95)", padding: 2, fontFamily: "inherit" }}
      onClick={(e) => e.stopPropagation()}
    />
  );
}

// --- Object Styles ---

function getObjectStyle(obj: EditorObject, zoom: number): React.CSSProperties {
  switch (obj.type) {
    case "textBox":
      return { background: obj.backgroundColor || "rgba(255,255,255,0.9)", border: `1px solid ${obj.borderColor || "rgba(116,162,255,0.5)"}`, borderRadius: 2, padding: 2, overflow: "hidden" };
    case "comment":
      return { display: "flex", alignItems: "center", justifyContent: "center", fontSize: 20 * zoom };
    case "highlight":
      return { background: obj.color || "rgba(255,255,0,0.35)", opacity: obj.opacity ?? 0.4, borderRadius: 2 };
    case "rectangle":
      return { background: obj.fillColor || "transparent", border: `${obj.strokeWidth}px solid ${obj.strokeColor || "#333"}`, borderRadius: 2 };
    case "redaction":
      return { background: obj.status === "applied" ? "#000" : "rgba(200,0,0,0.3)", borderRadius: 2, display: "flex", alignItems: "center", justifyContent: "center", color: "#fff", border: obj.status === "draft" ? "2px dashed #c00" : "none" };
    case "stamp":
      return { display: "flex", alignItems: "center", justifyContent: "center", border: `2px solid ${obj.color || "#c00"}`, borderRadius: 4, color: obj.color || "#c00", fontWeight: 700, letterSpacing: 1, textTransform: "uppercase" };
    case "strikethrough":
    case "underline":
      // Phase 26B: container is transparent; <RedlineLines> draws explicit
      // line elements so multi-line bboxes get one line per text row.
      return { background: "transparent", pointerEvents: "auto" };
    default:
      return {};
  }
}

// ---- Phase 27C — extract line bboxes from metadata.line_bboxes ----

/**
 * Read `metadata.line_bboxes` (an array of [x0, y0, x1, y1] PDF-point
 * tuples) and convert each entry to a `[topPct, heightPct]` band inside
 * the overlay container. Returns `null` if the metadata is absent or
 * malformed so the caller can fall back to the 14pt heuristic.
 *
 * The container's local coordinate space spans `obj.rect` in PDF point
 * units (origin top-left of the rect; y grows downward inside the
 * container even though PDF y grows upward — both axes are linear, so
 * we anchor on the rect's PDF-coord top-left).
 */
function extractLineBboxes(
  metadata: Record<string, unknown> | undefined,
  rect: { x: number; y: number; width: number; height: number },
): { topPct: number; heightPct: number }[] | null {
  if (!metadata) return null;
  const raw = metadata.line_bboxes;
  if (!Array.isArray(raw) || raw.length === 0) return null;
  const out: { topPct: number; heightPct: number }[] = [];
  // rect is in PDF coords: x grows right, y grows up.
  // Inside the overlay container, top is rect's max y (rect.y + rect.height)
  // and bottom is rect.y, because EditableOverlay renders with Y-flip.
  const rectTopPdf = rect.y + rect.height;
  for (const entry of raw) {
    if (!Array.isArray(entry) || entry.length !== 4) continue;
    const [, y0, , y1] = entry as [number, number, number, number];
    if (!Number.isFinite(y0) || !Number.isFinite(y1)) continue;
    const top = Math.min(y0, y1);
    const bot = Math.max(y0, y1);
    const lineTopPdf = bot; // PDF: higher y = visually higher
    const lineBotPdf = top;
    // Map PDF y → container y (0 at top, rect.height at bottom).
    const cTop = rectTopPdf - lineTopPdf;
    const cBot = rectTopPdf - lineBotPdf;
    const h = Math.max(0, cBot - cTop);
    if (h <= 0 || rect.height <= 0) continue;
    out.push({
      topPct: (cTop / rect.height) * 100,
      heightPct: (h / rect.height) * 100,
    });
  }
  return out.length > 0 ? out : null;
}

// ---- Phase 26B + 27C: explicit redline line renderer ----

/**
 * Render strikeout/underline as one or more horizontal lines, one per
 * detected text row in the rect. Row height defaults to 14pt (≈ standard
 * body text leading); if the rect is taller than one row we draw evenly
 * spaced lines covering each row band, matching how Acrobat displays
 * Strikeout / Underline annotations that span wrapped text.
 */
function RedlineLines({
  variant,
  color,
  rectHeightPts,
  zoom,
  contents,
  lineBboxes,
}: {
  variant: "strikethrough" | "underline";
  color: string;
  rectHeightPts: number;
  zoom: number;
  contents: string;
  /** Phase 27C — pre-computed line bands (topPct, heightPct) in container space. */
  lineBboxes?: { topPct: number; heightPct: number }[] | null;
}) {
  const rowPts = 14;
  const heuristicRows = Math.max(1, Math.round(rectHeightPts / rowPts));
  const useBboxes = Array.isArray(lineBboxes) && lineBboxes.length > 0;
  const rowCount = useBboxes ? lineBboxes!.length : heuristicRows;
  const lineThicknessPx = Math.max(1.5, 0.12 * rowPts * zoom);

  return (
    <div
      className={`editor-object__redline editor-object__redline--${variant}`}
      data-testid={`redline-${variant}`}
      data-rows={rowCount}
      data-source={useBboxes ? "line-bboxes" : "heuristic"}
      title={contents}
      aria-label={contents}
      style={{
        width: "100%",
        height: "100%",
        position: "absolute",
        inset: 0,
        pointerEvents: "none",
      }}
    >
      {useBboxes
        ? lineBboxes!.map((band, i) => {
            // Strikeout: middle of the line band.
            // Underline: 88% down the band (near baseline).
            const offsetPct = variant === "underline" ? 88 : 50;
            const topPct = band.topPct + (band.heightPct * offsetPct) / 100;
            return (
              <div
                key={i}
                className={`editor-object__redline-line editor-object__redline-line--${variant}`}
                data-testid={`redline-line-${i}`}
                style={{
                  position: "absolute",
                  left: 0,
                  right: 0,
                  top: `${topPct}%`,
                  height: lineThicknessPx,
                  background: color,
                  transform: "translateY(-50%)",
                }}
              />
            );
          })
        : Array.from({ length: heuristicRows }, (_, i) => {
            const rowHeightPct = 100 / heuristicRows;
            const offsetPct = variant === "underline" ? 88 : 50;
            const topPct = rowHeightPct * i + (rowHeightPct * offsetPct) / 100;
            return (
              <div
                key={i}
                className={`editor-object__redline-line editor-object__redline-line--${variant}`}
                data-testid={`redline-line-${i}`}
                style={{
                  position: "absolute",
                  left: 0,
                  right: 0,
                  top: `${topPct}%`,
                  height: lineThicknessPx,
                  background: color,
                  transform: "translateY(-50%)",
                }}
              />
            );
          })}
    </div>
  );
}
