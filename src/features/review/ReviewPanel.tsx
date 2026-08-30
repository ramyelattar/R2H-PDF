import { useState } from "react";
import { useDocumentReview } from "./useDocumentReview";
import type { ReviewFinding, ReviewSuggestedAction } from "./types";

interface ReviewPanelProps {
  sessionId: string;
  onNavigateToPage?: (pageIndex: number) => void;
  onApplySuggestedActions?: (actions: ReviewSuggestedAction[]) => void;
  onReviewCompleted?: (reviewId: string) => Promise<void> | void;
}

export const ReviewPanel = ({ sessionId, onNavigateToPage, onApplySuggestedActions, onReviewCompleted }: ReviewPanelProps) => {
  const review = useDocumentReview();
  const [includeEng, setIncludeEng] = useState(true);

  const handleReview = async () => {
    const completed = await review.reviewDocument(sessionId, includeEng);
    if (completed && onReviewCompleted) {
      await onReviewCompleted(completed.review_id);
    }
  };

  const allFindings = [
    ...(review.result?.risks ?? []),
    ...(review.result?.findings ?? []),
    ...(review.result?.missing_information ?? []),
    ...(review.result?.engineering_findings ?? []),
  ];

  const groupedFindings = {
    critical: allFindings.filter((f) => f.severity === "critical"),
    major: allFindings.filter((f) => f.severity === "major"),
    warning: allFindings.filter((f) => f.severity === "warning"),
    info: allFindings.filter((f) => f.severity === "info"),
  };

  return (
    <div className="review-panel" data-testid="review-panel">
      <div className="section-header">
        <h4 className="section-header__title">Review Document</h4>
        <span className="section-header__hint">Offline local AI</span>
      </div>

      <div className="inspector-card">
        <p className="inspector-card__title">Review settings</p>
      <div className="review-panel__options">
        <label><input type="checkbox" checked={includeEng} onChange={(e) => setIncludeEng(e.target.checked)} /> Include engineering findings</label>
      </div>

      <div className="review-panel__actions">
        <button className="btn btn--primary btn--sm" onClick={handleReview} disabled={review.status === "running"} data-testid="review-primary-action">
          {review.status === "running" ? "Reviewing…" : "Review Document"}
        </button>
        {review.result && <button className="btn btn--ghost btn--sm" onClick={() => void review.clearReview()}>Clear</button>}
      </div>
      </div>

      {review.error && (
        <div className="callout callout--danger" data-testid="review-error">
          <span className="callout__icon" aria-hidden="true">!</span>
          <div className="callout__body">
            <span className="callout__title">Review failed</span>
            {review.error}
          </div>
        </div>
      )}

      {review.result && (
        <>
          {/* Summary */}
          <div className="review-summary inspector-card">
            <p className="inspector-card__title">Summary</p>
            <p>{review.result.summary}</p>
            <span className="review-meta">{(review.result.elapsed_ms / 1000).toFixed(1)}s</span>
          </div>

          {/* Warnings */}
          {review.result.warnings.length > 0 && (
            <div className="callout callout--warn review-warnings">
              <span className="callout__icon" aria-hidden="true">!</span>
              <div className="callout__body">
                <span className="callout__title">Review warnings</span>
                {review.result.warnings.map((w, i) => <div key={i}>{w}</div>)}
              </div>
            </div>
          )}

          {/* Findings */}
          {allFindings.length > 0 && (
            <div className="review-findings">
              <h5>Findings ({allFindings.length})</h5>
              {(["critical", "major", "warning", "info"] as const).map((severity) => (
                groupedFindings[severity].length > 0 && (
                  <section
                    key={severity}
                    className={`review-severity-card review-severity-card--${severity}`}
                    data-testid={`review-severity-${severity}`}
                  >
                    <div className="review-severity-card__header">
                      <span className={`review-finding__severity review-finding__severity--${severity}`}>{severity}</span>
                      <strong>{groupedFindings[severity].length} finding{groupedFindings[severity].length !== 1 ? "s" : ""}</strong>
                    </div>
                    <ul className="review-findings-list">
                      {groupedFindings[severity].map((f) => (
                        <FindingItem key={f.finding_id} finding={f} onNavigateToPage={onNavigateToPage} />
                      ))}
                    </ul>
                  </section>
                )
              ))}
            </div>
          )}

          {/* Suggested Actions */}
          {review.result.suggested_actions.length > 0 && (
            <div className="review-suggested-actions">
              <h5>Suggested Actions ({review.result.suggested_actions.length})</h5>
              <ul>
                {review.result.suggested_actions.map((a) => (
                  <li key={a.action_id} className="review-action-item">
                    <span className="review-action-type">{formatActionType(a.action_type)}</span>
                    <span>p.{a.page_index + 1}</span>
                    <span>{a.text || a.reason}</span>
                  </li>
                ))}
              </ul>
              <button
                className="btn btn--secondary btn--sm"
                onClick={() => onApplySuggestedActions?.(review.result!.suggested_actions)}
                disabled={!onApplySuggestedActions}
                title={!onApplySuggestedActions ? "Action overlay pipeline is not available in this view." : "Apply suggested actions as visible document markups"}
                data-testid="review-apply-actions"
              >
                Apply All Suggested Actions
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};

function formatActionType(actionType: string): string {
  switch (actionType) {
    case "add_comment": return "Add comment";
    case "add_highlight": return "Add highlight";
    case "add_text_box": return "Add text box";
    case "add_redaction": return "Draft redaction";
    case "apply_stamp": return "Apply stamp";
    default: return actionType.replace(/_/g, " ");
  }
}

function FindingItem({ finding, onNavigateToPage }: { finding: ReviewFinding; onNavigateToPage?: (p: number) => void }) {
  return (
    <li className={`review-finding review-finding--${finding.severity}`}>
      <div className="review-finding__header">
        <span className={`review-finding__severity review-finding__severity--${finding.severity}`}>{finding.severity}</span>
        <strong>{finding.title}</strong>
      </div>
      <p>{finding.description}</p>
      {finding.recommendation && <p className="review-finding__rec">→ {finding.recommendation}</p>}
      {finding.page_refs.length > 0 && (
        <div className="review-finding__pages">
          {finding.page_refs.map((p) => (
            <button key={p} className="eng-page-link" onClick={() => onNavigateToPage?.(p)}>p.{p + 1}</button>
          ))}
        </div>
      )}
    </li>
  );
}
