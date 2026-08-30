import { useEffect, useRef, useState } from "react";
import type { ContentObject, FontResourceInfo } from "../../lib/ipc";

/**
 * Phase 29E — in-canvas inline text editor.
 *
 * Floats a textarea over the selected text span at the bbox derived from
 * the source PDF coords. Zoom-aware. Calls `onApply(replacement)` on
 * Enter / Ctrl+Enter and `onCancel()` on Escape.
 *
 * Honesty rules:
 *   - Read-only text spans render the textarea as disabled with a clear
 *     "Read Only" badge — there is no apply button.
 *   - When the parent calls `methodPreview = "safe_visual_replacement"`,
 *     a warning chip is shown before the user applies so they know the
 *     original font will NOT be preserved.
 *   - When no font registry entry is found the warning calls that out
 *     explicitly.
 */
export interface InlineMethodPreview {
  /** Resolved strategy hint from the registry. */
  strategy: "native_in_place" | "safe_visual_replacement" | "read_only";
  /** Whether the original font will be preserved. */
  fontPreserved: boolean;
  /** Free-form reasons surfaced to the user. */
  reasons: string[];
}

interface InlineTextEditorProps {
  /** PDF bbox of the source span [x0, y0, x1, y1] in points (Y up). */
  bbox: [number, number, number, number];
  /** Page height in points — required to flip Y-axis for top-left CSS. */
  pageHeightPts: number;
  /** Current viewer zoom factor (1.0 = 100%). */
  zoom: number;
  /** Initial text to populate the textarea. */
  initialText: string;
  /** Approximate font size in points. */
  fontSize: number;
  /** Resolved method preview shown to the user. */
  methodPreview: InlineMethodPreview;
  /** Per-font diagnostic chip data (rendered next to the preview). */
  fontInfo?: FontResourceInfo | null;
  /** Source content object for diagnostics. */
  contentObject?: ContentObject | null;
  /** Apply the edit. */
  onApply: (replacement: string) => void | Promise<void>;
  /** Close the editor without applying. */
  onCancel: () => void;
}

