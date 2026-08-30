import type { DiagnosticLog } from "../../types/shell";

interface DiagnosticsPanelProps {
  open: boolean;
  logs: DiagnosticLog[];
  onClose: () => void;
}

export const DiagnosticsPanel = ({
  open,
  logs,
  onClose,
}: DiagnosticsPanelProps) => {
  if (!open) {
    return null;
  }

  return (
    <section className="diagnostics-panel">
      <header>
        <h3>Diagnostics Viewer</h3>
        <button className="ghost-btn" onClick={onClose}>
          Hide
        </button>
      </header>

      <div className="diagnostics-table">
        {logs.map((entry) => (
          <article key={entry.id} className={`diagnostics-row level-${entry.level}`}>
            <span>{entry.at}</span>
            <strong>{entry.level}</strong>
            <span>{entry.source}</span>
            <p>{entry.message}</p>
          </article>
        ))}
      </div>
    </section>
  );
};
