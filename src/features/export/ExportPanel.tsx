import { useMemo, useState } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { EditorObject } from "../pdf-editor/types";
import type { AppendDiagnostic } from "../../lib/diagnostics";
import type { RedactionMode } from "./types";
import { usePdfExport } from "./usePdfExport";
import { ExportSummary } from "./ExportSummary";

interface ExportPanelProps {
  sessionId: string;
  sourcePath: string;
  objects: EditorObject[];
  appendDiagnostic: AppendDiagnostic;
  onLibraryUpdated?: (snapshot: import("../../types/shell").LibrarySnapshot) => void;
}

/**
 * Pass 2A — Export panel polish.
 *
 * The panel is now a workflow card: title → included items → options →
 * primary action → result/error. The "Included in export" card shows the
 * user exactly what their next export will contain, derived from the
 * actual overlay-object inventory (no fake categories).
 */
export const ExportPanel = ({
  sessionId,
  sourcePath,
  objects,
  appendDiagnostic,
  onLibraryUpdated,
}: ExportPanelProps) => {
  const { status, lastResult, error, exportPdf } = usePdfExport({
    sessionId,
    sourcePath,
    appendDiagnostic,
    onLibraryUpdated,
  });

  const [commitAnnotations, setCommitAnnotations] = useState(true);
  const [applyRedactions, setApplyRedactions] = useState(false);
  const [redactionMode, setRedactionMode] = useState<RedactionMode>("export_copy");
  const [overwriteExisting, setOverwriteExisting] = useState(false);
  const [showRedactionConfirm, setShowRedactionConfirm] = useState(false);

  const draftRedactionCount = objects.filter(
    (o) => o.type === "redaction" && "status" in o && (o as { status: string }).status === "draft",
  ).length;
  const objectCount = objects.filter((o) => !o.hidden).length;
  const isRunning = status === "running";

  // Build the "Included in export" inventory from the actual overlay
  // objects + flags. Rows stay visible even at zero so users understand
  // what the export pipeline can carry.
  const inventory = useMemo(() => {
    const byType = new Map<string, number>();
    const bySource = new Map<string, number>();
    for (const obj of objects) {
      if (obj.hidden) continue;
      byType.set(obj.type, (byType.get(obj.type) ?? 0) + 1);
      const source =
        typeof obj.metadata?.source === "string" ? (obj.metadata.source as string) : "manual";
      bySource.set(source, (bySource.get(source) ?? 0) + 1);
    }
    const get = (k: string) => byType.get(k) ?? 0;
    const fromSource = (k: string) => bySource.get(k) ?? 0;
    return {
      textBoxes: get("textBox"),
      comments: get("comment"),
      highlights: get("highlight") + get("strikethrough") + get("underline"),
      shapes: get("rectangle") + get("ellipse") + get("line") + get("arrow"),
      signatures: fromSource("signature"),
      stamps: Math.max(0, get("stamp") - fromSource("signature")),
      images: get("image"),
      forms: fromSource("form"),
      ocrOverlays: fromSource("ocr"),
      compareRedlines: fromSource("compare"),
      aiComments: fromSource("ai"),
      pathCovers: fromSource("path_cover"),
      redactionsDraft: draftRedactionCount,
    };
  }, [objects, draftRedactionCount]);

  const inventoryRows: ReadonlyArray<[string, number]> = [
    ["Text edits", inventory.textBoxes],
    ["Image edits", inventory.images + inventory.pathCovers],
    ["Annotations", inventory.comments + inventory.highlights + inventory.shapes],
    ["Signatures", inventory.signatures],
    ["Stamps", inventory.stamps],
    ["Forms", inventory.forms],
    ["OCR overlays", inventory.ocrOverlays],
    ["Compare redlines", inventory.compareRedlines],
    ["AI comments", inventory.aiComments],
  ];

  const handleExport = () => {
    if (applyRedactions && draftRedactionCount > 0) {
      setShowRedactionConfirm(true);
      return;
    }
    setOverwriteExisting(false);
    void doExport(false);
  };

  const doExport = async (replaceExisting = overwriteExisting) => {
    setShowRedactionConfirm(false);
    await exportPdf(objects, {
      commitAnnotations,
      flattenAnnotations: false,
      applyRedactions,
      redactionMode,
      overwriteExisting: replaceExisting,
    });
  };

  const handleOpenOutputFolder = async () => {
    if (!lastResult?.output_path) return;
    try {
      await revealItemInDir(lastResult.output_path);
    } catch (e) {
      appendDiagnostic({
        level: "WARN",
        source: "ui",
        message: `Open output folder failed: ${e instanceof Error ? e.message : String(e)}`,
      });
    }
  };

  const disableReason = (() => {
    if (isRunning) return "Export already running…";
    return null;
  })();

  return (
    <div className="export-panel" data-testid="export-panel">
      <div className="section-header">
        <h4 className="section-header__title">Export Edited PDF</h4>
        <span className="section-header__hint" data-testid="export-overlay-count">
          {objectCount} object{objectCount !== 1 ? "s" : ""} ready
        </span>
      </div>

      {/* — Included in export — */}
      <div className="inspector-card" data-testid="export-included-card">
        <p className="inspector-card__title">Included in this export</p>
        {objectCount === 0 ? (
          <p className="empty-state__hint" style={{ margin: 0 }}>
            No edits yet. Annotations, text edits, signatures, OCR overlays, compare redlines,
            and AI comments will appear here once you create them.
          </p>
        ) : null}
        {(
          <ul className="export-included-list" data-testid="export-included-list">
            {inventoryRows.map(([label, n]) => (
              <li key={label} className={n === 0 ? "export-included-list__item--empty" : undefined}>
                <span className="export-included-list__label">{label}</span>
                <span className="export-included-list__count">{n}</span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* — Options — */}
      <div className="inspector-card">
        <p className="inspector-card__title">Options</p>
        <div className="export-panel__options">
          <label className="export-option">
            <input
              type="checkbox"
              checked={commitAnnotations}
              onChange={(e) => setCommitAnnotations(e.target.checked)}
            />
            Commit annotations into the PDF (recommended)
          </label>
          <label className="export-option">
            <input
              type="checkbox"
              checked={applyRedactions}
              onChange={(e) => setApplyRedactions(e.target.checked)}
              disabled={draftRedactionCount === 0}
            />
            Apply draft redactions ({draftRedactionCount})
          </label>
          {applyRedactions && (
            <label className="export-option export-option--nested">
              <select
                value={redactionMode}
                onChange={(e) => setRedactionMode(e.target.value as RedactionMode)}
              >
                <option value="export_copy">Export copy (original unchanged)</option>
                <option value="apply_to_current_document">
                  Apply to current document (destructive)
                </option>
              </select>
            </label>
          )}
        </div>
      </div>

      {/* — Destructive-redaction warning callout — */}
      {applyRedactions && draftRedactionCount > 0 && (
        <div className="callout callout--warn" data-testid="export-redaction-warning">
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">
            <span className="callout__title">Redaction is permanent</span>
            Redacted content cannot be recovered from the exported file.{" "}
            {redactionMode === "export_copy"
              ? "The original document will remain unchanged."
              : "The current document will be permanently modified."}
          </div>
        </div>
      )}

      {/* — Primary action — */}
      <div className="action-row">
        <button
          className="btn btn--primary"
          onClick={handleExport}
          disabled={isRunning}
          aria-disabled={isRunning}
          title={disableReason ?? "Export the document with the options above"}
          data-testid="export-primary-btn"
        >
          {isRunning ? "Exporting…" : "Export Edited PDF"}
        </button>
        {disableReason && (
          <span
            className="section-header__hint"
            data-testid="export-disabled-reason"
          >
            {disableReason}
          </span>
        )}
      </div>

      <div className="export-secondary-actions" data-testid="export-secondary-actions">
        <button
          className="btn btn--secondary btn--sm"
          disabled
          aria-disabled="true"
          title="Use the Report panel to create a review report."
          data-testid="export-report-secondary"
        >
          Export Report
        </button>
        <button
          className="btn btn--secondary btn--sm"
          disabled={!lastResult?.redline_report_path}
          aria-disabled={!lastResult?.redline_report_path}
          title={
            lastResult?.redline_report_path
              ? "Open the last redline review report"
              : "Use the Compare panel to create a redline review first."
          }
          onClick={() => {
            if (lastResult?.redline_report_path) void revealItemInDir(lastResult.redline_report_path);
          }}
          data-testid="export-redline-secondary"
        >
          Export Redline Review
        </button>
        <button
          className="btn btn--secondary btn--sm"
          disabled={!lastResult?.output_path}
          aria-disabled={!lastResult?.output_path}
          title={
            lastResult?.output_path
              ? "Reveal the exported PDF in its folder"
              : "Export a PDF first to enable this action."
          }
          onClick={() => void handleOpenOutputFolder()}
          data-testid="export-open-folder"
        >
          Open Output Folder
        </button>
      </div>

      {/* — Confirmation prompt — */}
      {showRedactionConfirm && (
        <div className="callout callout--danger" data-testid="export-redaction-confirm">
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">
            <span className="callout__title">Confirm redaction</span>
            {draftRedactionCount} redaction{draftRedactionCount !== 1 ? "s" : ""} will be
            permanently applied to the{" "}
            {redactionMode === "export_copy" ? "exported copy" : "current document"}.
            <div className="action-row" style={{ marginTop: 8 }}>
              <button className="btn btn--danger btn--sm" onClick={() => void doExport(false)}>
                Confirm & Export
              </button>
              <button
                className="btn btn--ghost btn--sm"
                onClick={() => setShowRedactionConfirm(false)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* — Result / error — */}
      {error && /already exists/i.test(error) && !overwriteExisting && (
        <div className="callout callout--warn" data-testid="export-overwrite-confirm">
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">
            The selected destination already exists. Replace it only after confirming that the existing file may be overwritten.
            <div className="action-row" style={{ marginTop: 8 }}>
              <button
                className="btn btn--danger btn--sm"
                onClick={() => {
                  setOverwriteExisting(true);
                  void doExport(true);
                }}
                data-testid="export-overwrite-btn"
              >
                Replace existing file
              </button>
            </div>
          </div>
        </div>
      )}
      {error && (
        <div className="callout callout--danger" data-testid="export-error">
          <span className="callout__icon" aria-hidden="true">✕</span>
          <div className="callout__body">
            <span className="callout__title">Export failed</span>
            {error}
            <div className="action-row" style={{ marginTop: 8 }}>
              <button
                className="btn btn--secondary btn--sm"
                onClick={handleExport}
                data-testid="export-retry-btn"
              >
                Retry
              </button>
            </div>
          </div>
        </div>
      )}
      {lastResult && status === "completed" && <ExportSummary result={lastResult} />}
    </div>
  );
};
