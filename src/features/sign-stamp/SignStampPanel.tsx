import { useCallback, useRef, useState } from "react";
import type { EditorObject, StampObject } from "../pdf-editor/types";
import { STAMP_PRESETS, type StampPreset, composeStampText } from "./presets";

interface SignStampPanelProps {
  sessionId: string;
  activePageIndex: number;
  /** Hook that the parent provides to add a new overlay object. */
  onAddOverlay: (obj: EditorObject) => void;
}

interface StampDraft {
  preset: StampPreset;
  customText: string;
  includeDate: boolean;
  author: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

const DEFAULT_RECT = { x: 72, y: 600, width: 200, height: 50 };
const MAX_SIGNATURE_BYTES = 5 * 1024 * 1024;

function isValidRect(rect: { x: number; y: number; width: number; height: number }): boolean {
  return [rect.x, rect.y, rect.width, rect.height].every(Number.isFinite) && rect.width > 0 && rect.height > 0;
}

export const SignStampPanel = ({
  sessionId,
  activePageIndex,
  onAddOverlay,
}: SignStampPanelProps) => {
  const [draft, setDraft] = useState<StampDraft>(() => ({
    preset: STAMP_PRESETS[0],
    customText: "",
    includeDate: true,
    author: "",
    ...DEFAULT_RECT,
  }));
  const [signatureImage, setSignatureImage] = useState<string | null>(null);
  const [signatureLabel, setSignatureLabel] = useState<string>("");
  const [sigRect, setSigRect] = useState({ x: 72, y: 100, width: 220, height: 60 });
  // Phase 27A — preserve image aspect ratio when embedding the signature.
  // Default: enabled. Disabled means the signature is stretched to the rect.
  const [preserveAspect, setPreserveAspect] = useState<boolean>(true);
  const [imageNaturalSize, setImageNaturalSize] = useState<{ w: number; h: number } | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // ─── 25C: Add stamp ────────────────────────────────────────────────────
  const handleAddStamp = useCallback(() => {
    setError(null);
    const now = Date.now();
    const text = composeStampText({
      baseText: draft.preset.defaultText,
      includeDate: draft.includeDate,
      author: draft.author,
      customText: draft.preset.id === "CUSTOM" ? draft.customText : undefined,
    });
    if (text.trim().length === 0) {
      setError("Stamp text is required (use a preset or type a custom message).");
      return;
    }
    if (!isValidRect({ x: draft.x, y: draft.y, width: draft.width, height: draft.height })) {
      setError("Stamp position and size must be finite, with positive width and height.");
      return;
    }
    const obj: StampObject = {
      id: `stamp-${now}-${Math.floor(Math.random() * 1000)}`,
      sessionId,
      pageIndex: activePageIndex,
      type: "stamp",
      rect: { x: draft.x, y: draft.y, width: draft.width, height: draft.height },
      rotation: 0,
      zIndex: 1,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: { source: "manual", preset: draft.preset.id, includeDate: draft.includeDate },
      stampText: text,
      stampType: draft.preset.id,
      color: draft.preset.color,
    };
    onAddOverlay(obj);
    setNotice(`Added "${draft.preset.label}" stamp to page ${activePageIndex + 1}.`);
  }, [draft, sessionId, activePageIndex, onAddOverlay]);

  // ─── 25B: Visual signature image ───────────────────────────────────────
  // Uses the standard HTML file-input which Tauri's webview exposes
  // identically to a desktop browser. The image is loaded as a base64
  // data URL into the overlay's metadata for in-app preview; the exported
  // PDF carries a Stamp annotation with the user-supplied label (NOT a
  // cryptographic signature).
  const handlePickImage = useCallback(() => {
    setError(null);
    setNotice(null);
    fileInputRef.current?.click();
  }, []);

  const handleBrowserFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    if (!file.type.startsWith("image/")) {
      setError("Choose an image file for the visual signature.");
      e.target.value = "";
      return;
    }
    if (file.size > MAX_SIGNATURE_BYTES) {
      setError("Signature images must be 5 MB or smaller.");
      e.target.value = "";
      return;
    }
    const reader = new FileReader();
    reader.onload = () => {
      const dataUrl = reader.result as string;
      setSignatureImage(dataUrl);
      setSignatureLabel(file.name);
      // Phase 27A — record the image's natural dimensions so the export
      // pipeline can preserve aspect ratio and the preview can show a
      // fitted box.
      const img = new Image();
      img.onload = () => {
        if (img.naturalWidth > 0 && img.naturalHeight > 0) {
          setImageNaturalSize({ w: img.naturalWidth, h: img.naturalHeight });
        } else {
          setImageNaturalSize(null);
        }
      };
      img.onerror = () => setImageNaturalSize(null);
      img.src = dataUrl;
    };
    reader.onerror = () => setError("The signature image could not be read.");
    reader.readAsDataURL(file);
  };

