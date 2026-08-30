import type { AiProposedAction } from "./types";

interface AiActionListProps {
  actions: AiProposedAction[];
  onAccept: (actionId: string) => void;
  onReject: (actionId: string) => void;
}

export const AiActionList = ({ actions, onAccept, onReject }: AiActionListProps) => {
  if (actions.length === 0) {
    return <p className="ai-action-empty">No actions proposed.</p>;
  }

  return (
    <ul className="ai-action-list">
      {actions.map((action) => (
        <li key={action.action_id} className={`ai-action-card ai-action-card--${action.status}`}>
          <div className="ai-action-card__header">
            <span className="ai-action-card__type">{formatType(action.action_type)}</span>
            <span className="ai-action-card__page">Page {action.page_index + 1}</span>
            <span className="ai-action-card__confidence">{(action.confidence * 100).toFixed(0)}%</span>
            <span className={`ai-action-card__status ai-action-card__status--${action.status}`}>{action.status}</span>
          </div>
          {action.text && <p className="ai-action-card__text">{action.text}</p>}
          <p className="ai-action-card__reason">{action.reason}</p>
          {action.source_citations.length > 0 && (
            <div className="ai-action-card__citations">
              {action.source_citations.map((c) => (
                <span key={c.citation_id} className="ai-action-card__cite">p.{c.page_index + 1}</span>
              ))}
            </div>
          )}
          {action.status === "proposed" && (
            <div className="ai-action-card__buttons">
              <button className="ghost-btn" onClick={() => onAccept(action.action_id)}>Accept</button>
              <button className="ghost-btn" onClick={() => onReject(action.action_id)}>Reject</button>
            </div>
          )}
        </li>
      ))}
    </ul>
  );
};

function formatType(type: string): string {
  return type.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}
