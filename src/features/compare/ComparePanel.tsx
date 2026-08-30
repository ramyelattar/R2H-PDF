import { useCallback, useEffect, useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  aiReviewCompareResult,
  pdfCompareDocuments,
  reportExportCompareReview,
  type CompareMode,
  type CompareResult,
  type CompareTextChange,
  type CompareVisualChange,
  type DocumentReviewResult,
} from "../../lib/ipc";
import type { EditorObject } from "../pdf-editor/types";
import {
  bulkTextChangesToEditorObjects,
  textChangeToEditorObject,
  visualChangeToEditorObject,
} from "./changeToEditorObject";

interface ComparePanelProps {
  sessionId: string;
  sourcePath?: string;
  onNavigateToPage?: (pageIndex: number) => void;
  /**
   * Receives one or more compare-derived editor overlay objects. Wired
   * from RightInspector → App so they participate in render and export.
   */
  onAddEditorObjects?: (objects: EditorObject[]) => void;
  /** Phase 30H — called when a Compare run completes so the App-level
   *  "compare result stale after text edits" flag can clear. */
  onCompareRefreshed?: () => void;
}

type Filter = "all" | "added" | "removed" | "modified" | "visual_modified";

type WorkflowStatus = "todo" | "active" | "done";

interface StepRow {
  key: string;
  label: string;
  status: WorkflowStatus;
}

