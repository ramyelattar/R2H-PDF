import type { WorkstationState } from "../../state/useWorkstationState";
import type { LicenseStatus } from "../../lib/ipc";

interface StatusBarProps {
  state: WorkstationState;
  activeTab: WorkstationState["tabs"][number] | null;
  lastRenderMs: number | null;
  licenseStatus: LicenseStatus | null;
  onToggleDiagnostics: () => void;
}

export const StatusBar = ({
  state,
  activeTab,
  lastRenderMs,
  licenseStatus,
  onToggleDiagnostics,
}: StatusBarProps) => {
  const perms = activeTab?.permissions;
  const modeLabel = (() => {
    switch (state.currentMode) {
      case "read": return "Read";
      case "annotate": return "Annotate";
      case "edit-content": return "Edit";
      case "review": return "Review";
      default: return state.currentMode;
    }
  })();

  return (
    <footer className="status-bar" role="contentinfo">
      <div className="status-bar__left">
        <span
          className="status-pill"
          title={`Current mode: ${modeLabel}`}
          data-testid="status-mode-pill"
        >
          {modeLabel}
        </span>
        <span className="status-ws" title="Active workspace">
          {state.workspaces.find((ws) => ws.id === state.activeWorkspaceId)?.name ?? "No Workspace"}
        </span>
        {activeTab?.dirty && (
          <span className="status-dirty-badge" title="Document has unsaved changes">● Unsaved</span>
        )}
      </div>

      <div className="status-bar__center">
        {activeTab ? (
          <>
            <span className="status-page">
              Page <strong>{activeTab.page}</strong> of {activeTab.totalPages || "–"}
            </span>
            <span className="status-sep" aria-hidden="true">·</span>
            <span className="status-zoom">{activeTab.zoom}%</span>
            {lastRenderMs !== null && (
              <>
                <span className="status-sep" aria-hidden="true">·</span>
                <span className="status-render-time" title="Last page render time">
                  rendered in {lastRenderMs} ms
                </span>
              </>
            )}
          </>
        ) : (
          <span className="status-ready">Ready</span>
        )}
      </div>

      <div className="status-bar__right">
        <span
          className={`status-pill ${licenseStatus?.allowed === false ? "status-pill--warn" : ""}`}
          title={licenseStatus ? licenseStatus.reason : "License status is loading"}
          data-testid="license-status-pill"
        >
          License: {licenseStatus?.mode ?? "checking"}
        </span>
        {activeTab && perms && (
          <div
            className="status-perms"
            role="group"
            aria-label="Document permissions"
            title="Document permissions"
          >
            <span className={`status-perm-icon ${perms.canPrint ? "status-perm-icon--on" : "status-perm-icon--off"}`} title={perms.canPrint ? "Print allowed" : "Print blocked"} aria-label={perms.canPrint ? "Print allowed" : "Print blocked"}>🖨</span>
            <span className={`status-perm-icon ${perms.canCopy ? "status-perm-icon--on" : "status-perm-icon--off"}`} title={perms.canCopy ? "Copy allowed" : "Copy blocked"} aria-label={perms.canCopy ? "Copy allowed" : "Copy blocked"}>📋</span>
            <span className={`status-perm-icon ${perms.canEdit ? "status-perm-icon--on" : "status-perm-icon--off"}`} title={perms.canEdit ? "Edit allowed" : "Edit blocked"} aria-label={perms.canEdit ? "Edit allowed" : "Edit blocked"}>✏</span>
            <span className={`status-perm-icon ${perms.canAnnotate ? "status-perm-icon--on" : "status-perm-icon--off"}`} title={perms.canAnnotate ? "Annotate allowed" : "Annotate blocked"} aria-label={perms.canAnnotate ? "Annotate allowed" : "Annotate blocked"}>💬</span>
          </div>
        )}
        <button
          className={`btn btn--ghost btn--sm status-log-btn${state.diagnosticsOpen ? " status-log-btn--active" : ""}`}
          onClick={onToggleDiagnostics}
          aria-label={`Diagnostics (${state.diagnostics.length} entries)`}
          aria-pressed={state.diagnosticsOpen}
        >
          Diagnostics ({state.diagnostics.length})
        </button>
      </div>
    </footer>
  );
};
