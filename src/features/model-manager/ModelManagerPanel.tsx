import { useEffect } from "react";
import { useModelManager } from "./useModelManager";

/**
 * Pass 2B — Model Manager polish.
 *
 * Surfaces a clear status card (Ready / Missing / Validating) and groups
 * missing assets so users see the state at a glance. Technical paths and
 * the full asset list are tucked under "Advanced details" so the panel
 * feels calm by default.
 */
export const ModelManagerPanel = () => {
  const mm = useModelManager();

  useEffect(() => {
    void mm.getStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const v = mm.validation;
  const totalMB = v ? (v.total_size_bytes / 1_048_576).toFixed(0) : "—";

  const statusVariant: "ready" | "warning" | "error" | "info" = (() => {
    if (mm.loading && !v) return "info";
    if (!v) return "warning";
    if (v.ready) return "ready";
    if (v.required_missing_count > 0) return "error";
    return "warning";
  })();

  const statusLabel = (() => {
    if (mm.loading && !v) return "Validating";
    if (!v) return "Status unknown";
    if (v.ready) return "Ready";
    if (v.required_missing_count > 0) return "Missing required files";
    return "Optional files missing";
  })();

  const missingAssets = v?.assets.filter((a) => a.status === "missing" || a.status === "invalid") ?? [];
  const optionalMissing = v?.assets.filter((a) => a.status === "optional_missing") ?? [];

  return (
    <div className="model-manager-panel" data-testid="model-manager-panel">
      <div className="section-header">
        <h4 className="section-header__title">Local AI Runtime</h4>
        <span
          className={`ux-status-badge ux-status-badge--${statusVariant}`}
          data-testid="model-manager-status-badge"
        >
          {statusLabel}
        </span>
      </div>

      <div className="inspector-card" data-testid="model-manager-status-card">
        <p className="inspector-card__title">Status</p>
        {!v && mm.loading && (
          <div className="loading-bar" />
        )}
        {!v && !mm.loading && (
          <p className="empty-state__hint" style={{ margin: 0 }}>
            Local AI has not been validated yet. Click <strong>Validate Local AI</strong> below
            to check that the model files are installed.
          </p>
        )}
        {v && (
          <div className="inspector-grid">
            <span>Total size</span>
            <strong>{totalMB} MB</strong>
            <span>Required</span>
            <strong>
              {v.required_ok_count} ready
              {v.required_missing_count > 0 && (
                <span className="text-error" style={{ marginLeft: 4 }}>
                  · {v.required_missing_count} missing
                </span>
              )}
            </strong>
            <span>Optional</span>
            <strong>
              {optionalMissing.length === 0 ? "all present" : `${optionalMissing.length} missing`}
            </strong>
          </div>
        )}
      </div>

      {/* Primary action — keep it prominent. */}
      <div className="action-row">
        <button
          className="btn btn--primary"
          onClick={() => void mm.validate()}
          disabled={mm.loading}
          aria-disabled={mm.loading}
          data-testid="model-manager-validate-btn"
        >
          {mm.loading ? "Validating…" : "Validate Local AI"}
        </button>
      </div>

      {/* — Missing required files (only when relevant) — */}
      {v && missingAssets.length > 0 && (
        <div className="callout callout--danger" data-testid="model-manager-missing-card">
          <span className="callout__icon" aria-hidden="true">✕</span>
          <div className="callout__body">
            <span className="callout__title">
              {missingAssets.length} required file{missingAssets.length !== 1 ? "s" : ""} missing
            </span>
            <ul style={{ margin: "4px 0 0", paddingLeft: 14 }}>
              {missingAssets.map((a) => (
                <li key={a.asset_id}>
                  <strong>{a.name}</strong>
                  {a.message && <span style={{ color: "var(--text-2)" }}> — {a.message}</span>}
                </li>
              ))}
            </ul>
          </div>
        </div>
      )}

      {/* — Validation warnings (non-blocking) — */}
      {v && v.warnings.length > 0 && (
        <div className="callout callout--warn" data-testid="model-manager-warnings">
          <span className="callout__icon" aria-hidden="true">⚠</span>
          <div className="callout__body">
            <span className="callout__title">{v.warnings.length} warning{v.warnings.length !== 1 ? "s" : ""}</span>
            <ul style={{ margin: "4px 0 0", paddingLeft: 14 }}>
              {v.warnings.map((w, i) => <li key={i}>{w}</li>)}
            </ul>
          </div>
        </div>
      )}

      {mm.error && (
        <div className="callout callout--danger" role="alert" data-testid="model-manager-error">
          <span className="callout__icon" aria-hidden="true">!</span>
          <div className="callout__body">Validation request failed: {mm.error}</div>
        </div>
      )}

      {/* — Advanced details — full asset list + install root — */}
      {v && (
        <details className="inspector-card" data-testid="model-manager-advanced">
          <summary
            style={{ cursor: "pointer", fontWeight: 600, fontSize: "var(--fs-sm)" }}
          >
            Advanced details ({v.assets.length} asset{v.assets.length !== 1 ? "s" : ""})
          </summary>
          <div style={{ marginTop: 8 }}>
            <div className="inspector-grid" style={{ marginBottom: 8 }}>
              <span>Install root</span>
              <strong
                className="path-mono"
                title={v.local_ai_root}
                style={{ wordBreak: "break-all" }}
              >
                {v.local_ai_root}
              </strong>
            </div>
            <ul className="mm-asset-list">
              {v.assets.map((a) => (
                <li key={a.asset_id} className={`mm-asset mm-asset--${a.status}`}>
                  <span className="mm-asset-name">{a.name}</span>
                  <span className="mm-asset-status" aria-label={a.status}>
                    {a.status === "ok" ? "✓" : a.status === "optional_missing" ? "○" : "✗"}
                  </span>
                  <span className="mm-asset-msg">{a.message}</span>
                </li>
              ))}
            </ul>
          </div>
        </details>
      )}
    </div>
  );
};