  // Phase 27A — compute the fitted (letterboxed/pillarboxed) rect inside
  // sigRect for the preview / payload, exposed as a helper.
  const fittedSigRect = useCallback(() => {
    if (!preserveAspect || !imageNaturalSize) return { ...sigRect };
    const targetAspect = sigRect.width / Math.max(1e-3, sigRect.height);
    const imgAspect = imageNaturalSize.w / Math.max(1e-3, imageNaturalSize.h);
    if (Math.abs(targetAspect - imgAspect) < 1e-3) return { ...sigRect };
    if (imgAspect > targetAspect) {
      // image wider than rect → fit to width, center vertically (letterbox)
      const w = sigRect.width;
      const h = w / imgAspect;
      return {
        x: sigRect.x,
        y: sigRect.y + (sigRect.height - h) / 2,
        width: w,
        height: h,
      };
    } else {
      // image taller than rect → fit to height, center horizontally (pillarbox)
      const h = sigRect.height;
      const w = h * imgAspect;
      return {
        x: sigRect.x + (sigRect.width - w) / 2,
        y: sigRect.y,
        width: w,
        height: h,
      };
    }
  }, [preserveAspect, imageNaturalSize, sigRect]);

  const handlePlaceSignature = useCallback(() => {
    setError(null);
    if (!signatureImage) {
      setError("Choose a signature image first.");
      return;
    }
    if (!isValidRect(sigRect)) {
      setError("Signature position and size must be finite, with positive width and height.");
      return;
    }
    const now = Date.now();
    // We render the signature as a stamp annotation whose text is the
    // user-supplied label ("Signature: J. Doe"). The raw image preview is
    // kept inside the overlay's metadata so the in-app overlay shows the
    // bitmap; the exported PDF carries a Stamp annotation with the
    // signature label — explicitly NOT a cryptographic signature.
    const labelLine = signatureLabel ? `Signature — ${signatureLabel}` : "Signature";
    const obj: StampObject = {
      id: `sig-${now}-${Math.floor(Math.random() * 1000)}`,
      sessionId,
      pageIndex: activePageIndex,
      type: "stamp",
      // The overlay rect remains the user-drawn target rect; aspect-ratio
      // letterboxing is applied at export time by the backend so the
      // editor and the exported PDF agree on the final placement.
      rect: { ...sigRect },
      rotation: 0,
      zIndex: 5,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: {
        source: "signature",
        signature_kind: "visual_image_only",
        image_data_url: signatureImage,
        image_label: signatureLabel,
        // Phase 27A — payload signal: backend decides whether to letterbox
        // the image inside the target rect or stretch it.
        preserve_aspect: preserveAspect,
        image_natural_width: imageNaturalSize?.w ?? null,
        image_natural_height: imageNaturalSize?.h ?? null,
      },
      stampText: labelLine,
      stampType: "CUSTOM",
      color: "#0c4ea3",
    };
    onAddOverlay(obj);
    setNotice(
      `Placed visual signature on page ${activePageIndex + 1}` +
        (preserveAspect ? " (aspect ratio preserved)." : " (stretched to rect)."),
    );
  }, [signatureImage, signatureLabel, sigRect, sessionId, activePageIndex, onAddOverlay, preserveAspect, imageNaturalSize]);

