import { PageThumbnailGrid } from "./PageThumbnailGrid";
import type { EditorObject } from "../pdf-editor/types";

interface PageOrganizerProps {
  sessionId: string;
  totalPages: number;
  selectedPages: number[];
  busy: boolean;
  editorObjects: Map<string, EditorObject>;
  onSelectPage: (pageIndex: number) => void;
  onRotateCW: () => void;
  onRotateCCW: () => void;
  onDeletePage: () => void;
  onInsertBlank: () => void;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onClose: () => void;
}

export const PageOrganizer = ({
  sessionId,
  totalPages,
  selectedPages,
  busy,
  editorObjects,
  onSelectPage,
  onRotateCW,
  onRotateCCW,
  onDeletePage,
  onInsertBlank,
  onMoveUp,
  onMoveDown,
  onClose,
}: PageOrganizerProps) => {
  const hasSelection = selectedPages.length > 0;
  const selectedIndex = selectedPages[0] ?? -1;
  const canMoveUp = hasSelection && selectedIndex > 0;
  const canMoveDown = hasSelection && selectedIndex < totalPages - 1;
  const canDelete = hasSelection && totalPages > 1;

  return (
    <div className="page-organizer">
      <div className="page-organizer__header">
        <h3>Organize Pages</h3>
        <span className="page-organizer__count">{totalPages} pages</span>
        <button className="ghost-btn" onClick={onClose} title="Close">✕</button>
      </div>

      <div className="page-organizer__toolbar">
        <button className="ghost-btn" onClick={onRotateCCW} disabled={!hasSelection || busy} title="Rotate Left">↺</button>
        <button className="ghost-btn" onClick={onRotateCW} disabled={!hasSelection || busy} title="Rotate Right">↻</button>
        <button className="ghost-btn" onClick={onDeletePage} disabled={!canDelete || busy} title="Delete Page">🗑</button>
        <button className="ghost-btn" onClick={onInsertBlank} disabled={busy} title="Insert Blank Page">＋</button>
        <button className="ghost-btn" onClick={onMoveUp} disabled={!canMoveUp || busy} title="Move Up">↑</button>
        <button className="ghost-btn" onClick={onMoveDown} disabled={!canMoveDown || busy} title="Move Down">↓</button>
      </div>

      <div className="page-organizer__grid">
        <PageThumbnailGrid
          sessionId={sessionId}
          totalPages={totalPages}
          selectedPages={selectedPages}
          editorObjects={editorObjects}
          onSelectPage={onSelectPage}
        />
      </div>

      {busy && <div className="page-organizer__busy">Processing…</div>}
    </div>
  );
};
