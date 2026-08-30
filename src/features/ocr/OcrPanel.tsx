import { useCallback, useEffect, useState } from "react";
import { useOcr } from "./useOcr";
import { OcrProgress } from "./OcrProgress";
import { OcrResultPreview } from "./OcrResultPreview";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import {
  pdfCreateOcrEditableOverlays,
  pdfCreateOcrTextLayer,
  type OcrOverlaySpec,
} from "../../lib/ipc";
import type { OcrPageResult } from "./types";

interface OcrPanelProps {
  sessionId: string;
  currentPageIndex: number;
  totalPages: number;
  appendDiagnostic: AppendDiagnostic;
  /** The App-level acceptance path validates and persists through project_save. */
  onAcceptOcrOverlays?: (result: OcrPageResult, overlays: OcrOverlaySpec[]) => Promise<boolean> | boolean;
}

export const OcrPanel = ({
  sessionId,
  currentPageIndex,
  totalPages,
  appendDiagnostic,
  onAcceptOcrOverlays,
}: OcrPanelProps) => {
  const {
    state,
    checkAvailability,
    ocrCurrentPage,
    ocrPageRange,
    cancelOcr,
    setAcceptance,
  } = useOcr({ sessionId, totalPages, appendDiagnostic });

  useEffect(() => { void checkAvailability(); }, [checkAvailability]);

  const isBusy = state.status === "running" || state.status === "validating";
  const currentResult = state.lastResult?.page_index === currentPageIndex ? state.lastResult : null;

  const [overlayBusy, setOverlayBusy] = useState(false);
  const [overlayError, setOverlayError] = useState<string | null>(null);
  const [overlayNotice, setOverlayNotice] = useState<string | null>(null);
  const [layerStatus, setLayerStatus] = useState<string | null>(null);
  const [useOcrInIndex, setUseOcrInIndex] = useState(false);
  const [overlayDiagnostics, setOverlayDiagnostics] = useState<{
    coordinate_space?: string;
    conversion_scale?: [number, number];
  } | null>(null);

  const handleCreateOverlays = useCallback(async () => {
    setOverlayError(null);
    setOverlayNotice(null);
    setOverlayDiagnostics(null);
    if (!currentResult || currentResult.status !== "completed") {
      setOverlayError("Run OCR on the current page and receive a validated result before accepting overlays.");
      return;
    }
    if (!onAcceptOcrOverlays) {
      setOverlayError("OCR acceptance is not wired to the canonical project-save path.");
      return;
    }

    setOverlayBusy(true);
    const res = await pdfCreateOcrEditableOverlays({
      session_id: sessionId,
      page_index: currentPageIndex,
    });
    if (!res.ok) {
      setOverlayBusy(false);
      setAcceptance("failed", 0);
      setOverlayError(res.error.message);
      return;
    }

    const firstSpec = res.data.overlays[0];
    if (firstSpec) {
      setOverlayDiagnostics({
        coordinate_space: firstSpec.coordinate_space,
        conversion_scale: firstSpec.conversion_scale,
      });
    }
    for (const warning of res.data.warnings) {
      appendDiagnostic({ level: "WARN", source: "ipc", message: `OCR overlays: ${warning}` });
    }

    if (res.data.status === "no_text_detected" || res.data.overlays.length === 0) {
      setOverlayBusy(false);
      setAcceptance("none", 0);
      setOverlayNotice(`No OCR overlays were accepted for page ${currentPageIndex + 1}; the worker reported no usable text geometry.`);
      return;
    }

    const persisted = await onAcceptOcrOverlays(currentResult, res.data.overlays);
    setOverlayBusy(false);
    if (!persisted) {
      setAcceptance("failed", 0);
      setOverlayError("OCR overlays were not persisted. The project remains unsaved and the operation can be retried.");
      return;
    }
    setAcceptance("persisted", res.data.overlays.length);
    setOverlayNotice(`Persisted ${res.data.overlays.length} validated OCR overlay(s) through the project sidecar. These are visual editable overlays, not embedded searchable PDF text.`);
  }, [appendDiagnostic, currentPageIndex, currentResult, onAcceptOcrOverlays, sessionId, setAcceptance]);

  const handleCreateSearchableLayer = useCallback(async () => {
    setOverlayError(null);
    setLayerStatus(null);
    const res = await pdfCreateOcrTextLayer({
      session_id: sessionId,
      page_index: currentPageIndex,
    });
    if (!res.ok) {
      setOverlayError(res.error.message);
      return;
    }
    setLayerStatus(`Searchable PDF text layer status: ${res.data.status}. ${res.data.note}`);
    for (const warning of res.data.warnings) {
      appendDiagnostic({ level: "WARN", source: "ipc", message: `OCR text layer: ${warning}` });
    }
  }, [appendDiagnostic, currentPageIndex, sessionId]);

  const availabilityMessage = state.availability?.message ?? state.error;
  const availabilityStatus = state.availability?.status ?? "resource_resolution_failed";

  return (
    <div className="ocr-panel" data-testid="ocr-panel">
      <div className="section-header">
        <h4 className="section-header__title">OCR</h4>
        <span className={`ocr-status-badge ${state.available ? "ocr-status-badge--ready" : "ocr-status-badge--unavailable"}`}>
          {state.available ? "Ready" : state.status === "validating" ? "Checking" : "Blocked"}
        </span>
      </div>

      {!state.available && availabilityMessage && (
        <div className="callout callout--warning" role="status" data-testid="ocr-availability-message">
          <span className="callout__icon" aria-hidden="true">!</span>
          <div className="callout__body">
            <span className="callout__title">OCR unavailable ({availabilityStatus})</span>
            {availabilityMessage}
          </div>
        </div>
      )}

      <div className="inspector-card">
        <p className="inspector-card__title">Run OCR</p>
        <p className="empty-state__hint" style={{ marginTop: 0 }}>
          Extract text from the current page or a page range using the local worker and validated active-session render.
        </p>
        <div className="ocr-panel__actions">
          <button
            className="btn btn--primary btn--sm"
            onClick={() => void ocrCurrentPage(currentPageIndex, false)}
            disabled={isBusy || !state.available || totalPages === 0}
            title={!state.available ? "OCR is blocked until its local worker, runtime, model, and processor are available." : "Run OCR on the current page"}
            data-testid="ocr-run-primary"
          >
            Run OCR
          </button>
          <button
            className="btn btn--secondary btn--sm"
            onClick={() => void ocrPageRange(0, totalPages - 1, false)}
            disabled={isBusy || !state.available || totalPages === 0}
          >
            Run All Pages
          </button>
          <button
            className="btn btn--ghost btn--sm"
            onClick={() => void ocrCurrentPage(currentPageIndex, true)}
            disabled={isBusy || !state.available || totalPages === 0}
            title="Force re-run OCR even if cached"
          >
            Force Re-run
          </button>
          {isBusy && (
            <button className="btn btn--danger btn--sm" onClick={() => void cancelOcr()} data-testid="ocr-cancel">
              Cancel
            </button>
          )}
        </div>
      </div>

      {state.status === "running" && (
        <OcrProgress current={state.progressCurrent} total={state.progressTotal} currentPage={state.currentPage} />
      )}

      {state.status === "cancelled" && (
        <div className="callout callout--warning" role="status" data-testid="ocr-cancelled">OCR cancelled. No result was accepted or persisted.</div>
      )}

      {state.error && state.status !== "blocked" && state.status !== "cancelled" && (
        <div className="callout callout--danger" role="alert" data-testid="ocr-error">
          <span className="callout__icon" aria-hidden="true">!</span>
          <div className="callout__body">
            <span className="callout__title">OCR failed</span>
            {state.error}
          </div>
        </div>
      )}

      {state.lastResult && (state.status === "completed" || state.status === "no_text_detected") && (
        <OcrResultPreview result={state.lastResult} />
      )}

      <section className="inspector-card" data-testid="ocr-overlay-card">
        <p className="inspector-card__title">Accept OCR overlays</p>
        <p className="empty-text" style={{ fontSize: 11, margin: "4px 0" }}>
          Accept only validated model geometry for the current page. Acceptance enters the canonical editor collection and completes only after project_save succeeds.
        </p>
        <label className="export-option" title="Allow the RAG index to include cached OCR text when available.">
          <input type="checkbox" checked={useOcrInIndex} onChange={(event) => setUseOcrInIndex(event.target.checked)} />
          Use OCR in index
        </label>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
          <button
            className="btn btn--secondary btn--sm"
            onClick={() => void handleCreateOverlays()}
            disabled={overlayBusy || !currentResult || currentResult.status !== "completed" || !onAcceptOcrOverlays}
            data-testid="ocr-create-overlays"
            title={!onAcceptOcrOverlays ? "The canonical project-save acceptance path is not wired." : "Validate and persist OCR overlays"}
          >
            {overlayBusy ? "Saving..." : "Accept OCR overlays"}
          </button>
          <button
            className="btn btn--ghost btn--sm"
            onClick={() => void handleCreateSearchableLayer()}
            data-testid="ocr-create-searchable-layer"
            title="Reports status only; this build does not implement searchable PDF text-layer injection."
          >
            Check text-layer status
          </button>
        </div>
        {state.acceptance === "persisted" && <p className="text-ok" style={{ fontSize: 11 }}>Persisted {state.persistedBlockCount} OCR overlay(s).</p>}
        {overlayError && <p className="text-error" style={{ fontSize: 11 }}>{overlayError}</p>}
        {overlayNotice && <p className="text-ok" style={{ fontSize: 11 }}>{overlayNotice}</p>}
        {layerStatus && <p className="text-warn" style={{ fontSize: 11 }}>{layerStatus}</p>}
        {overlayDiagnostics && (
          <details style={{ marginTop: 4, fontSize: 11 }} data-testid="ocr-overlay-diagnostics">
            <summary>Conversion diagnostics</summary>
            <p style={{ margin: "2px 0" }}>Coordinate space: <code>{overlayDiagnostics.coordinate_space ?? "—"}</code></p>
            {overlayDiagnostics.conversion_scale && (
              <p style={{ margin: "2px 0" }}>Conversion scale: <code>[{overlayDiagnostics.conversion_scale[0].toFixed(4)}, {overlayDiagnostics.conversion_scale[1].toFixed(4)}]</code></p>
            )}
          </details>
        )}
        {!onAcceptOcrOverlays && (
          <p className="text-warn" style={{ fontSize: 11 }}>
            Canonical OCR acceptance is unavailable until App.tsx provides the project-save callback.
          </p>
        )}
      </section>
    </div>
  );
};
