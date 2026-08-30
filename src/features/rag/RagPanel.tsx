import { useEffect, useState } from "react";
import { useRag } from "./useRag";
import { CitationList } from "./CitationList";
import { RagAnswerView } from "./RagAnswerView";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { PageText } from "./types";

interface RagPanelProps {
  sessionId: string;
  appendDiagnostic: AppendDiagnostic;
  getPageTexts: () => Promise<PageText[]>;
  onNavigateToPage?: (pageIndex: number) => void;
  /** Phase 28F — true when text was edited after the index was built. */
  indexStale?: boolean;
  /** Phase 28F — clear the stale flag (called when the user rebuilds). */
  onIndexRefreshed?: () => void;
}

/**
 * Pass 2B — Ask PDF (RAG) panel polish.
 *
 * Two cards (Index + Ask) frame the workflow. The stale-index warning is
 * a proper callout. The empty state (no index yet) tells the user exactly
 * what to do.
 */
export const RagPanel = ({
  sessionId,
  appendDiagnostic,
  getPageTexts,
  onNavigateToPage,
  indexStale,
  onIndexRefreshed,
}: RagPanelProps) => {
  const rag = useRag({ sessionId, appendDiagnostic });
  const [question, setQuestion] = useState("");
  const [includeOcr, setIncludeOcr] = useState(false);
  const useReranker = false;

  useEffect(() => {
    void rag.refreshIndexStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleBuildIndex = async () => {
    const pageTexts = await getPageTexts();
    await rag.buildIndex(pageTexts, includeOcr);
    onIndexRefreshed?.();
  };

  const handleAsk = async () => {
    if (!question.trim()) return;
    await rag.askQuestion(question, includeOcr, useReranker);
  };

  const isIndexReady = rag.indexStatus?.status === "ready";
  const isBusy = rag.status === "building" || rag.status === "asking";

  const indexStatusBadge: "ready" | "warning" | "info" | "disabled" = (() => {
    if (rag.indexStatus?.status === "ready") return "ready";
    if (rag.indexStatus?.status === "building") return "info";
    if (rag.indexStatus?.status === "failed") return "warning";
    return "disabled";
  })();

  return (
    <div className="rag-panel" data-testid="rag-panel">
      <div className="section-header">
        <h4 className="section-header__title">Ask PDF</h4>
        <span
          className={`ux-status-badge ux-status-badge--${indexStatusBadge}`}
          data-testid="rag-index-status-badge"
        >
          {rag.indexStatus?.status === "ready" ? "Index ready" :
            rag.indexStatus?.status === "building" ? "Building…" :
              rag.indexStatus?.status === "failed" ? "Index failed" :
                "No index"}
        </span>
      </div>

      {/* — Stale-index callout — */}
      {indexStale && (
        <div
          className="callout callout--warn rag-panel__fallback-warning"
          data-testid="rag-index-stale-warning"
        >
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">
            <span className="callout__title">Index is out of date</span>
            Your edits changed the document since the last index was built.
            Click <strong>Build Index</strong> to refresh.
          </div>
        </div>
      )}

      {/* — Fallback notice from the backend — */}
      {rag.indexStatus?.fallback_reason && (
        <div className="callout callout--warn">
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">{rag.indexStatus.fallback_reason}</div>
        </div>
      )}

      {/* — Index card — */}
      <div className="inspector-card">
        <p className="inspector-card__title">Index</p>
        <div className="inspector-grid">
          <span>Chunks</span>
          <strong>{rag.indexStatus?.chunk_count ?? 0}</strong>
          <span>Retrieval mode</span>
          <strong>{rag.indexStatus?.retrieval_mode ?? "bm25"}</strong>
        </div>
        <div className="export-panel__options" style={{ marginTop: 8 }}>
          <label className="export-option">
            <input
              type="checkbox"
              checked={includeOcr}
              onChange={(e) => setIncludeOcr(e.target.checked)}
            />
            Include OCR text
          </label>
          <label className="export-option">
            <input
              type="checkbox"
              checked={false}
              disabled
              title="Reranking is not available in this build."
            />
            Use reranker (unavailable in this build)
          </label>
        </div>
        <div className="action-row" style={{ marginTop: 8 }}>
          <button
            className="btn btn--primary btn--sm"
            onClick={() => void handleBuildIndex()}
            disabled={isBusy}
            aria-disabled={isBusy}
            data-testid="rag-build-index-btn"
          >
            {rag.status === "building" ? "Building…" : "Build Index"}
          </button>
          <button
            className="btn btn--ghost btn--sm"
            onClick={() => void rag.clearIndex()}
            disabled={isBusy || !isIndexReady}
            aria-disabled={isBusy || !isIndexReady}
            data-testid="rag-clear-index-btn"
            title={!isIndexReady ? "No index to clear" : "Clear the current index"}
          >
            Clear
          </button>
        </div>
      </div>

      {/* — Ask card — */}
      <div className="inspector-card">
        <p className="inspector-card__title">Ask a question</p>
        {!isIndexReady && (
          <p className="empty-state__hint" style={{ marginTop: 0 }}>
            Build the index first so we can search the document text.
          </p>
        )}
        <textarea
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder="Ask a question about this document…"
          rows={2}
          aria-label="Question about the document"
          onKeyDown={(e) => {
            if (e.key === "Enter" && e.ctrlKey) {
              e.preventDefault();
              void handleAsk();
            }
          }}
          data-testid="rag-question-input"
        />
        <div className="action-row" style={{ marginTop: 6 }}>
          <button
            className="btn btn--primary btn--sm"
            onClick={() => void handleAsk()}
            disabled={!isIndexReady || isBusy || !question.trim()}
            aria-disabled={!isIndexReady || isBusy || !question.trim()}
            title={
              !isIndexReady
                ? "Build the index first"
                : !question.trim()
                  ? "Type a question first"
                  : "Search the document"
            }
            data-testid="rag-ask-btn"
          >
            {rag.status === "asking" ? "Thinking…" : "Ask"}
          </button>
        </div>
      </div>

      {/* — Error — */}
      {rag.error && (
        <div className="callout callout--danger" data-testid="rag-error">
          <span className="callout__icon" aria-hidden="true">✕</span>
          <div className="callout__body">{rag.error}</div>
        </div>
      )}

      {/* — Answer + citations — */}
      {rag.lastAnswer && (
        <>
          <RagAnswerView
            answer={rag.lastAnswer.answer}
            elapsedMs={rag.lastAnswer.elapsed_ms}
            retrievalMode={rag.lastAnswer.retrieval_mode_used}
            warnings={rag.lastAnswer.warnings}
          />
          <CitationList
            citations={rag.lastAnswer.citations}
            onNavigateToPage={onNavigateToPage}
          />
        </>
      )}
    </div>
  );
};
