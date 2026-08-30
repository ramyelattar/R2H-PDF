import { useEffect, useMemo, useRef, useState } from "react";
import { docRender, type RenderResponse, type SearchMatch } from "../../lib/ipc";
import { SearchHighlightOverlay } from "./SearchHighlightOverlay";
import { decodeBase64ToBytes } from "./pdfBase64";

interface PdfCanvasViewerProps {
  sessionId: string;
  pageIndex: number;
  zoomPercent: number;
  onRenderError: (message: string) => void;
  onRenderSuccess?: (render: RenderResponse) => void;
  searchMatches?: SearchMatch[];
  activeMatchIndex?: number;
  /** Increment to force re-render (e.g., after content edit). */
  renderVersion?: number;
}

/** Monotonically increasing render request ID for stale-render protection. */
let globalRenderSeq = 0;

export const PdfCanvasViewer = ({
  sessionId,
  pageIndex,
  zoomPercent,
  onRenderError,
  onRenderSuccess,
  searchMatches,
  activeMatchIndex = 0,
  renderVersion = 0,
}: PdfCanvasViewerProps) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const hostRef = useRef<HTMLDivElement | null>(null);
  const onRenderErrorRef = useRef(onRenderError);
  const onRenderSuccessRef = useRef(onRenderSuccess);
  const [isRendering, setIsRendering] = useState(false);
  const [lastRender, setLastRender] = useState<RenderResponse | null>(null);

  // Track the latest render request to discard stale results.
  const latestRenderIdRef = useRef(0);

  const zoom = useMemo(() => Math.max(0.25, zoomPercent / 100), [zoomPercent]);

  useEffect(() => {
    onRenderErrorRef.current = onRenderError;
  }, [onRenderError]);

  useEffect(() => {
    onRenderSuccessRef.current = onRenderSuccess;
  }, [onRenderSuccess]);

  useEffect(() => {
    // Assign a unique ID to this render request for stale-render protection.
    globalRenderSeq += 1;
    const thisRenderId = globalRenderSeq;
    latestRenderIdRef.current = thisRenderId;

    let canceled = false;

    const isStale = () =>
      canceled || latestRenderIdRef.current !== thisRenderId;

    const render = async () => {
      const host = hostRef.current;
      const canvas = canvasRef.current;
      if (!host || !canvas) {
        return;
      }

      setIsRendering(true);
      const width = Math.max(320, host.clientWidth - 12);
      const height = Math.max(420, host.clientHeight - 12);
      const devicePixelRatio = window.devicePixelRatio || 1;

      const response = await docRender({
        session_id: sessionId,
        page_index: pageIndex,
        zoom,
        viewport: {
          x: 0,
          y: 0,
          width,
          height,
        },
        device_pixel_ratio: devicePixelRatio,
      });

      // Stale render protection: a newer render superseded this one. Leave
      // the loading flag alone — the newer request owns it now.
      if (isStale()) {
        return;
      }

      if (!response.ok) {
        onRenderErrorRef.current(response.error.message);
        return;
      }

      const data = response.data;
      if (data.width_px === 0 || data.height_px === 0) {
        onRenderErrorRef.current(
          `Renderer returned zero-size pixmap (${data.width_px}x${data.height_px}).`,
        );
        return;
      }
      if (!data.pixels_rgba || data.pixels_rgba.length === 0) {
        onRenderErrorRef.current("Renderer returned an empty pixel buffer.");
        return;
      }
      canvas.width = data.width_px;
      canvas.height = data.height_px;
      canvas.style.width = `${Math.round(data.width_px / devicePixelRatio)}px`;
      canvas.style.height = `${Math.round(data.height_px / devicePixelRatio)}px`;

      const context = canvas.getContext("2d");
      if (!context) {
        onRenderErrorRef.current("2D canvas context is unavailable.");
        return;
      }

      // Decode base64 RGBA pixels. Synchronous + CSP-safe: never uses fetch.
      const pixels = decodeBase64ToBytes(data.pixels_rgba);

      // Re-check staleness after the (synchronous but potentially slow) decode.
      if (isStale()) {
        return;
      }

      const expectedLen = data.width_px * data.height_px * 4;
      if (pixels.length !== expectedLen) {
        onRenderErrorRef.current(
          `Decoded pixel buffer size mismatch (got ${pixels.length}, expected ${expectedLen}).`,
        );
        return;
      }

      const imageData = new ImageData(pixels, data.width_px, data.height_px);
      context.putImageData(imageData, 0, 0);
      setLastRender(data);
      onRenderSuccessRef.current?.(data);
    };

    // CRITICAL: always clear the loading flag, even when render() throws
    // (e.g. CSP rejection, ImageData length mismatch, decode failure). A
    // previous implementation let the rejection escape via `void render()`,
    // leaving the viewer stuck with a blank canvas and a permanent
    // "Rendering page…" overlay.
    void render()
      .catch((err: unknown) => {
        if (canceled || latestRenderIdRef.current !== thisRenderId) {
          return;
        }
        const message =
          err instanceof Error ? err.message : String(err ?? "render failed");
        onRenderErrorRef.current(`Render exception: ${message}`);
      })
      .finally(() => {
        if (canceled || latestRenderIdRef.current !== thisRenderId) {
          return;
        }
        setIsRendering(false);
      });

    return () => {
      canceled = true;
    };
  }, [pageIndex, sessionId, zoom, renderVersion]);

  return (
    <div className="pdf-canvas-viewer" ref={hostRef}>
      <div style={{ position: "relative", display: "inline-block" }}>
        <canvas ref={canvasRef} className="pdf-canvas-viewer__canvas" />
        {lastRender ? (
          <SearchHighlightOverlay
            matches={searchMatches ?? []}
            activeMatchIndex={activeMatchIndex}
            pageWidth={lastRender.width}
            pageHeight={lastRender.height}
            zoom={zoom}
          />
        ) : null}
      </div>
      {isRendering ? <div className="pdf-canvas-viewer__loading">Rendering page…</div> : null}
    </div>
  );
};
