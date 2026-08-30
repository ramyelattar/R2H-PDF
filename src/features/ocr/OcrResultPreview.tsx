import type { OcrPageResult } from "./types";

interface OcrResultPreviewProps {
  result: OcrPageResult;
}

export const OcrResultPreview = ({ result }: OcrResultPreviewProps) => {
  const confidence = result.confidence === null ? "not supplied" : `${(result.confidence * 100).toFixed(0)}%`;

  return (
    <div className="ocr-result-preview" data-testid="ocr-result-preview">
      <div className="ocr-result-preview__meta">
        <span>Page {result.page_index + 1}</span>
        <span>Engine: {result.engine}</span>
        <span>Confidence: {confidence}</span>
        <span>{result.blocks.length} blocks</span>
        <span>{result.status === "no_text_detected" ? "No text detected" : "Validated result"}</span>
      </div>
      {result.status === "no_text_detected" ? (
        <p className="empty-text">The local worker completed truthfully and detected no text on this page.</p>
      ) : (
        <>
          <div className="ocr-result-preview__text">
            {result.text.slice(0, 500)}
            {result.text.length > 500 && "…"}
          </div>
          <div
            className="ocr-result-preview__geometry"
            data-testid="ocr-result-geometry"
            style={{
              position: "relative",
              aspectRatio: `${result.page_width_points} / ${result.page_height_points}`,
              maxHeight: 240,
              background: "rgba(38,116,170,0.04)",
              border: "1px solid rgba(38,116,170,0.24)",
              overflow: "hidden",
            }}
            aria-label="Validated OCR geometry preview"
          >
            {result.blocks.map((block) => (
              <span
                key={block.id}
                title={block.text}
                style={{
                  position: "absolute",
                  left: `${(block.bbox.x / result.image_width_px) * 100}%`,
                  top: `${(block.bbox.y / result.image_height_px) * 100}%`,
                  width: `${(block.bbox.width / result.image_width_px) * 100}%`,
                  height: `${(block.bbox.height / result.image_height_px) * 100}%`,
                  border: "1px solid rgba(38,116,170,0.7)",
                  background: "rgba(38,116,170,0.12)",
                }}
              />
            ))}
          </div>
          <details className="ocr-result-preview__blocks">
            <summary>{result.blocks.length} text blocks</summary>
            <ul>
              {result.blocks.slice(0, 20).map((block) => (
                <li key={block.id}>
                  <span className="ocr-block-type">[{block.block_type}]</span>
                  <span className="ocr-block-text">{block.text.slice(0, 80)}</span>
                  <span className="ocr-block-conf">
                    {block.confidence === null ? "confidence not supplied" : `${(block.confidence * 100).toFixed(0)}%`}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        </>
      )}
    </div>
  );
};
