interface RagAnswerViewProps {
  answer: string;
  elapsedMs: number;
  retrievalMode: string;
  warnings: string[];
}

export const RagAnswerView = ({ answer, elapsedMs, retrievalMode, warnings }: RagAnswerViewProps) => {
  return (
    <div className="rag-answer">
      <div className="rag-answer__text">{answer}</div>
      <div className="rag-answer__meta">
        <span>{(elapsedMs / 1000).toFixed(1)}s</span>
        <span>via {retrievalMode}</span>
      </div>
      {warnings.length > 0 && (
        <div className="rag-answer__warnings">
          {warnings.map((w, i) => <div key={i} className="rag-answer__warning">⚠ {w}</div>)}
        </div>
      )}
    </div>
  );
};
