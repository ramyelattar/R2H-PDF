import type { WorkstationState } from "../../state/useWorkstationState";
import type { DocumentTab } from "../../types/shell";

interface LeftPanelProps {
  state: WorkstationState;
  onOpenTab: (tab: DocumentTab) => void;
  onOpenFilePath: (path: string) => void;
  onSetWorkspace: (workspaceId: string) => void;
  onClose: () => void;
}

const formatRelativeTime = (epochMs: number) => {
  const elapsedMs = Math.max(0, Date.now() - epochMs);
  const minutes = Math.floor(elapsedMs / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
};

export const LeftPanel = ({ state, onOpenTab, onOpenFilePath, onSetWorkspace, onClose }: LeftPanelProps) => {
  const pinnedTabs = state.tabs.filter((tab) => tab.pinned);

  return (
    <aside className="left-panel">
      <section className="panel-section panel-section--head">
        <h3>Document Browser</h3>
        <button className="ghost-btn" onClick={onClose}>
          Close
        </button>
      </section>

      <section className="panel-section">
        <h3>Recent Files</h3>
        {state.recentFiles.map((file) => (
          <button
            key={file.id}
            className="record-item"
            disabled={!file.available}
            onClick={() => onOpenFilePath(file.path)}
          >
            <span>{file.title}</span>
            <small>
              {file.available ? `${formatRelativeTime(file.lastOpenedAtEpochMs)} · ${file.pages || "?"}p` : "Unavailable"}
            </small>
          </button>
        ))}
        {!state.recentFiles.length && <p className="empty-text">No PDFs have been opened yet.</p>}
      </section>

      <section className="panel-section">
        <h3>Pinned</h3>
        {pinnedTabs.length ? (
          pinnedTabs.map((tab) => (
            <button key={tab.id} className="record-item" onClick={() => onOpenTab(tab)}>
              <span>{tab.title}</span>
              <small>{tab.workspaceId}</small>
            </button>
          ))
        ) : (
          <p className="empty-text">No pinned documents yet.</p>
        )}
      </section>

      <section className="panel-section">
        <h3>Workspace</h3>
        {state.workspaces.map((workspace) => (
          <button
            key={workspace.id}
            className={`record-item ${
              workspace.id === state.activeWorkspaceId ? "is-active" : ""
            }`}
            onClick={() => onSetWorkspace(workspace.id)}
          >
            <span>{workspace.name}</span>
            <small>{workspace.documentCount} document{workspace.documentCount === 1 ? "" : "s"}</small>
          </button>
        ))}
        {!state.workspaces.length && <p className="empty-text">No persisted project workspaces yet. Save a PDF project to create one.</p>}
      </section>
    </aside>
  );
};