export function InlineTextEditor({
  bbox,
  pageHeightPts,
  zoom,
  initialText,
  fontSize,
  methodPreview,
  fontInfo,
  contentObject,
  onApply,
  onCancel,
}: InlineTextEditorProps) {
  const [text, setText] = useState(initialText);
  const [shrinkToFit, setShrinkToFit] = useState(false);
  const ref = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  const isReadOnly = methodPreview.strategy === "read_only";
  const isVisualFallback = methodPreview.strategy === "safe_visual_replacement";

  // PDF (origin bottom-left, y growing up) → screen (origin top-left).
  // Top-left of bbox in screen px = (x0 * zoom, (pageH - y1) * zoom)
  const screenLeft = bbox[0] * zoom;
  const screenTop = (pageHeightPts - bbox[3]) * zoom;
  const screenWidth = Math.max(40, (bbox[2] - bbox[0]) * zoom);
  const screenHeight = Math.max(20, (bbox[3] - bbox[1]) * zoom);
  const baseFontPx = Math.max(9, fontSize * zoom * 0.92);
  const estimatedTextWidth = Math.max(initialText.length, text.length) * baseFontPx * 0.54;
  const overflowsBox = estimatedTextWidth > screenWidth - 8;
  const effectiveFontPx = shrinkToFit && overflowsBox
    ? Math.max(8, Math.min(baseFontPx, ((screenWidth - 8) / Math.max(1, text.length)) / 0.54))
    : baseFontPx;
  const overflowState = overflowsBox ? (shrinkToFit ? "shrink" : "warn") : "fits";

  const onKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Escape") {
      e.preventDefault();
      onCancel();
      return;
    }
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (!isReadOnly) void onApply(text);
      return;
    }
    if (e.key === "Enter" && !e.shiftKey && !text.includes("\n") && screenHeight <= 28) {
      e.preventDefault();
      if (!isReadOnly) void onApply(text);
    }
  };

  return (
    <div
      className="inline-text-editor"
      data-testid="inline-text-editor"
      data-overflow-state={overflowState}
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
      style={{
        position: "absolute",
        left: screenLeft,
        top: screenTop,
        width: screenWidth,
        minHeight: screenHeight,
        zIndex: 50,
        background: "rgba(255,255,255,0.98)",
        border: `2px solid ${isReadOnly ? "#888" : isVisualFallback ? "#c80" : "#74a2ff"}`,
        borderRadius: 4,
        boxShadow: "0 4px 14px rgba(0,0,0,0.25)",
        padding: 4,
        display: "flex",
        flexDirection: "column",
        gap: 3,
      }}
    >
      <div
        aria-hidden="true"
        style={{
          position: "absolute",
          inset: -2,
          border: "1px dashed rgba(20,80,180,0.45)",
          pointerEvents: "none",
        }}
      />
      {/* Method preview chip row */}
      <div
        style={{ display: "flex", alignItems: "center", gap: 3, flexWrap: "wrap", fontSize: 10, lineHeight: 1.2 }}
        data-testid="inline-text-editor-preview"
      >
        {methodPreview.strategy === "native_in_place" && (
          <span className="badge badge-info" data-testid="inline-method-native">Native</span>
        )}
        {methodPreview.strategy === "safe_visual_replacement" && (
          <span className="badge badge-warning" data-testid="inline-method-visual">Visual Replacement</span>
        )}
        {methodPreview.strategy === "read_only" && (
          <span className="badge badge-major" data-testid="inline-method-readonly">Read Only</span>
        )}
        {fontInfo && (
          <span className="badge" title={`${fontInfo.subtype} / ${fontInfo.encoding_kind}`}>
            {fontInfo.base_font_name}
          </span>
        )}
        {fontInfo?.is_subset && <span className="badge badge-warning">Subset</span>}
        {fontInfo?.has_to_unicode && <span className="badge">ToUnicode</span>}
        {contentObject?.text_info?.operator_type && (
          <span className="badge" title={`Operator: ${contentObject.text_info.operator_type}`}>
            {contentObject.text_info.operator_type}
          </span>
        )}
        <span className="badge" data-testid="inline-verification-status">Verify: on apply</span>
      </div>

      <textarea
        ref={ref}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
        disabled={isReadOnly}
        rows={Math.max(1, Math.min(6, Math.ceil(screenHeight / Math.max(14, effectiveFontPx * 1.2))))}
        data-testid="inline-text-editor-textarea"
        style={{
          width: "100%",
          fontSize: effectiveFontPx,
          lineHeight: 1.15,
          resize: "none",
          border: "1px solid rgba(60,80,120,0.35)",
          padding: 2,
          fontFamily: "inherit",
          color: isReadOnly ? "#666" : "#000",
          background: "rgba(255,255,255,0.92)",
          boxSizing: "border-box",
        }}
      />

      {overflowsBox && (
        <div
          data-testid="inline-text-overflow-warning"
          style={{ color: "#9a5a00", fontSize: 10, lineHeight: 1.25 }}
        >
          Replacement is longer than the selected text box.
          <label style={{ display: "flex", gap: 4, alignItems: "center", marginTop: 2 }}>
            <input
              type="checkbox"
              checked={shrinkToFit}
              disabled={isReadOnly}
              onChange={(e) => setShrinkToFit(e.target.checked)}
              data-testid="inline-text-shrink-to-fit"
            />
            Shrink to fit
          </label>
        </div>
      )}

      {/* Reasons surfaced before apply */}
      {methodPreview.reasons.length > 0 && (
        <ul
          style={{ margin: 0, padding: "0 0 0 14px", color: "#a40", fontSize: 11 }}
          data-testid="inline-text-editor-reasons"
        >
          {methodPreview.reasons.slice(0, 4).map((r, i) => (
            <li key={i}>{r}</li>
          ))}
        </ul>
      )}

      <div style={{ display: "flex", gap: 4, justifyContent: "flex-end" }}>
        <button
          className="ghost-btn"
          onClick={() => onCancel()}
          data-testid="inline-text-editor-cancel"
        >
          Cancel
        </button>
        {!isReadOnly && (
          <button
            className="ghost-btn"
            onClick={() => void onApply(text)}
            disabled={text === initialText}
            data-testid="inline-text-editor-apply"
          >
            Apply
          </button>
        )}
      </div>
    </div>
  );
}
