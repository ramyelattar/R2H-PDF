import type { LocalModelInfo } from "./types";

interface LocalModelListProps {
  models: LocalModelInfo[];
  onValidate: (modelId: string) => Promise<LocalModelInfo | null>;
}

export const LocalModelList = ({ models }: LocalModelListProps) => {
  if (models.length === 0) {
    return <p className="local-ai-empty">No models registered. Check local-ai/config/models.json.</p>;
  }

  return (
    <div className="local-ai-models">
      <h5>Registered Models</h5>
      <ul className="local-ai-model-list">
        {models.map((model) => (
          <li key={model.id} className={`local-ai-model-item ${model.exists ? "" : "local-ai-model-item--missing"}`}>
            <div className="local-ai-model-name">
              {model.name}
              {model.default && <span className="local-ai-badge">default</span>}
            </div>
            <div className="local-ai-model-meta">
              <span>{model.format.toUpperCase()}</span>
              <span>{model.exists ? formatBytes(model.size_bytes) : "NOT FOUND"}</span>
              <span>ctx: {model.context_window.toLocaleString()}</span>
            </div>
            {!model.exists && (
              <div className="local-ai-model-error">Model file not found: {model.path}</div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}
