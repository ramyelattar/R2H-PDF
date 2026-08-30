import { useState } from "react";
import { useAiActions } from "./useAiActions";
import { AiActionList } from "./AiActionList";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { AiProposedAction } from "./types";

interface AiActionPanelProps {
  sessionId: string;
  pageCount: number;
  appendDiagnostic: AppendDiagnostic;
  onApplyActions: (actions: AiProposedAction[]) => void;
}

export const AiActionPanel = ({ sessionId, pageCount, appendDiagnostic, onApplyActions }: AiActionPanelProps) => {
  const ai = useAiActions({ sessionId, pageCount, appendDiagnostic });
  const [prompt, setPrompt] = useState("");

  const handlePlan = async () => {
    if (!prompt.trim()) return;
    await ai.planActions(prompt);
  };

  const handleApply = async () => {
    const applied = await ai.applyAccepted();
    if (applied.length > 0) {
      onApplyActions(applied);
      appendDiagnostic({ level: "INFO", source: "ui", message: `Applied ${applied.length} AI actions to editor overlay` });
    }
  };

  const acceptedCount = ai.batch?.actions.filter((a) => a.status === "accepted").length ?? 0;

  return (
    <div className="ai-action-panel" data-testid="ai-action-panel">
      <div className="section-header">
        <h4 className="section-header__title">Suggested actions</h4>
        <span className="section-header__hint">
          {ai.batch ? `${ai.batch.actions.length} proposed` : "Idle"}
        </span>
      </div>

      <div className="inspector-card">
        <p className="inspector-card__title">Prompt</p>
        <textarea
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
          placeholder="What should the AI do? e.g. 'Highlight risk clauses and add comments'"
          rows={2}
          aria-label="AI action prompt"
          data-testid="ai-action-prompt"
        />
        <div className="action-row" style={{ marginTop: 6 }}>
          <button
            className="btn btn--primary btn--sm"
            onClick={() => void handlePlan()}
            disabled={ai.planning || !prompt.trim()}
            aria-disabled={ai.planning || !prompt.trim()}
            data-testid="ai-action-plan-btn"
          >
            {ai.planning ? "Planning…" : "Plan actions"}
          </button>
        </div>
      </div>

      {ai.error && (
        <div className="callout callout--danger">
          <span className="callout__icon" aria-hidden="true">✕</span>
          <div className="callout__body">{ai.error}</div>
        </div>
      )}

      {ai.batch && (
        <>
          {ai.batch.warnings.length > 0 && (
            <div className="callout callout--warn">
              <span className="callout__icon" aria-hidden="true">⚠</span>
              <div className="callout__body">
                <ul style={{ margin: 0, paddingLeft: 14 }}>
                  {ai.batch.warnings.map((w, i) => (<li key={i}>{w}</li>))}
                </ul>
              </div>
            </div>
          )}

          <AiActionList
            actions={ai.batch.actions}
            onAccept={ai.acceptAction}
            onReject={ai.rejectAction}
          />

          <div className="action-row">
            <button
              className="btn btn--primary btn--sm"
              onClick={() => void handleApply()}
              disabled={acceptedCount === 0}
              aria-disabled={acceptedCount === 0}
              data-testid="ai-action-apply-btn"
            >
              Apply accepted ({acceptedCount})
            </button>
            <button
              className="btn btn--ghost btn--sm"
              onClick={() => void ai.clearBatch()}
              data-testid="ai-action-clear-btn"
            >
              Clear
            </button>
          </div>
        </>
      )}
    </div>
  );
};
