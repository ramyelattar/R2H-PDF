import { useState } from "react";
import { useEngineering } from "./useEngineering";
import type { UnitParseResult } from "./types";

interface EngineeringPanelProps {
  sessionId: string;
  onNavigateToPage?: (pageIndex: number) => void;
}

export const EngineeringPanel = ({ sessionId, onNavigateToPage }: EngineeringPanelProps) => {
  const eng = useEngineering();
  const [unitInput, setUnitInput] = useState("");
  const [unitResult, setUnitResult] = useState<UnitParseResult | null>(null);
  const [includeOcr, setIncludeOcr] = useState(false);

  const handleParseUnits = async () => {
    const result = await eng.parseUnits(unitInput);
    setUnitResult(result);
  };

  const handleExtract = async () => {
    await eng.extractLoadSchedule(sessionId, includeOcr);
  };

  const nativeCount = eng.pageTexts.filter((t) => t.source === "native_text" || t.source === "mixed").length;
  const ocrCount = eng.pageTexts.filter((t) => t.source === "ocr_text" || t.source === "mixed").length;
  const emptyCount = eng.pageTexts.filter((t) => t.source === "empty").length;

  return (
    <div className="engineering-panel" data-testid="engineering-panel">
      <div className="section-header">
        <h4 className="section-header__title">Engineering</h4>
        <span className="section-header__hint">Loads, units, findings</span>
      </div>

      {eng.pageTexts.length === 0 && eng.loadRows.length === 0 && !eng.loading && (
        <div className="empty-state" data-testid="engineering-empty-state">
          <span className="empty-state__icon" aria-hidden="true">kW</span>
          <p className="empty-state__title">No engineering data extracted yet</p>
          <p className="empty-state__hint">Extract a load schedule first. Use OCR when the schedule is scanned or image-based.</p>
        </div>
      )}

      {/* Text source status */}
      {eng.pageTexts.length > 0 && (
        <div className="eng-text-status">
          <span>Native: {nativeCount} pages</span>
          <span>OCR: {ocrCount} pages</span>
          {emptyCount > 0 && <span>Empty: {emptyCount} pages</span>}
        </div>
      )}

      {/* Unit Normalizer */}
      <details className="eng-section inspector-card">
        <summary>Unit Normalizer</summary>
        <textarea value={unitInput} onChange={(e) => setUnitInput(e.target.value)} placeholder="Paste text with electrical values…" rows={2} />
        <button className="ghost-btn" onClick={() => void handleParseUnits()}>Parse Units</button>
        {unitResult && (
          <div className="eng-unit-results">
            {unitResult.values.map((v, i) => (
              <div key={i} className="eng-unit-row">
                <span>{v.original_text}</span> → <strong>{v.normalized_value.toFixed(2)} {v.normalized_unit}</strong>
              </div>
            ))}
            {unitResult.warnings.map((w, i) => <div key={i} className="eng-warning">⚠ {w}</div>)}
          </div>
        )}
      </details>

      {/* Load Schedule */}
      <details className="eng-section inspector-card" open>
        <summary>Load Schedule ({eng.loadRows.length} rows)</summary>
        <div className="eng-options">
          <label><input type="checkbox" checked={includeOcr} onChange={(e) => setIncludeOcr(e.target.checked)} /> Include OCR text</label>
        </div>
        <div className="eng-actions">
          <button className="ghost-btn" onClick={() => void handleExtract()} disabled={eng.loading}>
            {eng.loading ? "Extracting…" : "Extract Load Schedule"}
          </button>
          <button className="ghost-btn" onClick={() => void eng.generateFindings()} disabled={eng.loadRows.length === 0}>
            Generate Findings
          </button>
          <button className="ghost-btn" onClick={() => void eng.clearState()}>Clear</button>
        </div>
        {eng.loadRows.length > 0 && (
          <div className="eng-table-wrap">
            <table className="eng-table">
              <thead><tr><th>Pg</th><th>Tag</th><th>kW</th><th>kVA</th><th>A</th><th>V</th><th>PF</th><th>Conf</th></tr></thead>
              <tbody>
                {eng.loadRows.map((row) => (
                  <tr key={row.row_id}>
                    <td><button className="eng-page-link" onClick={() => onNavigateToPage?.(row.page_index)}>{row.page_index + 1}</button></td>
                    <td>{row.equipment_tag || "—"}</td>
                    <td>{row.power_kw?.toFixed(1) ?? "—"}</td>
                    <td>{row.apparent_power_kva?.toFixed(1) ?? "—"}</td>
                    <td>{row.current_a?.toFixed(1) ?? "—"}</td>
                    <td>{row.voltage_v?.toFixed(0) ?? "—"}</td>
                    <td>{row.power_factor?.toFixed(2) ?? "—"}</td>
                    <td>{(row.confidence * 100).toFixed(0)}%</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </details>

      {/* Findings */}
      {eng.findings.length > 0 && (
        <details className="eng-section inspector-card" open data-testid="engineering-findings">
          <summary>Findings ({eng.findings.length})</summary>
          <ul className="eng-findings-list">
            {(["critical", "major", "warning", "info"] as const).map((severity) =>
              eng.findings.filter((f) => f.severity === severity).map((f) => (
                <li key={f.finding_id} className={`eng-finding eng-finding--${f.severity}`} data-testid={`engineering-finding-${severity}`}>
                  <span className={`review-finding__severity review-finding__severity--${severity}`}>{severity}</span>
                  <strong>{f.title}</strong>
                  <p>{f.description}</p>
                  {f.recommendation && <p className="eng-recommendation">Recommendation: {f.recommendation}</p>}
                  {f.page_refs.length > 0 && (
                    <div className="eng-page-refs">
                      Pages: {f.page_refs.map((p) => (
                        <button key={p} className="eng-page-link" onClick={() => onNavigateToPage?.(p)}>{p + 1}</button>
                      ))}
                    </div>
                  )}
                  {f.calculation_trace_id && (
                    <button className="ghost-btn" onClick={() => void eng.getTrace(f.calculation_trace_id!)} data-testid="engineering-load-trace">
                      Load calculation trace
                    </button>
                  )}
                </li>
              )),
            )}
          </ul>
        </details>
      )}

      {eng.traces.length > 0 && (
        <details className="eng-section inspector-card" open data-testid="engineering-traces">
          <summary>Calculation traces ({eng.traces.length})</summary>
          {eng.traces.map((trace) => (
            <div key={trace.calculation_id} className="eng-trace-card">
              <strong>{trace.title}</strong>
              <div className="inspector-grid">
                <span>Final value</span>
                <strong>{trace.final_value.toFixed(2)} {trace.final_unit}</strong>
                <span>Type</span>
                <strong>{trace.calculation_type.replace(/_/g, " ")}</strong>
              </div>
              <ol className="eng-trace-steps">
                {trace.steps.map((step) => (
                  <li key={step.step_id}>
                    <strong>{step.description}</strong>
                    <code>{step.formula}</code>
                  </li>
                ))}
              </ol>
            </div>
          ))}
        </details>
      )}

      {eng.error && <div className="callout callout--danger"><span className="callout__icon" aria-hidden="true">!</span><div className="callout__body">{eng.error}</div></div>}
    </div>
  );
};
