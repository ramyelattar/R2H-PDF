import type { EditorObject, EditorObjectId, EditorObjectPatch } from "../pdf-editor/types";
import { useObjectsPanel } from "./useObjectsPanel";
import type { ObjectRow, ZOrderOp } from "./types";

interface ObjectsPanelProps {
  sessionId: string;
  overlayObjects: EditorObject[];
  activePageIndex: number;
  totalPages: number;
  onNavigateToPage?: (pageIndex: number) => void;
  /** Overlay-object mutation hooks. */
  onSelectOverlay?: (id: EditorObjectId) => void;
  onPatchOverlay?: (id: EditorObjectId, patch: EditorObjectPatch) => void;
  onDeleteOverlay?: (id: EditorObjectId) => void;
}

const SOURCE_LABEL: Record<string, string> = {
  manual: "Manual",
  ai: "AI",
  compare: "Compare",
  compare_visual: "Compare (visual)",
  ai_review: "AI Review",
  form: "Form",
  content_edit: "Content edit",
  signature: "Signature",
  ocr: "OCR",
  native_content: "Native PDF",
};

export const ObjectsPanel = ({
  sessionId,
  overlayObjects,
  activePageIndex,
  totalPages,
  onNavigateToPage,
  onSelectOverlay,
  onPatchOverlay,
  onDeleteOverlay,
}: ObjectsPanelProps) => {
  const panel = useObjectsPanel({ sessionId, overlayObjects, activePageIndex });
  const { filter, setFilter, filteredRows, actionsFor, loading, error, refresh, computeReorderZIndex } = panel;

  const handleJump = (row: ObjectRow) => {
    onNavigateToPage?.(row.pageIndex);
  };
  const handleSelect = (row: ObjectRow) => {
    if (row.id.startsWith("overlay:")) onSelectOverlay?.(row.sourceId);
  };
  const handleToggleHidden = (row: ObjectRow) => {
    if (!row.id.startsWith("overlay:")) return;
    onPatchOverlay?.(row.sourceId, { hidden: !(row.hidden ?? false) });
  };
  const handleToggleLocked = (row: ObjectRow) => {
    if (!row.id.startsWith("overlay:")) return;
    onPatchOverlay?.(row.sourceId, { locked: !(row.locked ?? false) });
  };
  const handleDelete = (row: ObjectRow) => {
    if (!row.id.startsWith("overlay:")) return;
    onDeleteOverlay?.(row.sourceId);
  };
  // Phase 27B — z-order reorder dispatch.
  const handleReorder = (row: ObjectRow, op: ZOrderOp) => {
    if (!row.id.startsWith("overlay:")) return;
    const newZ = computeReorderZIndex(row.sourceId, op);
    if (newZ == null) return;
    onPatchOverlay?.(row.sourceId, { zIndex: newZ });
  };

  return (
    <div className="objects-panel" data-testid="objects-panel">
      <div className="section-header">
        <h4 className="section-header__title">Objects &amp; Layers</h4>
        <span className="section-header__hint">{filteredRows.length} shown</span>
      </div>

      {/* filter / search bar */}
      <div
        className="objects-filter"
        style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 6, fontSize: 12 }}
      >
        <input
          type="text"
          aria-label="Search objects"
          value={filter.search}
          onChange={(e) => setFilter({ ...filter, search: e.target.value })}
          className="compact-input"
          style={{ flex: 1, minWidth: 100 }}
          data-testid="objects-search"
        />
        <select
          value={filter.source}
          onChange={(e) => setFilter({ ...filter, source: e.target.value as typeof filter.source })}
          data-testid="objects-source-filter"
        >
          <option value="all">All sources</option>
          <option value="manual">Manual</option>
          <option value="compare">Compare</option>
          <option value="compare_visual">Compare visual</option>
          <option value="ai_review">AI Review</option>
          <option value="ai">AI</option>
          <option value="signature">Signature</option>
          <option value="ocr">OCR</option>
          <option value="form">Form</option>
          <option value="content_edit">Content edit</option>
          <option value="native_content">Native PDF</option>
        </select>
        <select
          value={filter.kind}
          onChange={(e) => setFilter({ ...filter, kind: e.target.value as typeof filter.kind })}
          data-testid="objects-kind-filter"
        >
          <option value="all">All kinds</option>
          <option value="textBox">Text box</option>
          <option value="comment">Comment</option>
          <option value="highlight">Highlight</option>
          <option value="rectangle">Rectangle</option>
          <option value="redaction">Redaction</option>
          <option value="stamp">Stamp</option>
          <option value="image">Image</option>
          <option value="strikethrough">Strikethrough</option>
          <option value="underline">Underline</option>
          <option value="form_field">Form field</option>
          <option value="content_edit">Content edit</option>
          <option value="native_text">Native text</option>
          <option value="native_image">Native image</option>
          <option value="native_path">Native path</option>
        </select>
        <select
          value={filter.page === "all" ? "all" : String(filter.page)}
          onChange={(e) => setFilter({
            ...filter,
            page: e.target.value === "all" ? "all" : Number(e.target.value),
          })}
          data-testid="objects-page-filter"
        >
          <option value="all">All pages</option>
          {Array.from({ length: totalPages }, (_, i) => (
            <option key={i} value={i}>page {i + 1}</option>
          ))}
        </select>
        <button className="ghost-btn" onClick={() => void refresh()} disabled={loading}>
          Refresh
        </button>
      </div>

      {loading && <div className="loading-bar" />}
      {error && <p className="text-error">{error}</p>}

      {filteredRows.length === 0 && !loading && (
        <p className="empty-text">No objects match the current filter.</p>
      )}

      <ul className="objects-list" style={{ listStyle: "none", padding: 0, margin: 0, maxHeight: 480, overflowY: "auto" }}>
        {filteredRows.map((row) => {
          const a = actionsFor(row);
          return (
            <li
              key={row.id}
              data-testid="objects-row"
              data-source={row.source}
              data-kind={row.kind}
              style={{
                borderBottom: "1px solid var(--border, #333)",
                padding: 6,
                display: "grid",
                gridTemplateColumns: "1fr auto",
                gap: 4,
                opacity: row.hidden ? 0.55 : 1,
              }}
            >
              <div style={{ overflow: "hidden" }}>
                <div style={{ display: "flex", gap: 4, alignItems: "center", flexWrap: "wrap" }}>
                  <span className="badge" style={{ fontSize: 10 }}>
                    {objectIcon(row.kind)} {SOURCE_LABEL[row.source] || row.source}
                  </span>
                  <span className="badge" style={{ fontSize: 10 }} data-testid="objects-type-badge">{formatObjectKind(row.kind)}</span>
                  <span style={{ fontSize: 11, color: "var(--muted, #888)" }}>p.{row.pageIndex + 1}</span>
                  {typeof row.zIndex === "number" && (
                    <span
                      className="badge"
                      style={{ fontSize: 10 }}
                      title="z-order index (higher = on top)"
                      data-testid="objects-zindex-badge"
                    >z {row.zIndex}</span>
                  )}
                  <span className="badge" style={{ fontSize: 10 }}>{row.hidden ? "Hidden" : "Visible"}</span>
                  <span className="badge" style={{ fontSize: 10 }}>{row.locked ? "Locked" : "Unlocked"}</span>
                </div>
                <div style={{ fontSize: 12, marginTop: 2, wordBreak: "break-word" }}>{row.label}</div>
                {row.status && (
                  <div style={{ fontSize: 11, color: "var(--muted, #888)" }}>{row.status}</div>
                )}
                {row.warnings && row.warnings.length > 0 && (
                  <ul style={{ fontSize: 11, color: "var(--warn, #c80)", margin: "2px 0 0 12px", padding: 0 }}>
                    {row.warnings.slice(0, 3).map((w, i) => <li key={i}>{w}</li>)}
                  </ul>
                )}
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
                <button
                  className="ghost-btn"
                  onClick={() => handleJump(row)}
                  disabled={!a.canJump}
                  title={a.canJump ? `Go to page ${row.pageIndex + 1}` : a.disabledReason}
                  style={{ fontSize: 11, padding: "1px 6px" }}
                  data-testid="objects-jump"
                >Go</button>
                {a.canSelect && (
                  <button
                    className="ghost-btn"
                    onClick={() => handleSelect(row)}
                    style={{ fontSize: 11, padding: "1px 6px" }}
                    data-testid="objects-select"
                  >Select</button>
                )}
                {a.canHide && (
                  <button
                    className="ghost-btn"
                    onClick={() => handleToggleHidden(row)}
                    style={{ fontSize: 11, padding: "1px 6px" }}
                    title={row.hidden ? "Show" : "Hide"}
                    data-testid="objects-toggle-hidden"
                  >{row.hidden ? "Show" : "Hide"}</button>
                )}
                {a.canLock && (
                  <button
                    className="ghost-btn"
                    onClick={() => handleToggleLocked(row)}
                    style={{ fontSize: 11, padding: "1px 6px" }}
                    title={row.locked ? "Unlock" : "Lock"}
                    data-testid="objects-toggle-locked"
                  >{row.locked ? "Unlock" : "Lock"}</button>
                )}
                {/* Phase 27B — z-order controls. Show for overlay rows; disabled
                    with an honest reason for non-overlay/locked rows. */}
                <div
                  style={{ display: "flex", gap: 2, flexWrap: "wrap" }}
                  data-testid="objects-zorder-group"
                >
                  <button
                    className="ghost-btn"
                    onClick={() => handleReorder(row, "front")}
                    disabled={!a.canReorder}
                    title={a.canReorder ? "Bring to front" : a.reorderDisabledReason}
                    style={{ fontSize: 11, padding: "1px 4px" }}
                    data-testid="objects-zorder-front"
                  >⤒</button>
                  <button
                    className="ghost-btn"
                    onClick={() => handleReorder(row, "forward")}
                    disabled={!a.canReorder}
                    title={a.canReorder ? "Bring forward" : a.reorderDisabledReason}
                    style={{ fontSize: 11, padding: "1px 4px" }}
                    data-testid="objects-zorder-forward"
                  >↑</button>
                  <button
                    className="ghost-btn"
                    onClick={() => handleReorder(row, "backward")}
                    disabled={!a.canReorder}
                    title={a.canReorder ? "Send backward" : a.reorderDisabledReason}
                    style={{ fontSize: 11, padding: "1px 4px" }}
                    data-testid="objects-zorder-backward"
                  >↓</button>
                  <button
                    className="ghost-btn"
                    onClick={() => handleReorder(row, "back")}
                    disabled={!a.canReorder}
                    title={a.canReorder ? "Send to back" : a.reorderDisabledReason}
                    style={{ fontSize: 11, padding: "1px 4px" }}
                    data-testid="objects-zorder-back"
                  >⤓</button>
                </div>
                <button
                  className="ghost-btn"
                  onClick={() => handleDelete(row)}
                  disabled={!a.canDelete}
                  title={a.canDelete ? "Delete overlay object" : a.disabledReason}
                  style={{ fontSize: 11, padding: "1px 6px", color: a.canDelete ? "var(--danger, #c33)" : undefined }}
                  data-testid="objects-delete"
                  >Delete overlay</button>
              </div>
            </li>
          );
        })}
      </ul>

      <p className="text-muted" style={{ fontSize: 11, marginTop: 6 }}>
        Native PDF objects are read-only here. Use the Edit Content panel to modify them safely.
      </p>
    </div>
  );
};

function formatObjectKind(kind: string): string {
  switch (kind) {
    case "textBox": return "Text box";
    case "form_field": return "Form field";
    case "content_edit": return "Content edit";
    case "native_text": return "Native text";
    case "native_image": return "Native image";
    case "native_path": return "Native path";
    default: return kind.replace(/_/g, " ");
  }
}

function objectIcon(kind: string): string {
  if (kind.includes("text")) return "T";
  if (kind.includes("image")) return "IMG";
  if (kind.includes("native")) return "PDF";
  if (kind.includes("form")) return "FORM";
  if (kind === "stamp") return "STAMP";
  return "OBJ";
}
