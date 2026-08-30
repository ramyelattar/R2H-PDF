import { useState } from "react";
import { useReportExport } from "./useReportExport";
import type { ReportFormat } from "./types";

interface ReportExportPanelProps {
  sessionId: string;
}

export const ReportExportPanel = ({ sessionId }: ReportExportPanelProps) => {
  const report = useReportExport();
  const [title, setTitle] = useState("Document Review Report");
  const [format, setFormat] = useState<ReportFormat>("html");
  const [includeReview, setIncludeReview] = useState(true);
  const [includeEng, setIncludeEng] = useState(true);
  const [includeTraces, setIncludeTraces] = useState(true);
  const [includeActions, setIncludeActions] = useState(true);
  const [includeAudit, setIncludeAudit] = useState(true);
  const [includeAppendix, setIncludeAppendix] = useState(true);

  const handleExport = () => {
    void report.exportReport({
      sessionId, reportTitle: title, format,
      includeReview, includeEngineering: includeEng, includeTraces,
      includeAiActions: includeActions, includeAudit, includeAppendix,
    });
  };

  return (
    <div className="report-panel" data-testid="report-panel">
      <div className="section-header">
        <h4 className="section-header__title">Report</h4>
        <span className="section-header__hint">HTML review packet</span>
      </div>

      <div className="inspector-card" data-testid="report-html-card">
        <p className="inspector-card__title">HTML report export</p>
      <div className="report-panel__field">
        <label>Title</label>
        <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} />
      </div>

      <div className="report-panel__field">
        <label>Format</label>
        <select value={format} onChange={(e) => setFormat(e.target.value as ReportFormat)}>
          <option value="html">HTML</option>
          <option value="pdf" disabled>PDF (not available in this build)</option>
        </select>
        <p className="empty-state__hint" style={{ marginTop: 4 }}>
          PDF report export is disabled because this build only implements HTML report generation.
        </p>
      </div>
      </div>

      <div className="inspector-card report-panel__options" data-testid="report-sections-card">
        <p className="inspector-card__title">Included sections</p>
        <label><input type="checkbox" checked={includeReview} onChange={(e) => setIncludeReview(e.target.checked)} /> Review findings</label>
        <label><input type="checkbox" checked={includeEng} onChange={(e) => setIncludeEng(e.target.checked)} /> Engineering findings</label>
        <label><input type="checkbox" checked={includeTraces} onChange={(e) => setIncludeTraces(e.target.checked)} /> Calculation traces</label>
        <label><input type="checkbox" checked={includeActions} onChange={(e) => setIncludeActions(e.target.checked)} /> AI actions</label>
        <label><input type="checkbox" checked={includeAudit} onChange={(e) => setIncludeAudit(e.target.checked)} /> Audit summary</label>
        <label><input type="checkbox" checked={includeAppendix} onChange={(e) => setIncludeAppendix(e.target.checked)} /> Source citations appendix</label>
      </div>

      <button className="btn btn--primary btn--sm" onClick={handleExport} disabled={report.status === "exporting"} data-testid="report-export-action">
        {report.status === "exporting" ? "Exporting…" : "Export Report"}
      </button>

      {report.error && <div className="callout callout--danger"><span className="callout__icon" aria-hidden="true">!</span><div className="callout__body">{report.error}</div></div>}

      {report.lastResult && report.status === "completed" && (
        <div className="report-panel__summary inspector-card" data-testid="report-last-result">
          <p className="inspector-card__title">Last report result</p>
          <p>Saved to: <code>{report.lastResult.output_path}</code></p>
          <p>{report.lastResult.findings_count} findings, {report.lastResult.citations_count} citations</p>
          {report.lastResult.warnings.length > 0 && (
            <div className="callout callout--warn">
              <span className="callout__icon" aria-hidden="true">!</span>
              <div className="callout__body">
                <span className="callout__title">Warnings</span>
                {report.lastResult.warnings.map((w, i) => <p key={i}>{w}</p>)}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
