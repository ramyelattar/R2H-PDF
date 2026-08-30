import { useEffect, useRef, useState } from "react";
import { docRender } from "../../lib/ipc";

interface PageThumbnailItemProps {
  sessionId: string;
  pageIndex: number;
  rotation: number;
  isSelected: boolean;
  hasOverlayObjects: boolean;
  onClick: (pageIndex: number) => void;
}

export const PageThumbnailItem = ({
  sessionId,
  pageIndex,
  rotation,
  isSelected,
  hasOverlayObjects,
  onClick,
}: PageThumbnailItemProps) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState(false);

  useEffect(() => {
    let canceled = false;
    setLoaded(false);
    setError(false);

    const render = async () => {
      const canvas = canvasRef.current;
      if (!canvas) return;

      const response = await docRender({
        session_id: sessionId,
        page_index: pageIndex,
        zoom: 0.15,
        viewport: { x: 0, y: 0, width: 100, height: 140 },
        device_pixel_ratio: 1,
      });

      if (canceled) return;
      if (!response.ok) { setError(true); return; }

      const data = response.data;
      canvas.width = data.width_px;
      canvas.height = data.height_px;
      const ctx = canvas.getContext("2d");
      if (!ctx) { setError(true); return; }

      const binaryString = atob(data.pixels_rgba);
      const pixels = new Uint8ClampedArray(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) pixels[i] = binaryString.charCodeAt(i);
      ctx.putImageData(new ImageData(pixels, data.width_px, data.height_px), 0, 0);
      setLoaded(true);
    };

    void render();
    return () => { canceled = true; };
  }, [sessionId, pageIndex, rotation]);

  return (
    <button
      className={`page-thumb-item ${isSelected ? "page-thumb-item--selected" : ""}`}
      onClick={() => onClick(pageIndex)}
      title={`Page ${pageIndex + 1}${rotation ? ` (${rotation}°)` : ""}`}
    >
      <div className="page-thumb-canvas-wrap">
        <canvas ref={canvasRef} className={`page-thumb-canvas ${loaded ? "page-thumb-canvas--visible" : ""}`} />
        {!loaded && !error && <div className="page-thumb-placeholder">…</div>}
        {error && <div className="page-thumb-error">✕</div>}
      </div>
      <div className="page-thumb-label">
        <span>{pageIndex + 1}</span>
        {rotation !== 0 && <span className="page-thumb-rotation">↻{rotation}°</span>}
        {hasOverlayObjects && <span className="page-thumb-dirty">●</span>}
      </div>
    </button>
  );
};
