import { useCallback, useEffect, useRef, useState } from "react";
import { docRender } from "../../lib/ipc";

interface ThumbnailProps {
  sessionId: string;
  pageIndex: number;
  isActive: boolean;
  onClick: (pageIndex: number) => void;
}

const Thumbnail = ({ sessionId, pageIndex, isActive, onClick }: ThumbnailProps) => {
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
        zoom: 0.18,
        viewport: { x: 0, y: 0, width: 120, height: 160 },
        device_pixel_ratio: 1,
      });

      if (canceled) return;

      if (!response.ok) {
        setError(true);
        return;
      }

      const data = response.data;
      canvas.width = data.width_px;
      canvas.height = data.height_px;
      canvas.style.width = `${data.width_px}px`;
      canvas.style.height = `${data.height_px}px`;

      const ctx = canvas.getContext("2d");
      if (!ctx) { setError(true); return; }

      const binaryString = atob(data.pixels_rgba);
      const pixels = new Uint8ClampedArray(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        pixels[i] = binaryString.charCodeAt(i);
      }
      ctx.putImageData(new ImageData(pixels, data.width_px, data.height_px), 0, 0);
      setLoaded(true);
    };

    void render();
    return () => { canceled = true; };
  }, [sessionId, pageIndex]);

  return (
    <button
      className={`thumb-item ${isActive ? "thumb-item--active" : ""}`}
      onClick={() => onClick(pageIndex)}
      title={`Page ${pageIndex + 1}`}
    >
      <div className="thumb-canvas-wrap">
        {!loaded && !error && <div className="thumb-placeholder" />}
        {error && <div className="thumb-error">!</div>}
        <canvas ref={canvasRef} className={`thumb-canvas ${loaded ? "thumb-canvas--visible" : ""}`} />
      </div>
      <span className="thumb-label">{pageIndex + 1}</span>
    </button>
  );
};

const ITEM_HEIGHT = 180; // px per thumbnail slot
const OVERSCAN = 3;      // extra items above and below visible area

interface ThumbnailStripProps {
  sessionId: string;
  totalPages: number;
  currentPage: number; // 1-based
  onNavigate: (pageIndex: number) => void;
}

export const ThumbnailStrip = ({ sessionId, totalPages, currentPage, onNavigate }: ThumbnailStripProps) => {
  const listRef = useRef<HTMLDivElement | null>(null);
  const containerHeightRef = useRef<number>(0);
  const [scrollTop, setScrollTop] = useState(0);

  // Compute virtual window using ref-based containerHeight
  const containerHeight = containerHeightRef.current;
  const firstVisible = Math.floor(scrollTop / ITEM_HEIGHT);
  const lastVisible = Math.ceil((scrollTop + containerHeight) / ITEM_HEIGHT);
  const renderEnd = Math.min(totalPages, lastVisible + OVERSCAN);
  const renderStart = Math.min(Math.max(0, firstVisible - OVERSCAN), renderEnd);

  const topSpacerHeight = renderStart * ITEM_HEIGHT;
  const bottomSpacerHeight = (totalPages - renderEnd) * ITEM_HEIGHT;

  const handleScroll = useCallback(() => {
    if (listRef.current) {
      setScrollTop(listRef.current.scrollTop);
      containerHeightRef.current = listRef.current.clientHeight;
    }
  }, []);

  // Scroll active thumbnail into view when currentPage changes programmatically
  useEffect(() => {
    const container = listRef.current;
    if (!container) return;

    const pageIndex = currentPage - 1;
    const itemTop = pageIndex * ITEM_HEIGHT;
    const itemBottom = itemTop + ITEM_HEIGHT;
    const viewTop = container.scrollTop;
    const viewBottom = viewTop + container.clientHeight;

    if (itemTop < viewTop) {
      container.scrollTop = itemTop;
    } else if (itemBottom > viewBottom) {
      container.scrollTop = itemBottom - container.clientHeight;
    }
  }, [currentPage]);

  // Measure container height on mount and when totalPages changes
  useEffect(() => {
    if (listRef.current) {
      containerHeightRef.current = listRef.current.clientHeight;
      setScrollTop(listRef.current.scrollTop);
    }
  }, [totalPages]);

  return (
    <div className="thumbnail-strip">
      <div className="thumbnail-strip__header">
        <span>Pages ({totalPages})</span>
      </div>
      <div
        className="thumbnail-strip__list"
        ref={listRef}
        onScroll={handleScroll}
      >
        {/* Top spacer for items above the render window */}
        <div style={{ height: topSpacerHeight, flexShrink: 0 }} />

        {/* Render only thumbnails in [renderStart, renderEnd) */}
        {Array.from({ length: renderEnd - renderStart }, (_, i) => {
          const pageIndex = renderStart + i;
          return (
            <Thumbnail
              key={pageIndex}
              sessionId={sessionId}
              pageIndex={pageIndex}
              isActive={currentPage === pageIndex + 1}
              onClick={onNavigate}
            />
          );
        })}

        {/* Bottom spacer for items below the render window */}
        <div style={{ height: bottomSpacerHeight, flexShrink: 0 }} />
      </div>
    </div>
  );
};
