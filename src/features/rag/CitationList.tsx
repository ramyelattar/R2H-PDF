import type { Citation } from "./types";

interface CitationListProps {
  citations: Citation[];
  onNavigateToPage?: (pageIndex: number) => void;
}

export const CitationList = ({ citations, onNavigateToPage }: CitationListProps) => {
  if (citations.length === 0) return null;

  return (
    <div className="rag-citations">
      <h5>Sources ({citations.length})</h5>
      <ul className="rag-citation-list">
        {citations.map((cite) => (
          <li key={cite.citation_id} className="rag-citation-item">
            <button
              className="rag-citation-page"
              onClick={() => onNavigateToPage?.(cite.page_index)}
              title={`Go to page ${cite.page_index + 1}`}
            >
              Page {cite.page_index + 1}
            </button>
            <span className="rag-citation-snippet">{cite.snippet}</span>
            <span className="rag-citation-score">{(cite.score * 100).toFixed(0)}%</span>
          </li>
        ))}
      </ul>
    </div>
  );
};