export const ComparePanel = ({ sessionId, sourcePath, onNavigateToPage, onAddEditorObjects, onCompareRefreshed }: ComparePanelProps) => {
  const [revisedPath, setRevisedPath] = useState<string | null>(null);
  const [mode, setMode] = useState<CompareMode>("text_only");
  const [includeOcr, setIncludeOcr] = useState(true);
  const [autoOcr, setAutoOcr] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [result, setResult] = useState<CompareResult | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [annotationsCreated, setAnnotationsCreated] = useState(0);
  const [aiReview, setAiReview] = useState<DocumentReviewResult | null>(null);
  const [aiBusy, setAiBusy] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [reportPath, setReportPath] = useState<string | null>(null);
  const [ocrReady, setOcrReady] = useState<boolean | null>(null);

  // Phase 26C — best-effort check: if auto-OCR is requested, surface a
  // visible OCR-readiness warning instead of silently failing per page.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const ipc = await import("../../lib/ipc");
        const res = await ipc.invokeSafe<boolean>("ocr_check_availability", {});
        if (!cancelled) setOcrReady(res.ok ? res.data : false);
      } catch {
        if (!cancelled) setOcrReady(false);
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const pickRevised = useCallback(async () => {
    setError(null);
    const picked = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (picked && typeof picked === "string") setRevisedPath(picked);
  }, []);

  const runCompare = useCallback(async () => {
    setError(null);
    setNotice(null);
    setAnnotationsCreated(0);
    setAiReview(null);
    setReportPath(null);
    setResult(null);
    if (!revisedPath) {
      setError("Choose a revised PDF first.");
      return;
    }
    if (sourcePath && normalizePdfPath(sourcePath) === normalizePdfPath(revisedPath)) {
      setError("The revised PDF must be different from the active PDF.");
      return;
    }
    setBusy(true);
    const res = await pdfCompareDocuments({
      base_session_id: sessionId,
      revised_file_path: revisedPath,
      mode,
      include_ocr: includeOcr,
      auto_ocr_scanned: autoOcr,
    });
    setBusy(false);
    if (!res.ok) {
      setError(res.error.message);
      return;
    }
    setResult(res.data);
    // Phase 30H — clear the App-level "compare stale after edits" flag.
    onCompareRefreshed?.();
  }, [revisedPath, sessionId, sourcePath, mode, includeOcr, autoOcr, onCompareRefreshed]);

  const filteredChanges: CompareTextChange[] = useMemo(() => {
    if (!result) return [];
    return result.text_changes.filter((c) =>
      filter === "all" || filter === "visual_modified" ? true : c.change_type === filter,
    ).filter((c) => filter !== "visual_modified" || c.change_type === "visual_modified");
  }, [result, filter]);

  const visualChanges: CompareVisualChange[] = useMemo(() => result?.visual_changes ?? [], [result]);

  // --- 24A: compare → annotation actions ---

  const addOneAsComment = useCallback((c: CompareTextChange) => {
    if (!onAddEditorObjects) return;
    const obj = textChangeToEditorObject(sessionId, c, "comment");
    onAddEditorObjects([obj]);
    setAnnotationsCreated((n) => n + 1);
  }, [onAddEditorObjects, sessionId]);

  const addOneAsHighlight = useCallback((c: CompareTextChange) => {
    if (!onAddEditorObjects) return;
    const obj = textChangeToEditorObject(sessionId, c, "highlight");
    onAddEditorObjects([obj]);
    setAnnotationsCreated((n) => n + 1);
  }, [onAddEditorObjects, sessionId]);

  const addOneAsRedline = useCallback((c: CompareTextChange) => {
    if (!onAddEditorObjects) return;
    const obj = textChangeToEditorObject(sessionId, c, "redline");
    onAddEditorObjects([obj]);
    setAnnotationsCreated((n) => n + 1);
  }, [onAddEditorObjects, sessionId]);

  const addAll = useCallback((kind: "comment" | "highlight" | "redline") => {
    if (!onAddEditorObjects || !result) return;
    const objects = bulkTextChangesToEditorObjects(sessionId, result.text_changes, kind);
    onAddEditorObjects(objects);
    setAnnotationsCreated((n) => n + objects.length);
    setNotice(`Added ${objects.length} change(s) as ${kind === "redline" ? "redlines" : `${kind}s`}.`);
  }, [onAddEditorObjects, result, sessionId]);

  const addOneVisual = useCallback((v: CompareVisualChange) => {
    if (!onAddEditorObjects) return;
    const obj = visualChangeToEditorObject(sessionId, v);
    onAddEditorObjects([obj]);
    setAnnotationsCreated((n) => n + 1);
  }, [onAddEditorObjects, sessionId]);

  const addAllVisual = useCallback(() => {
    if (!onAddEditorObjects || !result) return;
    const objs = result.visual_changes.map((v) => visualChangeToEditorObject(sessionId, v));
    onAddEditorObjects(objs);
    setAnnotationsCreated((n) => n + objs.length);
    setNotice(`Added ${objs.length} visual region(s) as rectangles.`);
  }, [onAddEditorObjects, result, sessionId]);

  // --- 24C: export JSON / HTML report ---

  const exportJson = useCallback(() => {
    if (!result) return;
    const blob = new Blob([JSON.stringify(result, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `compare-${result.compare_id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [result]);

  const exportHtmlReport = useCallback(async () => {
    if (!result) return;
    setError(null);
    const target = await save({
      defaultPath: `compare-${result.compare_id}.html`,
      filters: [{ name: "HTML", extensions: ["html"] }],
    });
    if (!target) return;
    const res = await reportExportCompareReview({
      compare_id: result.compare_id,
      output_path: target,
      overwrite_existing: true,
    });
    if (!res.ok) {
      setError(res.error.message);
      return;
    }
    setReportPath(res.data.output_path);
    setNotice(`Redline report written: ${res.data.bytes_written} bytes.`);
  }, [result]);

  // --- 24D: AI review of compare changes ---

  const runAiReview = useCallback(async () => {
    if (!result) return;
    setAiError(null);
    setAiBusy(true);
    const res = await aiReviewCompareResult({
      compare_id: result.compare_id,
      session_id: sessionId,
    });
    setAiBusy(false);
    if (!res.ok) {
      setAiError(res.error.message);
      return;
    }
    setAiReview(res.data);
  }, [result, sessionId]);

  const applyAiReviewAsComments = useCallback(() => {
    if (!aiReview || !onAddEditorObjects) return;
    const now = Date.now();
    const objs: EditorObject[] = aiReview.suggested_actions.map((a, i) => ({
      id: `ai-cmp-${now}-${i}`,
      sessionId,
      pageIndex: a.page_index,
      type: "comment",
      rect: { x: 36, y: 720 - i * 30, width: 240, height: 24 },
      rotation: 0,
      zIndex: 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: { source: "ai_review", action_type: a.action_type },
      contents: `[AI Review] ${a.text}\nReason: ${a.reason}`,
      author: "AI Compare Review",
      color: "#7e60ff",
      status: "open" as const,
    }));
    onAddEditorObjects(objs);
    setAnnotationsCreated((n) => n + objs.length);
    setNotice(`Applied ${objs.length} AI suggestion(s) as comments.`);
  }, [aiReview, onAddEditorObjects, sessionId]);

  // --- 24F: workflow steps ---

  const steps: StepRow[] = useMemo(() => {
    return [
      { key: "revised",  label: "1. Select revised PDF",           status: revisedPath ? "done" : "active" },
      { key: "mode",     label: "2. Choose compare mode",          status: revisedPath ? "done" : "active" },
      { key: "compare",  label: "3. Run compare",                  status: result ? "done" : (revisedPath ? "active" : "todo") },
      { key: "review",   label: "4. Review changes",               status: result ? "active" : "todo" },
      { key: "annotate", label: `5. Add annotations/redlines (${annotationsCreated})`,
        status: annotationsCreated > 0 ? "done" : (result ? "active" : "todo") },
      { key: "export",   label: "6. Export report/PDF",            status: reportPath ? "done" : (result ? "active" : "todo") },
    ];
  }, [revisedPath, result, annotationsCreated, reportPath]);

  const groupedCounts = useMemo(() => {
    const text = result?.text_changes ?? [];
    return {
      added: text.filter((c) => c.change_type === "added").length,
      removed: text.filter((c) => c.change_type === "removed").length,
      modified: text.filter((c) => c.change_type === "modified").length,
      visual: visualChanges.length,
    };
  }, [result, visualChanges.length]);

  return (
    <div className="compare-panel" data-testid="compare-panel">
      <div className="section-header">
        <h4 className="section-header__title">Compare Workflow</h4>
        <span className="section-header__hint">Base PDF is the open document</span>
      </div>

      {/* 24F step strip */}
      <ol className="compare-steps" style={{ paddingLeft: 0, marginBottom: 8 }}>
        {steps.map((s) => (
          <li
            key={s.key}
            data-testid={`compare-step-${s.key}`}
            data-status={s.status}
            style={{
              listStyle: "none",
              padding: "2px 6px",
              fontSize: 12,
              color: s.status === "done" ? "var(--ok, #5fb96b)" : s.status === "active" ? "var(--warn, #d99537)" : "var(--muted, #888)",
            }}
          >
            {s.status === "done" ? "✓ " : s.status === "active" ? "▸ " : "○ "}{s.label}
          </li>
        ))}
      </ol>

      <p className="empty-text" style={{ marginTop: 0 }}>
        Compare the open PDF (base) against a revised file. Choose mode and OCR options below.
      </p>

      <div className="compare-source" style={{ display: "flex", flexWrap: "wrap", gap: 6, marginBottom: 6 }}>
        <button className="btn btn--secondary btn--sm" onClick={() => void pickRevised()} disabled={busy} data-testid="compare-pick">
          Select revised PDF
        </button>
        <button className="btn btn--primary btn--sm" onClick={() => void runCompare()} disabled={busy || !revisedPath} data-testid="compare-run">
          {busy ? "Comparing…" : "Run Compare"}
        </button>
      </div>

      <div className="compare-options" style={{ display: "flex", flexWrap: "wrap", gap: 12, alignItems: "center", marginBottom: 6, fontSize: 12 }}>
        <label>
          Mode
          <select
            value={mode}
            onChange={(e) => setMode(e.target.value as CompareMode)}
            style={{ marginLeft: 4 }}
            data-testid="compare-mode"
          >
            <option value="text_only">Text</option>
            <option value="page_visual">Visual</option>
            <option value="combined">Combined</option>
          </select>
        </label>
        <label>
          <input
            type="checkbox"
            checked={includeOcr}
            onChange={(e) => setIncludeOcr(e.target.checked)}
            data-testid="compare-include-ocr"
          /> Include OCR for scanned pages
        </label>
        <label title="When a page has no native text and no cached OCR, run OCR before comparing.">
          <input
            type="checkbox"
            checked={autoOcr}
            onChange={(e) => setAutoOcr(e.target.checked)}
            disabled={!includeOcr}
            data-testid="compare-auto-ocr"
          /> Auto OCR scanned pages before compare
        </label>
      </div>

      {/* Phase 26C — preflight + OCR readiness hint. */}
      {(autoOcr || includeOcr) && (
        <p
          className={ocrReady === false ? "text-error" : "text-warn"}
          style={{ fontSize: 11, marginBottom: 4 }}
          data-testid="compare-ocr-warning"
        >
          {ocrReady === false
            ? "OCR worker not available — auto-OCR will warn per page. Check the OCR tab / model registry."
            : "OCR is local/offline. Scanned pages may take longer to compare."}
        </p>
      )}

      {revisedPath && (
        <p className="text-muted" style={{ fontSize: 11, marginBottom: 6, wordBreak: "break-all" }}>
          Revised: {revisedPath}
        </p>
      )}

      {error && <p className="text-error">{error}</p>}
      {notice && <p className="text-ok">{notice}</p>}

      {result && (
        <>
          <div className="compare-summary" style={{ border: "1px solid var(--border, #444)", padding: 6, borderRadius: 4, marginBottom: 8 }}>
            <strong>Summary</strong>
            <div style={{ display: "grid", gridTemplateColumns: "auto auto", gap: 4, fontSize: 12 }}>
              <span>Mode</span><strong>{formatCompareMode(result.mode)}</strong>
              <span>Pages compared</span><strong>{result.summary.pages_compared}</strong>
              <span>Pages with changes</span><strong>{result.summary.pages_with_changes}</strong>
              <span>Lines added</span><strong style={{ color: "var(--ok, #4a4)" }}>{result.summary.lines_added}</strong>
              <span>Lines removed</span><strong style={{ color: "var(--danger, #c33)" }}>{result.summary.lines_removed}</strong>
              <span>Lines modified</span><strong style={{ color: "var(--warn, #c80)" }}>{result.summary.lines_modified}</strong>
              <span>Visual regions</span><strong style={{ color: "#39f" }}>{visualChanges.length}</strong>
              <span>Identical?</span><strong>{result.summary.identical ? "Yes" : "No"}</strong>
            </div>
            {result.warnings.length > 0 && (
              <ul style={{ fontSize: 11, marginTop: 6 }}>
                {result.warnings.map((w, i) => (
                  <li key={i} className="text-warn">{w}</li>
                ))}
              </ul>
            )}
            <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 6 }}>
              <button className="ghost-btn" onClick={exportJson} data-testid="compare-export-json">Export JSON</button>
              <button className="ghost-btn" onClick={() => void exportHtmlReport()} data-testid="compare-export-html">Export HTML Redline Report</button>
              <button className="ghost-btn" onClick={() => addAll("comment")} disabled={!onAddEditorObjects} data-testid="compare-add-all-comments">
                Add All as Comments
              </button>
              <button className="ghost-btn" onClick={() => addAll("highlight")} disabled={!onAddEditorObjects} data-testid="compare-add-all-highlights">
                Add all highlights/redlines
              </button>
              <button className="ghost-btn" onClick={() => addAll("redline")} disabled={!onAddEditorObjects} data-testid="compare-add-all-redlines">
                Add all redlines
              </button>
              {visualChanges.length > 0 && (
                <button className="ghost-btn" onClick={addAllVisual} disabled={!onAddEditorObjects} data-testid="compare-add-all-visual">
                  Add All Visual as Rectangles
                </button>
              )}
            </div>
          </div>

          {/* 24D: AI review */}
          <div className="compare-ai" style={{ border: "1px solid var(--border, #444)", padding: 6, borderRadius: 4, marginBottom: 8 }}>
            <strong>AI Review of Changes</strong>
            <div style={{ display: "flex", gap: 6, margin: "4px 0", flexWrap: "wrap" }}>
              <button
                className="ghost-btn"
                onClick={() => void runAiReview()}
                disabled={aiBusy}
                data-testid="compare-ai-run"
              >
                {aiBusy ? "Thinking…" : "AI Review Changes"}
              </button>
              {aiReview && aiReview.suggested_actions.length > 0 && (
                <button
                  className="ghost-btn"
                  onClick={applyAiReviewAsComments}
                  disabled={!onAddEditorObjects}
                  data-testid="compare-ai-apply"
                >
                  Apply {aiReview.suggested_actions.length} Suggestion(s) as Comments
                </button>
              )}
            </div>
            {aiError && <p className="text-error" style={{ fontSize: 12 }}>{aiError}</p>}
            {aiReview && (
              <div style={{ fontSize: 12 }}>
                <p><strong>Summary:</strong> {aiReview.summary || "(none)"}</p>
                {aiReview.risks.length > 0 && (
                  <p><strong>Risks:</strong> {aiReview.risks.length}</p>
                )}
                {aiReview.findings.length > 0 && (
                  <ul>
                    {aiReview.findings.slice(0, 8).map((f) => (
                      <li key={f.finding_id}>
                        <strong>{f.title}</strong> — {f.description}
                      </li>
                    ))}
                  </ul>
                )}
                {aiReview.warnings.length > 0 && (
                  <ul>
                    {aiReview.warnings.map((w, i) => (
                      <li key={i} className="text-warn">{w}</li>
                    ))}
                  </ul>
                )}
              </div>
            )}
            <p className="text-muted" style={{ fontSize: 11 }}>
              Runs locally via the offline AI runtime. If the model is missing, open the Models tab to install it.
            </p>
          </div>

          <div className="compare-filters" style={{ display: "flex", gap: 6, marginBottom: 6 }}>
            <span className="compare-group-summary" data-testid="compare-group-summary">
              Added {groupedCounts.added} · Removed {groupedCounts.removed} · Modified {groupedCounts.modified} · Visual {groupedCounts.visual}
            </span>
            {(["all", "added", "removed", "modified", "visual_modified"] as const).map((f) => (
              <button
                key={f}
                className={`ghost-btn ${filter === f ? "ghost-btn--active" : ""}`}
                onClick={() => setFilter(f)}
                style={filter === f ? { fontWeight: "bold" } : undefined}
              >
                {f === "visual_modified" ? "visual" : f}
              </button>
            ))}
          </div>

          <ul className="compare-changes" style={{ maxHeight: 320, overflowY: "auto", paddingLeft: 0 }}>
            {filter !== "visual_modified" && filteredChanges.slice(0, 500).map((c, i) => (
              <li
                key={i}
                className={`compare-change compare-change--${c.change_type}`}
                style={{
                  listStyle: "none",
                  padding: 6,
                  marginBottom: 4,
                  borderLeft: `3px solid ${
                    c.change_type === "added" ? "var(--ok, #4a4)" :
                    c.change_type === "removed" ? "var(--danger, #c33)" :
                    "var(--warn, #c80)"
                  }`,
                  background: "var(--surface, #1a1a1a)",
                  fontSize: 12,
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: 4, marginBottom: 2 }}>
                  <span><strong>{c.change_type.toUpperCase()}</strong> · {c.citation}</span>
                  <span style={{ display: "flex", gap: 4 }}>
                    <button
                      className="ghost-btn"
                      onClick={() => onNavigateToPage?.(c.page_index)}
                      title={`Go to page ${c.page_index + 1}`}
                      style={{ fontSize: 11, padding: "0 4px" }}
                    >Go</button>
                    <button
                      className="ghost-btn"
                      onClick={() => addOneAsComment(c)}
                      disabled={!onAddEditorObjects}
                      title="Add Comment"
                      style={{ fontSize: 11, padding: "0 4px" }}
                      data-testid="compare-add-comment"
                    >Comment</button>
                    <button
                      className="ghost-btn"
                      onClick={() => addOneAsHighlight(c)}
                      disabled={!onAddEditorObjects}
                      title="Highlight Change"
                      style={{ fontSize: 11, padding: "0 4px" }}
                      data-testid="compare-add-highlight"
                    >Highlight</button>
                    <button
                      className="ghost-btn"
                      onClick={() => addOneAsRedline(c)}
                      disabled={!onAddEditorObjects}
                      title="Add Redline"
                      style={{ fontSize: 11, padding: "0 4px" }}
                      data-testid="compare-add-redline"
                    >Redline</button>
                  </span>
                </div>
                {c.old_text && <div style={{ color: "var(--danger, #c33)" }}>− {c.old_text}</div>}
                {c.new_text && <div style={{ color: "var(--ok, #4a4)" }}>+ {c.new_text}</div>}
              </li>
            ))}
            {filter === "visual_modified" && visualChanges.slice(0, 500).map((v, i) => (
              <li
                key={i}
                style={{
                  listStyle: "none",
                  padding: 6,
                  marginBottom: 4,
                  borderLeft: "3px solid #39f",
                  background: "var(--surface, #1a1a1a)",
                  fontSize: 12,
                }}
              >
                <div style={{ display: "flex", justifyContent: "space-between", gap: 4 }}>
                  <span><strong>VISUAL</strong> · page {v.page_index + 1} · conf {v.confidence.toFixed(2)}</span>
                  <span style={{ display: "flex", gap: 4 }}>
                    <button
                      className="ghost-btn"
                      onClick={() => onNavigateToPage?.(v.page_index)}
                      style={{ fontSize: 11, padding: "0 4px" }}
                    >Go</button>
                    <button
                      className="ghost-btn"
                      onClick={() => addOneVisual(v)}
                      disabled={!onAddEditorObjects}
                      style={{ fontSize: 11, padding: "0 4px" }}
                      data-testid="compare-add-visual"
                    >+Rect</button>
                  </span>
                </div>
                <code style={{ fontSize: 11 }}>bbox=[{v.bbox.map((n) => n.toFixed(1)).join(", ")}]</code>
              </li>
            ))}
            {filter !== "visual_modified" && filteredChanges.length === 0 && (
              <li className="empty-text">No text changes match the filter.</li>
            )}
            {filter === "visual_modified" && visualChanges.length === 0 && (
              <li className="empty-text">No visual changes (try mode=Visual or Combined).</li>
            )}
            {filteredChanges.length > 500 && (
              <li className="text-warn" style={{ fontSize: 11 }}>
                Showing first 500 of {filteredChanges.length} changes. Export JSON or HTML for the full list.
              </li>
            )}
          </ul>

          {reportPath && (
            <p className="text-ok" style={{ fontSize: 11 }}>Report: {reportPath}</p>
          )}
        </>
      )}
    </div>
  );
};

function normalizePdfPath(path: string): string {
  return path.trim().replace(/\//g, "\\").toLowerCase();
}

function formatCompareMode(mode: CompareMode): string {
  switch (mode) {
    case "text_only": return "Text";
    case "page_visual": return "Visual";
    case "combined": return "Combined";
    default: return mode;
  }
}
