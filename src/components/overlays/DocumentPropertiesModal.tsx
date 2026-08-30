import type { DocumentTab } from "../../types/shell";

interface DocumentPropertiesModalProps {
  tab: DocumentTab;
  onClose: () => void;
}

export const DocumentPropertiesModal = ({ tab, onClose }: DocumentPropertiesModalProps) => {
  const handleBackdropClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) onClose();
  };

  return (
    <div className="modal-backdrop" onClick={handleBackdropClick}>
      <div className="modal-dialog" role="dialog" aria-label="Document Properties">
        <div className="modal-header">
          <h2 className="modal-title">Document Properties</h2>
          <button className="ghost-btn" onClick={onClose} title="Close">✕</button>
        </div>

        <div className="modal-body">
          <section className="doc-props-section">
            <h3 className="doc-props-section-title">General</h3>
            <dl className="doc-props-list">
              <dt>File Name</dt>
              <dd>{tab.sourcePath.split(/[/\\]/).pop() ?? "—"}</dd>
              <dt>Full Path</dt>
              <dd className="doc-props-path">{tab.sourcePath}</dd>
              <dt>Title</dt>
              <dd>{tab.title || "—"}</dd>
              <dt>Pages</dt>
              <dd>{tab.totalPages || "—"}</dd>
              <dt>Current Page</dt>
              <dd>{tab.page}</dd>
              <dt>Zoom</dt>
              <dd>{tab.zoom}%</dd>
              {tab.rotation !== undefined && (
                <>
                  <dt>Rotation</dt>
                  <dd>{tab.rotation}°</dd>
                </>
              )}
            </dl>
          </section>

          {tab.permissions && (
            <section className="doc-props-section">
              <h3 className="doc-props-section-title">Security &amp; Permissions</h3>
              <dl className="doc-props-list">
                <dt>Print</dt>
                <dd className={tab.permissions.canPrint ? "text-ok" : "text-error"}>
                  {tab.permissions.canPrint ? "Allowed" : "Restricted"}
                </dd>
                <dt>Copy Text</dt>
                <dd className={tab.permissions.canCopy ? "text-ok" : "text-error"}>
                  {tab.permissions.canCopy ? "Allowed" : "Restricted"}
                </dd>
                <dt>Edit Content</dt>
                <dd className={tab.permissions.canEdit ? "text-ok" : "text-error"}>
                  {tab.permissions.canEdit ? "Allowed" : "Restricted"}
                </dd>
                <dt>Annotate</dt>
                <dd className={tab.permissions.canAnnotate ? "text-ok" : "text-error"}>
                  {tab.permissions.canAnnotate ? "Allowed" : "Restricted"}
                </dd>
              </dl>
            </section>
          )}

          <section className="doc-props-section">
            <h3 className="doc-props-section-title">Session</h3>
            <dl className="doc-props-list">
              <dt>Session ID</dt>
              <dd className="doc-props-mono">{tab.id}</dd>
              <dt>Workspace</dt>
              <dd>{tab.workspaceId}</dd>
              <dt>Tab Kind</dt>
              <dd>{tab.kind.toUpperCase()}</dd>
              <dt>Dirty</dt>
              <dd className={tab.dirty ? "text-warn" : "text-ok"}>
                {tab.dirty ? "Yes — unsaved changes" : "No"}
              </dd>
              {tab.isScanned !== undefined && (
                <>
                  <dt>Scanned PDF</dt>
                  <dd>{tab.isScanned ? "Yes (OCR required)" : "No"}</dd>
                </>
              )}
              {tab.lastSavedAt && (
                <>
                  <dt>Last Saved</dt>
                  <dd>{new Date(tab.lastSavedAt).toLocaleString()}</dd>
                </>
              )}
            </dl>
          </section>
        </div>

        <div className="modal-footer">
          <button className="ghost-btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </div>
  );
};
