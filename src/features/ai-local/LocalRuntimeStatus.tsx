import type { LocalRuntimeStatus } from "./types";

interface LocalRuntimeStatusViewProps {
  status: LocalRuntimeStatus | null;
}

export const LocalRuntimeStatusView = ({ status }: LocalRuntimeStatusViewProps) => {
  if (!status) {
    return <div className="local-ai-status">Loading runtime status…</div>;
  }

  return (
    <div className="local-ai-status">
      <div className="local-ai-status__row">
        <span>Runtime:</span>
        <strong className={status.available ? "text-ok" : "text-error"}>
          {status.available ? "Available" : "Not Found"}
        </strong>
      </div>
      {status.available && (
        <div className="local-ai-status__row">
          <span>Version:</span>
          <strong>{status.runtime_version}</strong>
        </div>
      )}
      {status.default_model_id && (
        <div className="local-ai-status__row">
          <span>Default model:</span>
          <strong>{status.default_model_id}</strong>
        </div>
      )}
      {status.errors.length > 0 && (
        <div className="local-ai-errors">
          {status.errors.map((err, i) => (
            <div key={i} className="local-ai-error">{err}</div>
          ))}
        </div>
      )}
    </div>
  );
};