  return (
    <div className="sign-stamp-panel" data-testid="sign-stamp-panel">
      <div className="section-header">
        <h4 className="section-header__title">Sign &amp; Stamp</h4>
        <span className="section-header__hint">Visual markups</span>
      </div>

      <div className="callout callout--warn" data-testid="signature-warning">
        <span className="callout__icon" aria-hidden="true">!</span>
        <div className="callout__body">
          <span className="callout__title">Visual signature only</span>
          This is not a certificate-based digital signature. Export embeds the visible signature or stamp annotation, not a cryptographic signature.
        </div>
      </div>

      {error && <div className="callout callout--danger"><span className="callout__icon" aria-hidden="true">!</span><div className="callout__body">{error}</div></div>}
      {notice && <div className="callout callout--success"><span className="callout__icon" aria-hidden="true">i</span><div className="callout__body">{notice}</div></div>}

      {/* ─── Stamps section (25C) ─── */}
      <section className="inspector-card">
        <p className="inspector-card__title">Stamp presets</p>
        <div className="stamp-preset-grid" data-testid="stamp-presets">
          {STAMP_PRESETS.map((p) => (
            <button
              key={p.id}
              type="button"
              className={`stamp-preset-card ${draft.preset.id === p.id ? "is-active" : ""}`}
              onClick={() => setDraft({ ...draft, preset: p })}
              style={{ borderColor: p.color }}
            >
              <span style={{ color: p.color }}>{p.label}</span>
            </button>
          ))}
        </div>
        <div style={{ display: "grid", gap: 4, marginTop: 4, fontSize: 12 }}>
          <label>
            Preset{" "}
            <select
              value={draft.preset.id}
              onChange={(e) => setDraft({ ...draft, preset: STAMP_PRESETS.find((p) => p.id === e.target.value) ?? STAMP_PRESETS[0] })}
              data-testid="stamp-preset"
            >
              {STAMP_PRESETS.map((p) => (
                <option key={p.id} value={p.id}>{p.label}</option>
              ))}
            </select>
          </label>
          {draft.preset.id === "CUSTOM" && (
            <label>
              Custom text{" "}
              <input
                type="text"
                value={draft.customText}
                onChange={(e) => setDraft({ ...draft, customText: e.target.value })}
                className="compact-input"
                style={{ width: 220 }}
                data-testid="stamp-custom-text"
              />
            </label>
          )}
          <label>
            <input
              type="checkbox"
              checked={draft.includeDate}
              onChange={(e) => setDraft({ ...draft, includeDate: e.target.checked })}
              data-testid="stamp-include-date"
            /> Include today's date
          </label>
          <label>
            Author{" "}
            <input
              type="text"
              value={draft.author}
              onChange={(e) => setDraft({ ...draft, author: e.target.value })}
              className="compact-input"
              style={{ width: 180 }}
              placeholder="Reviewer name (optional)"
              data-testid="stamp-author"
            />
          </label>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, auto)", gap: 4 }}>
            <label>x <input type="number" value={draft.x} onChange={(e) => setDraft({ ...draft, x: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
            <label>y <input type="number" value={draft.y} onChange={(e) => setDraft({ ...draft, y: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
            <label>w <input type="number" value={draft.width} onChange={(e) => setDraft({ ...draft, width: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
            <label>h <input type="number" value={draft.height} onChange={(e) => setDraft({ ...draft, height: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
          </div>
          <button className="btn btn--primary btn--sm" onClick={handleAddStamp} data-testid="stamp-add">
            Add Stamp to Page {activePageIndex + 1}
          </button>
        </div>
      </section>

      {/* ─── Signature image section (25B) ─── */}
      <section className="inspector-card">
        <p className="inspector-card__title">Visual signature image</p>
        <p className="empty-state__hint" style={{ marginTop: 0 }}>
          Add a signature image as a visible PDF overlay. It will be included when you export the edited PDF.
        </p>
        <div style={{ display: "flex", flexWrap: "wrap", gap: 6, marginTop: 4 }}>
          <button className="btn btn--secondary btn--sm" onClick={() => void handlePickImage()} data-testid="signature-pick">
            Choose Image…
          </button>
          {signatureImage && (
            <button className="btn btn--primary btn--sm" onClick={handlePlaceSignature} data-testid="signature-place">
              Place on Page {activePageIndex + 1}
            </button>
          )}
          {signatureImage && (
            <button
              className="ghost-btn"
              onClick={() => { setSignatureImage(null); setSignatureLabel(""); }}
              data-testid="signature-clear"
            >
              Clear
            </button>
          )}
        </div>
        <input
          ref={fileInputRef}
          type="file"
          accept="image/*"
          style={{ display: "none" }}
          onChange={handleBrowserFile}
          data-testid="signature-file-input"
        />
        {signatureImage && (
          <>
            <div className="signature-preview-card" style={{ marginTop: 6 }} data-testid="signature-preview-card">
              <img
                src={signatureImage}
                alt={signatureLabel || "signature preview"}
                style={{
                  maxWidth: "100%",
                  maxHeight: 80,
                  border: "1px solid var(--border, #444)",
                  borderRadius: 2,
                  // Preview mirrors the export decision: preserveAspect → contain
                  // (letterbox), otherwise stretch.
                  objectFit: preserveAspect ? "contain" : "fill",
                }}
                data-testid="signature-preview-image"
              />
              <p style={{ fontSize: 11, color: "var(--muted, #888)" }}>
                {signatureLabel}
                {imageNaturalSize && (
                  <span style={{ marginLeft: 6 }}>
                    ({imageNaturalSize.w}×{imageNaturalSize.h})
                  </span>
                )}
              </p>
            </div>
            {/* Phase 27A — preserve-aspect checkbox. */}
            <label style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, marginTop: 4 }}>
              <input
                type="checkbox"
                checked={preserveAspect}
                onChange={(e) => setPreserveAspect(e.target.checked)}
                data-testid="signature-preserve-aspect"
              />
              Preserve image aspect ratio
            </label>
            <div style={{ display: "grid", gridTemplateColumns: "repeat(4, auto)", gap: 4, fontSize: 12, marginTop: 4 }}>
              <label>x <input type="number" value={sigRect.x} onChange={(e) => setSigRect({ ...sigRect, x: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
              <label>y <input type="number" value={sigRect.y} onChange={(e) => setSigRect({ ...sigRect, y: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
              <label>w <input type="number" value={sigRect.width} onChange={(e) => setSigRect({ ...sigRect, width: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
              <label>h <input type="number" value={sigRect.height} onChange={(e) => setSigRect({ ...sigRect, height: parseFloat(e.target.value) || 0 })} style={{ width: 60 }} /></label>
            </div>
            {/* Phase 27A — show the fitted rect that will be drawn when preserve-aspect is on. */}
            {preserveAspect && imageNaturalSize && (() => {
              const fr = fittedSigRect();
              if (Math.abs(fr.width - sigRect.width) < 0.5 && Math.abs(fr.height - sigRect.height) < 0.5) {
                return null;
              }
              return (
                <p style={{ fontSize: 11, color: "var(--muted, #888)", marginTop: 4 }} data-testid="signature-fitted-rect">
                  Fitted draw rect: x {fr.x.toFixed(1)}, y {fr.y.toFixed(1)},
                  w {fr.width.toFixed(1)}, h {fr.height.toFixed(1)}
                </p>
              );
            })()}
          </>
        )}
      </section>
    </div>
  );
};
