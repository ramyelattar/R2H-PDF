interface OcrProgressProps {
  current: number;
  total: number;
  currentPage: number | null;
}

export const OcrProgress = ({ current, total, currentPage }: OcrProgressProps) => {
  const percent = total > 0 ? Math.round((current / total) * 100) : 0;

  return (
    <div className="ocr-progress">
      <div className="ocr-progress__bar">
        <div className="ocr-progress__fill" style={{ width: `${percent}%` }} />
      </div>
      <div className="ocr-progress__label">
        {currentPage !== null && <span>Page {currentPage + 1}</span>}
        <span>{current}/{total} ({percent}%)</span>
      </div>
    </div>
  );
};
