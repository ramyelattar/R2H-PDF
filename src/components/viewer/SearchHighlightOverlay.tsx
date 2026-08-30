import { useEffect, useRef } from "react";
import type { SearchMatch } from "../../lib/ipc";

interface SearchHighlightOverlayProps {
  matches: SearchMatch[];
  activeMatchIndex: number;
  pageWidth: number;
  pageHeight: number;
  zoom: number;
}

/**
 * An absolutely-positioned canvas overlay rendered as a sibling to the PDF
 * canvas. Draws semi-transparent yellow rectangles for all search matches on
 * the current page and a distinct orange/red rectangle for the active match.
 *
 * BBox coordinates from the backend are in PDF points and are scaled by
 * `zoom * devicePixelRatio` to map to physical canvas pixels.
 */
export const SearchHighlightOverlay = ({
  matches,
  activeMatchIndex,
  pageWidth,
  pageHeight,
  zoom,
}: SearchHighlightOverlayProps) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const dpr = window.devicePixelRatio || 1;

    // Size the canvas backing store to match the PDF canvas exactly.
    const physicalWidth = pageWidth * zoom * dpr;
    const physicalHeight = pageHeight * zoom * dpr;
    canvas.width = physicalWidth;
    canvas.height = physicalHeight;

    // CSS size matches the logical (CSS pixel) dimensions of the PDF canvas.
    canvas.style.width = `${pageWidth * zoom}px`;
    canvas.style.height = `${pageHeight * zoom}px`;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    ctx.clearRect(0, 0, physicalWidth, physicalHeight);

    if (matches.length === 0) return;

    // Draw all non-active matches first (underneath the active highlight).
    ctx.fillStyle = "rgba(255, 255, 0, 0.35)";
    for (let i = 0; i < matches.length; i++) {
      if (i === activeMatchIndex) continue;
      const { bbox } = matches[i];
      const x = bbox.x0 * zoom * dpr;
      const y = bbox.y0 * zoom * dpr;
      const w = (bbox.x1 - bbox.x0) * zoom * dpr;
      const h = (bbox.y1 - bbox.y0) * zoom * dpr;
      ctx.fillRect(x, y, w, h);
    }

    // Draw the active match on top with a distinct orange/red fill and border.
    if (activeMatchIndex >= 0 && activeMatchIndex < matches.length) {
      const { bbox } = matches[activeMatchIndex];
      const x = bbox.x0 * zoom * dpr;
      const y = bbox.y0 * zoom * dpr;
      const w = (bbox.x1 - bbox.x0) * zoom * dpr;
      const h = (bbox.y1 - bbox.y0) * zoom * dpr;

      ctx.fillStyle = "rgba(255, 140, 0, 0.6)";
      ctx.fillRect(x, y, w, h);

      ctx.strokeStyle = "rgba(200, 60, 0, 0.9)";
      ctx.lineWidth = 2 * dpr;
      ctx.strokeRect(x, y, w, h);
    }
  }, [matches, activeMatchIndex, zoom, pageWidth, pageHeight]);

  return (
    <canvas
      ref={canvasRef}
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        pointerEvents: "none",
      }}
    />
  );
};
