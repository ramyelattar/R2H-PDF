import { useCallback, useEffect, useState } from "react";
import {
  pdfGetPageContentObjects,
  pdfApplyNativeTextEdit,
  pdfCoverPathObject,
  pdfCropNativeImage,
  pdfDeleteNativeImage,
  pdfMoveNativeImage,
  pdfApplyFindReplace,
  pdfPreviewFindReplace,
  pdfRotateNativeImage,
  pdfListContentEdits,
  pdfRevertContentEdit,
  pdfPrepareTextBlockEdit,
  pdfApplyTextBlockEdit,
  type ContentObject,
  type ContentEditRecord,
  type NativeTextEditResult,
  type NativeImageEditResult,
  type TextBlock,
  type TextBlockEditResult,
  type BlockEditStrategy,
  type EditMethodValue,
  type FindReplaceMatchPreview,
  type FindReplaceScope,
} from "../../lib/ipc";
import type { EditorObject, ShapeObject } from "../pdf-editor/types";
import { useTextEditStrategy } from "./useTextEditStrategy";
import { InlineTextEditor, type InlineMethodPreview } from "./InlineTextEditor";
import { resolveTextLabel, truncate } from "./textLabel";
import { computeInsetCropRect, type ImageLayoutMode, type StagedImageReplacement } from "./imageLayout";

function readImageDimensions(dataUrl: string, fallback: { width: number; height: number }): Promise<{ width: number; height: number }> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const timer = window.setTimeout(() => resolve(fallback), 200);
    img.onload = () => {
      window.clearTimeout(timer);
      resolve({ width: img.naturalWidth || img.width || 1, height: img.naturalHeight || img.height || 1 });
    };
    img.onerror = () => {
      window.clearTimeout(timer);
      reject(new Error("Could not decode replacement image dimensions."));
    };
    img.src = dataUrl;
  });
}

interface ContentEditPanelProps {
  sessionId: string;
  currentPageIndex: number;
  onContentEdited?: () => void;
  /** Phase 26D — sink for path-cover overlay objects. */
  onAddOverlay?: (object: EditorObject) => void;
  /** Phase 28F — fired after any text edit so the parent can invalidate
   *  search/RAG indices. */
  onTextEdited?: (info: { method: EditMethodValue; pageIndex: number }) => void;
  /** Phase 30C — experimental ToUnicode native edit feature flag. */
  enableExperimentalToUnicode?: boolean;
  /** Phase 30C — toggle handler for the experimental flag. */
  onToggleExperimentalToUnicode?: (next: boolean) => void;
  /** Phase 30E — promote a block edit to the canvas overlay. */
  onRequestCanvasBlockEdit?: (block: TextBlock) => void;
  onStageImageReplace?: (draft: StagedImageReplacement) => void;
  stagedImageReplace?: StagedImageReplacement | null;
  onUpdateStagedImageReplace?: (patch: Partial<Pick<StagedImageReplacement, "layoutMode" | "preserveAspect">>) => void;
  onApplyStagedImageReplace?: () => void | Promise<void>;
  onCancelStagedImageReplace?: () => void;
}

export const ContentEditPanel = ({
  sessionId,
  currentPageIndex,
  onContentEdited,
  onAddOverlay,
  onTextEdited,
  enableExperimentalToUnicode,
  onToggleExperimentalToUnicode,
  onRequestCanvasBlockEdit,
  onStageImageReplace,
  stagedImageReplace,
  onUpdateStagedImageReplace,
  onApplyStagedImageReplace,
  onCancelStagedImageReplace,
}: ContentEditPanelProps) => {
  const [objects, setObjects] = useState<ContentObject[]>([]);
  const [selectedObject, setSelectedObject] = useState<ContentObject | null>(null);
  const [editText, setEditText] = useState("");
  const [editResult, setEditResult] = useState<NativeTextEditResult | null>(null);
  const [imageEditResult, setImageEditResult] = useState<NativeImageEditResult | null>(null);
  const [history, setHistory] = useState<ContentEditRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Phase 27E — Path cover styling controls.
  const [coverColor, setCoverColor] = useState<string>("#ffffff");
  const [coverOpacity, setCoverOpacity] = useState<number>(1.0);
  // Phase 28D/E — block editing state.
  const [blocks, setBlocks] = useState<TextBlock[]>([]);
  const [selectedBlock, setSelectedBlock] = useState<TextBlock | null>(null);
  const [blockEditText, setBlockEditText] = useState<string>("");
  const [blockEditStrategy, setBlockEditStrategy] = useState<BlockEditStrategy>("auto");
  const [blockEditResult, setBlockEditResult] = useState<TextBlockEditResult | null>(null);
  // Phase 29E/29H — inline text editor + diagnostics.
  const [inlineEditorOpen, setInlineEditorOpen] = useState(false);
  const [imageRect, setImageRect] = useState<[number, number, number, number] | null>(null);
  const [imageFitMode, setImageFitMode] = useState<ImageLayoutMode>("fit");
  const [imagePreserveAspect, setImagePreserveAspect] = useState(true);
  const [cropInsetPct, setCropInsetPct] = useState(10);
  const [findText, setFindText] = useState("");
  const [replaceText, setReplaceText] = useState("");
  const [findScope, setFindScope] = useState<FindReplaceScope>("current_page");
  const [findCaseSensitive, setFindCaseSensitive] = useState(false);
  const [findWholeWord, setFindWholeWord] = useState(false);
  const [findMatches, setFindMatches] = useState<FindReplaceMatchPreview[]>([]);
  const [selectedFindMatchId, setSelectedFindMatchId] = useState<string | null>(null);
  const strategyHook = useTextEditStrategy(sessionId, currentPageIndex);

  const loadObjects = useCallback(async () => {
    setLoading(true);
    setError(null);
    const [objsRes, blocksRes] = await Promise.all([
      pdfGetPageContentObjects(sessionId, currentPageIndex),
      pdfPrepareTextBlockEdit(sessionId, currentPageIndex),
    ]);
    setLoading(false);
    if (objsRes.ok) {
      setObjects(objsRes.data);
      setSelectedObject(null);
      setEditResult(null);
    } else {
      setError(objsRes.error.message);
    }
    if (blocksRes.ok) {
      setBlocks(blocksRes.data);
      setSelectedBlock(null);
      setBlockEditResult(null);
    }
  }, [sessionId, currentPageIndex]);

  const loadHistory = useCallback(async () => {
    const result = await pdfListContentEdits(sessionId);
    if (result.ok) setHistory(result.data);
  }, [sessionId]);

  useEffect(() => { void loadObjects(); }, [loadObjects]);
  useEffect(() => { void loadHistory(); }, [loadHistory]);

  const handleSelectObject = (obj: ContentObject) => {
    setSelectedObject(obj);
    setEditText(obj.text_info?.decoded_text ?? "");
    setEditResult(null);
    setImageEditResult(null);
    setImageRect(obj.object_type === "image_xobject" ? obj.bbox : null);
    setCropInsetPct(10);
  };

  const updateImageFitMode = (mode: ImageLayoutMode) => {
    const nextPreserveAspect = mode !== "stretch";
    setImageFitMode(mode);
    setImagePreserveAspect(nextPreserveAspect);
    if (selectedObject?.object_type === "image_xobject" && stagedImageReplace?.contentObjectId === selectedObject.id) {
      onUpdateStagedImageReplace?.({ layoutMode: mode, preserveAspect: nextPreserveAspect });
    }
  };

  const updateImagePreserveAspect = (preserveAspect: boolean) => {
    setImagePreserveAspect(preserveAspect);
    if (preserveAspect && imageFitMode === "stretch") {
      updateImageFitMode("fit");
    }
    if (selectedObject?.object_type === "image_xobject" && stagedImageReplace?.contentObjectId === selectedObject.id) {
      onUpdateStagedImageReplace?.({ preserveAspect, layoutMode: preserveAspect && imageFitMode === "stretch" ? "fit" : imageFitMode });
    }
  };

  const handleApplyTextEdit = async () => {
    if (!selectedObject || !selectedObject.text_info) return;
    setLoading(true);
    setError(null);
    const result = await pdfApplyNativeTextEdit({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
      replacement_text: editText,
      preserve_style: true,
    });
    setLoading(false);
    if (result.ok) {
      setEditResult(result.data);
      if (result.data.success) {
        void loadObjects(); // Refresh
        void loadHistory();
        onContentEdited?.(); // Trigger canvas re-render
        // Phase 28F — notify parent so caches/indexes can be invalidated.
        onTextEdited?.({ method: result.data.method, pageIndex: currentPageIndex });
      }
    } else {
      setError(result.error.message);
    }
  };

  // Phase 28D/E — block edit handlers.
  const handleSelectBlock = (block: TextBlock) => {
    setSelectedBlock(block);
    setBlockEditText(block.combined_text);
    setBlockEditResult(null);
  };

  const handleApplyBlockEdit = async () => {
    if (!selectedBlock) return;
    setLoading(true);
    setError(null);
    const result = await pdfApplyTextBlockEdit({
      session_id: sessionId,
      page_index: currentPageIndex,
      block_id: selectedBlock.block_id,
      replacement_text: blockEditText,
      strategy: blockEditStrategy,
      overflow_policy: "reject",
    });
    setLoading(false);
    if (result.ok) {
      setBlockEditResult(result.data);
      if (result.data.success) {
        void loadObjects();
        void loadHistory();
        onContentEdited?.();
        onTextEdited?.({ method: result.data.method, pageIndex: currentPageIndex });
      }
    } else {
      setError(result.error.message);
    }
  };

  const handleRevert = async (editId: string) => {
    setLoading(true);
    setError(null);
    const result = await pdfRevertContentEdit(sessionId, editId);
    setLoading(false);
    if (result.ok) {
      void loadObjects();
      void loadHistory();
      onContentEdited?.();
      // Phase 28F/H — a revert is a write; flag the search/RAG cache as
      // stale so the user is prompted to rebuild the index.
      onTextEdited?.({ method: "native_in_place_edit", pageIndex: currentPageIndex });
    } else {
      setError(result.error.message);
    }
  };

  const handleDeleteImage = async () => {
    if (!selectedObject || selectedObject.object_type !== "image_xobject") return;
    if (!window.confirm("Delete this image using a visual cover? This creates a reversible snapshot when available.")) {
      return;
    }
    setLoading(true);
    const result = await pdfDeleteNativeImage({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
    });
    setLoading(false);
    if (result.ok && result.data.success) {
      setImageEditResult(result.data);
      void loadObjects();
      void loadHistory();
      onContentEdited?.(); // Trigger canvas re-render
    } else {
      setError(result.ok ? result.data.warnings.join("; ") : result.error.message);
    }
  };

  const handleMoveImage = async () => {
    if (!selectedObject || selectedObject.object_type !== "image_xobject") return;
    const [x0, y0, x1, y1] = imageRect ?? selectedObject.bbox;
    const x = x0;
    const y = y0;
    const w = x1 - x0;
    const h = y1 - y0;
    if (isNaN(x) || isNaN(y) || isNaN(w) || isNaN(h) || w <= 0 || h <= 0) {
      setError("Invalid dimensions. Width and height must be positive numbers.");
      return;
    }
    setLoading(true);
    setError(null);
    const result = await pdfMoveNativeImage({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
      new_rect: [x, y, x + w, y + h],
    });
    setLoading(false);
    if (result.ok && result.data.success) {
      setImageEditResult(result.data);
      void loadObjects();
      void loadHistory();
      onContentEdited?.();
    } else {
      setError(result.ok ? result.data.warnings.join("; ") : result.error.message);
    }
  };

  const handleReplaceImageFile = async (file: File | null) => {
    if (!file || !selectedObject || selectedObject.object_type !== "image_xobject") return;
    const allowed = ["image/png", "image/jpeg"];
    if (!allowed.includes(file.type)) {
      setError(file.type === "image/webp"
        ? "WebP replacement is disabled: the PDF backend decoder cannot guarantee export-safe WebP embedding on this build. Use PNG or JPG."
        : "Replace Image supports PNG or JPG only.");
      return;
    }
    setLoading(true);
    setError(null);
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () => reject(new Error("Could not read replacement image."));
      reader.onload = () => {
        resolve(String(reader.result ?? ""));
      };
      reader.readAsDataURL(file);
    }).catch((err: Error) => {
      setError(err.message);
      return null;
    });
    if (!dataUrl) {
      setLoading(false);
      return;
    }
    const imageBytesBase64 = dataUrl.includes(",") ? dataUrl.split(",")[1] : dataUrl;
    const fallbackDims = {
      width: selectedObject.image_info?.width ?? 1,
      height: selectedObject.image_info?.height ?? 1,
    };
    const dims = await readImageDimensions(dataUrl, fallbackDims).catch(() => fallbackDims);
    onStageImageReplace?.({
      sessionId,
      pageIndex: currentPageIndex,
      contentObjectId: selectedObject.id,
      bbox: selectedObject.bbox,
      dataUrl,
      imageBytesBase64,
      layoutMode: imageFitMode,
      naturalWidth: dims.width,
      naturalHeight: dims.height,
      preserveAspect: imagePreserveAspect,
    });
    setImageEditResult(null);
    setLoading(false);
  };

  const handleCropImage = async () => {
    if (!selectedObject || selectedObject.object_type !== "image_xobject") return;
    const crop = computeInsetCropRect(selectedObject.bbox, cropInsetPct);
    if (crop[2] <= crop[0] || crop[3] <= crop[1]) {
      setError("Invalid crop: crop rectangle cannot be empty.");
      return;
    }
    setLoading(true);
    setError(null);
    const result = await pdfCropNativeImage({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
      crop_rect: crop,
      target_rect: selectedObject.bbox,
    });
    setLoading(false);
    if (result.ok && result.data.success) {
      setImageEditResult(result.data);
      void loadObjects();
      void loadHistory();
      onContentEdited?.();
    } else {
      setError(result.ok ? result.data.warnings.join("; ") : result.error.message);
    }
  };

  const handleRotateImage = async (degrees: -90 | 90 | 180 | 270) => {
    if (!selectedObject || selectedObject.object_type !== "image_xobject") return;
    setLoading(true);
    setError(null);
    const result = await pdfRotateNativeImage({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
      degrees,
    });
    setLoading(false);
    if (result.ok && result.data.success) {
      setImageEditResult(result.data);
      void loadObjects();
      void loadHistory();
      onContentEdited?.();
    } else {
      setError(result.ok ? result.data.warnings.join("; ") : result.error.message);
    }
  };

  const handleFindReplacePreview = async () => {
    setLoading(true);
    setError(null);
    const result = await pdfPreviewFindReplace({
      session_id: sessionId,
      current_page_index: currentPageIndex,
      find_text: findText,
      replace_text: replaceText,
      scope: findScope,
      case_sensitive: findCaseSensitive,
      whole_word: findWholeWord,
    });
    setLoading(false);
    if (result.ok) {
      setFindMatches(result.data.matches);
      setSelectedFindMatchId(result.data.matches[0]?.id ?? null);
    } else {
      setError(result.error.message);
    }
  };

  const applyFindReplaceMatches = async (matches: FindReplaceMatchPreview[]) => {
    const safe = matches.filter((m) => m.safe);
    if (safe.length === 0) {
      setError("No safely editable find/replace matches selected.");
      return;
    }
    setLoading(true);
    setError(null);
    const result = await pdfApplyFindReplace({
      session_id: sessionId,
      replace_text: replaceText,
      matches: safe.map((m) => ({
        id: m.id,
        page_index: m.page_index,
        content_object_id: m.content_object_id,
      })),
    });
    setLoading(false);
    if (result.ok) {
      void loadObjects();
      void loadHistory();
      onContentEdited?.();
      onTextEdited?.({ method: "safe_visual_replacement", pageIndex: currentPageIndex });
      if (result.data.failed_count > 0 || result.data.skipped_count > 0) {
        setError(result.data.warnings.join("; ") || "Some find/replace matches were skipped.");
      }
    } else {
      setError(result.error.message);
    }
  };

  // Phase 26D — Safe visual removal for path/vector objects.
  const handleCoverPath = async () => {
    if (!selectedObject || selectedObject.object_type !== "path") return;
    setLoading(true);
    setError(null);
    const result = await pdfCoverPathObject({
      session_id: sessionId,
      page_index: currentPageIndex,
      content_object_id: selectedObject.id,
    });
    setLoading(false);
    if (!result.ok) {
      setError(result.error.message);
      return;
    }
    if (!result.data.success || !result.data.spec) {
      setError(result.data.warnings.join("; ") || "Cover Path produced no spec.");
      return;
    }
    if (!onAddOverlay) {
      setError("Overlay sink not wired — cannot apply the cover rectangle.");
      return;
    }
    const spec = result.data.spec;
    const [x0, y0, x1, y1] = spec.bbox;
    const now = Date.now();
    const obj: ShapeObject = {
      id: spec.id,
      sessionId,
      pageIndex: spec.page_index,
      type: "rectangle",
      rect: { x: x0, y: y0, width: Math.max(0, x1 - x0), height: Math.max(0, y1 - y0) },
      rotation: 0,
      zIndex: 10,
      locked: false,
      hidden: false,
      createdAt: now,
      updatedAt: now,
      metadata: {
        source: "path_cover",
        method: spec.method,
        method_label: result.data.method_label,
        // Phase 27E — record chosen styling so the audit log and export
        // pipeline can reflect the actual cover color/opacity.
        cover_color: coverColor,
        cover_opacity: coverOpacity,
      },
      strokeColor: "transparent",
      strokeWidth: 0,
      fillColor: coverColor,
    };
    // ShapeObject doesn't define opacity directly; we tag metadata.opacity
    // so the editor can reflect it and the export pipeline can honour it.
    (obj.metadata as Record<string, unknown>).opacity = coverOpacity;
    onAddOverlay(obj);
    void loadHistory();
    void loadObjects();
  };

  const textObjects = objects.filter(o => o.object_type === "text_span" || o.object_type === "text_block");
  const imageObjects = objects.filter(o => o.object_type === "image_xobject");

  const formatMethod = (method: string) => {
    switch (method) {
      case "safe_visual_replacement": return "Visual replacement";
      case "native_in_place_edit": return "Native text edit";
      case "native_multi_operator": return "Native block edit";
      case "native_xobject_swap": return "Replace image";
      case "visual_patch_fallback": return "Visual overlay";
      case "rejected": return "Not available for this object";
      default: return method;
    }
  };

  /** Phase 28E — colored method badge that shows what actually happened.
   *  Labels are product-grade copy; technical strategy names live in the
   *  diagnostics expander for advanced users. */
  const methodBadge = (method: string) => {
    const map: Record<string, { label: string; cls: string; title: string }> = {
      native_in_place_edit: { label: "Native edit", cls: "badge-info", title: "Edits the original glyphs in place — original font preserved." },
      native_multi_operator: { label: "Native edit (block)", cls: "badge-info", title: "Each line in the block is rewritten with the original font." },
      safe_visual_replacement: { label: "Visual replacement", cls: "badge-warning", title: "Redacts the original glyphs and redraws the replacement in Helvetica." },
      visual_patch_fallback: { label: "Visual overlay", cls: "badge-warning", title: "Overlay only — the underlying PDF stream is not modified." },
      native_xobject_swap: { label: "Image replace", cls: "badge-info", title: "Swaps the image resource referenced by the page." },
      rejected: { label: "Read-only", cls: "badge-major", title: "This edit cannot be applied safely." },
      cannot_safely_edit: { label: "Cannot safely edit this text directly", cls: "badge-major", title: "Text encoding is unreliable — native edit is refused to avoid corrupting the page." },
    };
    const m = map[method] ?? { label: method, cls: "badge", title: method };
    return <span className={`badge ${m.cls}`} title={m.title} data-testid={`method-badge-${method}`}>{m.label}</span>;
  };

  const levelBadge = (level: string) => {
    switch (level) {
      case "native_editable": return <span className="badge badge-info">Native editable</span>;
      case "native_replaceable": return <span className="badge badge-info">Replaceable</span>;
      case "visual_patch_only": return <span className="badge badge-warning">Visual replacement</span>;
      case "read_only": return <span className="badge badge-major">Read only</span>;
      default: return <span className="badge">{level}</span>;
    }
  };

  return (
    <div className="content-edit-panel" data-testid="content-edit-panel">
      <div
        className="callout callout--info"
        role="note"
        data-testid="content-edit-instructions"
      >
        <span className="callout__icon" aria-hidden="true">✎</span>
        <div className="callout__body">
          <span className="callout__title">Edit directly on the page</span>
          Double-click any text in the PDF to edit it in place — that's
          the primary editing path. Native edits preserve the original
          font; otherwise the editor falls back to a safe visual
          replacement (and tells you which it used). The advanced
          object list below is a fallback for cases where on-page
          selection doesn't work — pick a row to inspect the same
          objects the page shows.
        </div>
      </div>
      {/* Phase 30C — experimental ToUnicode native edit toggle. */}
      {onToggleExperimentalToUnicode && (
        <label
          style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, marginBottom: 6 }}
          data-testid="experimental-tounicode-toggle"
        >
          <input
            type="checkbox"
            checked={!!enableExperimentalToUnicode}
            onChange={(e) => onToggleExperimentalToUnicode?.(e.target.checked)}
          />
          <span>
            Experimental: enable ToUnicode-based native edit for simple CID fonts.
            <span className="text-warn" style={{ marginLeft: 4 }}>
              May fall back to visual when reverse mapping is ambiguous.
            </span>
          </span>
        </label>
      )}

      {loading && <div className="loading-bar" />}
      {error && <p className="text-error">{error}</p>}

      <details className="content-edit-find-replace" open data-testid="find-replace-panel">
        <summary>Find / Replace with review</summary>
        <div className="content-edit-move-fields">
          <label>
            Find text
            <input value={findText} onChange={(e) => setFindText(e.target.value)} data-testid="find-replace-find" />
          </label>
          <label>
            Replace text
            <input value={replaceText} onChange={(e) => setReplaceText(e.target.value)} data-testid="find-replace-replace" />
          </label>
          <label>
            Scope
            <select value={findScope} onChange={(e) => setFindScope(e.target.value as FindReplaceScope)} data-testid="find-replace-scope">
              <option value="current_page">Current page</option>
              <option value="whole_document">Whole document</option>
            </select>
          </label>
          <label className="search-option-toggle">
            <input type="checkbox" checked={findCaseSensitive} onChange={(e) => setFindCaseSensitive(e.target.checked)} data-testid="find-replace-case" />
            Case sensitive
          </label>
          <label className="search-option-toggle">
            <input type="checkbox" checked={findWholeWord} onChange={(e) => setFindWholeWord(e.target.checked)} data-testid="find-replace-whole-word" />
            Whole word
          </label>
          <button className="ghost-btn" onClick={() => void handleFindReplacePreview()} disabled={loading || !findText} data-testid="find-replace-preview">
            Preview
          </button>
        </div>
        {findMatches.length > 0 && (
          <>
            <div style={{ display: "flex", gap: 4, flexWrap: "wrap", marginTop: 6 }}>
              <button
                className="ghost-btn"
                onClick={() => void applyFindReplaceMatches(findMatches.filter((m) => m.id === selectedFindMatchId))}
                disabled={loading || !selectedFindMatchId}
                data-testid="find-replace-selected"
              >
                Replace selected
              </button>
              <button
                className="ghost-btn"
                onClick={() => void applyFindReplaceMatches(findMatches.filter((m) => m.checked))}
                disabled={loading}
                data-testid="find-replace-checked"
              >
                Replace checked
              </button>
              <button
                className="ghost-btn"
                onClick={() => void applyFindReplaceMatches(findMatches.filter((m) => m.safe))}
                disabled={loading}
                data-testid="find-replace-all-safe"
              >
                Replace all safe
              </button>
              <span className="text-muted" style={{ fontSize: 11 }}>
                Unsafe matches are shown and skipped; no silent partial success.
              </span>
            </div>
            <ul className="content-edit-list" data-testid="find-replace-results">
              {findMatches.map((m) => (
                <li key={m.id} className={`content-edit-item ${selectedFindMatchId === m.id ? "content-edit-item--selected" : ""}`}>
                  <button type="button" onClick={() => setSelectedFindMatchId(m.id)} className="content-edit-row-button">
                    <span className="content-edit-row-main">
                      <input
                        type="checkbox"
                        checked={m.checked}
                        disabled={!m.safe}
                        onClick={(e) => e.stopPropagation()}
                        onChange={(e) => {
                          const checked = e.target.checked;
                          setFindMatches((prev) => prev.map((item) => item.id === m.id ? { ...item, checked } : item));
                        }}
                        aria-label={`Check match on page ${m.page_index + 1}`}
                      />
                      <span className="content-edit-text-label">p.{m.page_index + 1}: {truncate(m.text_preview, 80)}</span>
                    </span>
                    <span className="content-edit-row-meta">
                      <span>{m.replacement_method}</span>
                      <span>{m.safe ? "Safe" : `Skipped: ${m.reason ?? "Not safely editable"}`}</span>
                      <span>bbox [{m.bbox.map((v) => v.toFixed(1)).join(", ")}]</span>
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}
      </details>

      {objects.length === 0 && !loading && (
        <p className="empty-text">
          No editable content objects found on this page.
          {currentPageIndex >= 0 && " This page may be scanned (image-only). Try running OCR."}
        </p>
      )}

      {textObjects.length > 0 && (
        <details data-testid="content-edit-text-objects-details">
          <summary>
            Advanced — text object list ({textObjects.length}){" "}
            <span className="text-muted" style={{ fontSize: 11 }}>
              (fallback)
            </span>
          </summary>
          <p className="content-edit-note">
            Prefer double-clicking text directly on the page. This list is
            a fallback for cases where the on-page hit-test is unreliable
            (rotated pages, very small text, locked text identity).
          </p>
          <ul className="content-edit-list" role="listbox" aria-label="Editable text objects">
            {textObjects.map(obj => {
              const isSelected = selectedObject?.id === obj.id;
              const level = obj.editable_level;
              const isNative = level === "native_editable";
              const isVisual = level === "visual_patch_only";
              // Honest label: refuse to render garbage as the row title.
              const { label, garbled, partial } = resolveTextLabel(obj);
              return (
                <li
                  key={obj.id}
                  role="option"
                  tabIndex={0}
                  aria-selected={isSelected}
                  className={`content-edit-item ${isSelected ? "content-edit-item--selected" : ""}`}
                  onClick={() => handleSelectObject(obj)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      handleSelectObject(obj);
                    }
                  }}
                  onDoubleClick={() => {
                    // Double-click opens the inline editor on editable text.
                    // Read-only and garbled text select but do not open the
                    // editor (the inline component itself also refuses, but
                    // we keep the row click honest here too).
                    handleSelectObject(obj);
                    if (level !== "read_only" && !garbled) {
                      setInlineEditorOpen(true);
                    }
                  }}
                  data-testid={`text-obj-row-${obj.id}`}
                >
                  <span
                    className={`content-edit-item__text ${garbled ? "content-edit-item__text--garbled" : ""}`}
                  >
                    {truncate(label, 60)}
                  </span>
                  {levelBadge(level)}
                  {garbled && (
                    <span
                      className="content-edit-item__hint text-warn"
                      data-testid="text-row-garbled-hint"
                      title="The font's encoding could not be decoded safely; the original characters are unknown. Native edit is refused."
                    >
                      Text encoding could not be decoded safely
                    </span>
                  )}
                  {!garbled && partial && (
                    <span
                      className="content-edit-item__hint text-warn"
                      data-testid="text-row-partial-hint"
                      title="Some glyphs in this text object could not be decoded — labels may be imprecise."
                    >
                      Partial decode
                    </span>
                  )}
                  {!garbled && isNative && (
                    <span className="content-edit-item__hint text-info" data-testid="text-row-native-hint">
                      Native editable
                    </span>
                  )}
                  {!garbled && isVisual && (
                    <span className="content-edit-item__hint text-warn" data-testid="text-row-visual-hint">
                      Safe visual replacement will be used
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        </details>
      )}

      {imageObjects.length > 0 && (
        <details>
          <summary>Image objects ({imageObjects.length})</summary>
          <ul className="content-edit-list">
            {imageObjects.map(obj => (
              <li
                key={obj.id}
                className={`content-edit-item ${selectedObject?.id === obj.id ? "content-edit-item--selected" : ""}`}
                onClick={() => handleSelectObject(obj)}
                data-testid={`image-obj-row-${obj.id}`}
                role="option"
                aria-selected={selectedObject?.id === obj.id}
                tabIndex={0}
              >
                <span className="content-edit-item__text">
                  Image {obj.image_info?.width}×{obj.image_info?.height} ({obj.image_info?.color_space})
                </span>
                {levelBadge(obj.editable_level)}
              </li>
            ))}
          </ul>
        </details>
      )}

      {/* Selected object inspector */}
      {selectedObject && (
        <div className="content-edit-inspector">
          <h5>Selected: {formatPdfObjectType(selectedObject.object_type)}</h5>
          <div className="inspector-grid">
            <span>Level</span><strong>{selectedObject.editable_level.replace("_", " ")}</strong>
            <span>Position</span><strong>[{selectedObject.bbox.map(v => v.toFixed(0)).join(", ")}]</strong>
            {selectedObject.text_info && (
              <>
                <span>Font</span><strong>{selectedObject.text_info.font_name} {selectedObject.text_info.font_size}pt</strong>
                <span>Glyphs</span><strong>{selectedObject.text_info.glyph_count}</strong>
              </>
            )}
            {selectedObject.image_info && (
              <>
                <span>Size</span><strong>{selectedObject.image_info.width}×{selectedObject.image_info.height}</strong>
                <span>Color</span><strong>{selectedObject.image_info.color_space}</strong>
                <span>BBox</span><strong>{selectedObject.bbox.map(v => v.toFixed(1)).join(", ")}</strong>
                <span>Editable method</span><strong>Visual image replacement</strong>
                <span>XObject</span><strong>{selectedObject.image_info.xobject_name || "(not exposed)"}</strong>
                <span>Transform</span><strong>{selectedObject.image_info.transform_matrix.map(v => v.toFixed(2)).join(", ")}</strong>
              </>
            )}
          </div>

          {selectedObject.diagnostics.length > 0 && (
            <div className="content-edit-diagnostics">
              {selectedObject.diagnostics.map((d, i) => <p key={i} className="text-warn">⚠ {d}</p>)}
            </div>
          )}

          {/* Phase 29H — text-editing diagnostics panel for selected text. */}
          {selectedObject.text_info && (() => {
            const fi = strategyHook.fontInfoFor(selectedObject);
            const ti = selectedObject.text_info;
            return (
              <details className="content-edit-diagnostics-panel" data-testid="text-edit-diagnostics" open>
                <summary>Text edit diagnostics</summary>
                <div className="inspector-grid" style={{ fontSize: 11 }}>
                  <span>Font resource</span><strong>{ti.font_name}</strong>
                  <span>Base font</span><strong>{fi?.base_font_name ?? "(unknown)"}</strong>
                  <span>Subtype</span><strong>{fi?.subtype ?? "(unknown)"}</strong>
                  <span>Encoding</span><strong>{fi?.encoding_kind ?? ti.font_encoding ?? "(unknown)"}</strong>
                  <span>Subset font</span><strong>{fi?.is_subset ? "yes" : "no"}</strong>
                  <span>ToUnicode</span><strong>{fi?.has_to_unicode ? "yes" : "no"}</strong>
                  <span>Differences</span><strong>{(fi?.differences_count ?? 0) > 0 ? `yes (${fi?.differences_count})` : "no"}</strong>
                  <span>Operator type</span><strong>{ti.operator_type ?? "(unknown)"}</strong>
                  <span>Stream index</span><strong>{ti.content_stream_index ?? "(unknown)"}</strong>
                  <span>Operator index</span><strong>{ti.operator_index ?? "(unknown)"}</strong>
                  <span>Occurrence index</span><strong>{ti.occurrence_index}</strong>
                  <span>Native eligibility</span><strong>{ti.editable_strategy ?? "(unknown)"}</strong>
                </div>
                {(ti.unsupported_reason && ti.unsupported_reason.length > 0) && (
                  <ul style={{ fontSize: 11, marginTop: 4, paddingLeft: 14, color: "#a40" }}>
                    {ti.unsupported_reason.map((r, i) => <li key={i}>{r}</li>)}
                  </ul>
                )}
                {fi?.unsupported_reasons && fi.unsupported_reasons.length > 0 && (
                  <ul style={{ fontSize: 11, marginTop: 4, paddingLeft: 14, color: "#a40" }}>
                    {fi.unsupported_reasons.map((r, i) => <li key={`f${i}`}>{r}</li>)}
                  </ul>
                )}
              </details>
            );
          })()}

          {/* Text edit action */}
          {selectedObject.text_info && selectedObject.editable_level === "native_editable" && (
            <div className="content-edit-action" data-testid="content-edit-text-action">
              <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
                <span style={{ fontSize: 11, color: "var(--muted, #888)" }}>Method:</span>
                {(() => {
                  const preview = strategyHook.resolve(selectedObject, editText);
                  if (preview.strategy === "native_in_place") return methodBadge("native_in_place_edit");
                  if (preview.strategy === "safe_visual_replacement") return methodBadge("safe_visual_replacement");
                  return methodBadge("rejected");
                })()}
                {selectedObject.text_info.font_encoding && (
                  <span className="badge" style={{ fontSize: 10 }} title="Detected font encoding">
                    {selectedObject.text_info.font_encoding}
                  </span>
                )}
                {selectedObject.text_info.is_subset_font && (
                  <span className="badge badge-warning" style={{ fontSize: 10 }}>Subset font</span>
                )}
                {/* Phase 31D — Differences badge when the font uses /Differences. */}
                {(() => {
                  const fi = strategyHook.fontInfoFor(selectedObject);
                  if (fi && (fi.differences_count > 0 || fi.encoding_kind === "custom_differences")) {
                    return (
                      <span
                        className="badge badge-info"
                        style={{ fontSize: 10 }}
                        title="This font's encoding uses a custom /Differences table; native edit emits exact byte codes."
                        data-testid="differences-encoding-badge"
                      >
                        Custom Differences encoding
                      </span>
                    );
                  }
                  return null;
                })()}
                <button
                  className="ghost-btn"
                  onClick={() => setInlineEditorOpen(true)}
                  style={{ fontSize: 11, padding: "1px 6px" }}
                  data-testid="open-inline-editor"
                >
                  Edit Inline
                </button>
                <button
                  className="ghost-btn"
                  onClick={() => {
                    // Focus the panel-side textarea so the user knows where
                    // to type. This is the "Edit in Panel" entry point.
                    const ta = document.querySelector<HTMLTextAreaElement>(
                      '[data-testid="text-edit-replacement-input"]',
                    );
                    ta?.focus();
                  }}
                  style={{ fontSize: 11, padding: "1px 6px" }}
                  data-testid="open-panel-editor"
                >
                  Edit in Panel
                </button>
              </div>
              <div className="content-edit-original" data-testid="text-edit-original">
                <span className="content-edit-original__label">Original:</span>{" "}
                <span className="content-edit-original__value">{selectedObject.text_info.decoded_text}</span>
              </div>
              <label>Replacement text:</label>
              <textarea
                value={editText}
                onChange={(e) => setEditText(e.target.value)}
                rows={2}
                data-testid="text-edit-replacement-input"
              />
              <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
                <button
                  className="ghost-btn"
                  onClick={() => void handleApplyTextEdit()}
                  disabled={loading || editText === selectedObject.text_info?.decoded_text}
                  data-testid="text-edit-apply"
                >
                  Apply
                </button>
                <button
                  className="ghost-btn"
                  onClick={() => setEditText(selectedObject.text_info?.decoded_text ?? "")}
                  disabled={loading || editText === selectedObject.text_info?.decoded_text}
                  data-testid="text-edit-cancel"
                >
                  Cancel
                </button>
              </div>
              <p className="content-edit-note">
                Native edit preserves the original font. If the replacement cannot be encoded with the
                original font, the editor falls back to safe visual replacement honestly and shows the
                reason. The method used will be shown in the result.
              </p>
            </div>
          )}

          {selectedObject.text_info && selectedObject.editable_level === "visual_patch_only" && (
            <div className="content-edit-action" data-testid="content-edit-text-action-visual">
              {methodBadge("safe_visual_replacement")}
              <p className="text-warn">
                This text cannot be natively edited (see diagnostics above). Use safe visual
                replacement: the original glyphs are redacted and the new text is drawn with Helvetica.
              </p>
              <label>Replacement text:</label>
              <textarea
                value={editText}
                onChange={(e) => setEditText(e.target.value)}
                rows={2}
                data-testid="text-edit-replacement-input-visual"
              />
              <button
                className="ghost-btn"
                onClick={() => void handleApplyTextEdit()}
                disabled={loading || editText === selectedObject.text_info?.decoded_text}
                data-testid="text-edit-apply-visual"
              >
                Use Safe Visual Replacement
              </button>
            </div>
          )}

          {selectedObject.text_info && selectedObject.editable_level === "read_only" && (
            <div className="content-edit-action" data-testid="content-edit-text-readonly">
              {methodBadge("rejected")}
              <p className="text-warn">
                This text object is read-only — the engine cannot edit it natively or visually.
              </p>
            </div>
          )}

          {/* Phase 26D + 27E — Vector / path actions */}
          {selectedObject.object_type === "path" && (
            <div className="content-edit-action" data-testid="content-edit-path-actions">
              {(() => {
                const [x0, y0, x1, y1] = selectedObject.bbox;
                const w = x1 - x0;
                const h = y1 - y0;
                const bboxValid = w > 0 && h > 0 && Number.isFinite(w) && Number.isFinite(h);
                const presets: { id: string; label: string; value: string }[] = [
                  { id: "white", label: "White", value: "#ffffff" },
                  { id: "black", label: "Black", value: "#000000" },
                  { id: "page-bg", label: "Page background (approx)", value: "#f7f5ef" },
                ];
                return (
                  <>
                    {/* Phase 27E — color preset chips */}
                    <div
                      style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center", marginBottom: 4 }}
                      data-testid="path-cover-color-presets"
                    >
                      <span style={{ fontSize: 11 }}>Cover color:</span>
                      {presets.map((p) => (
                        <button
                          key={p.id}
                          className={`ghost-btn ${coverColor === p.value ? "ghost-btn--active" : ""}`}
                          onClick={() => setCoverColor(p.value)}
                          style={{
                            fontSize: 11,
                            padding: "1px 6px",
                            background: p.value,
                            color: p.value === "#000000" ? "#fff" : "#000",
                            border: coverColor === p.value ? "2px solid #74a2ff" : "1px solid #888",
                          }}
                          data-testid={`path-cover-preset-${p.id}`}
                          title={`${p.label}: ${p.value}`}
                        >
                          {p.label}
                        </button>
                      ))}
                      <label style={{ fontSize: 11 }}>
                        Custom
                        <input
                          type="color"
                          value={coverColor}
                          onChange={(e) => setCoverColor(e.target.value)}
                          style={{ marginLeft: 4, verticalAlign: "middle", width: 28, height: 18, padding: 0, border: 0 }}
                          data-testid="path-cover-color-input"
                        />
                      </label>
                    </div>
                    <div
                      style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4, fontSize: 11 }}
                      data-testid="path-cover-opacity-row"
                    >
                      <span>Opacity:</span>
                      <input
                        type="range"
                        min={0.1}
                        max={1}
                        step={0.05}
                        value={coverOpacity}
                        onChange={(e) => setCoverOpacity(parseFloat(e.target.value) || 1)}
                        data-testid="path-cover-opacity"
                      />
                      <span>{(coverOpacity * 100).toFixed(0)}%</span>
                    </div>
                    <button
                      className="ghost-btn"
                      onClick={() => void handleCoverPath()}
                      disabled={loading || !bboxValid || !onAddOverlay}
                      title={
                        !bboxValid
                          ? "Path bbox is degenerate — cannot cover."
                          : !onAddOverlay
                            ? "Overlay sink not wired."
                            : `Cover the path bbox with ${coverColor} at ${(coverOpacity * 100).toFixed(0)}%.`
                      }
                      data-testid="content-edit-cover-path"
                    >
                      Cover Path (Safe Visual Removal)
                    </button>
                    <p className="content-edit-note">
                      Safe visual cover — not true vector point editing.
                    </p>
                  </>
                );
              })()}
            </div>
          )}

          {/* Image actions */}
          {selectedObject.object_type === "image_xobject" && (
            <div className="content-edit-action" data-testid="content-edit-image-actions">
              {methodBadge("safe_visual_replacement")}
              <p className="content-edit-note">
                Image operations use visual replacement unless the engine can prove a true native
                XObject transform/swap. Preview and export use the same session bytes after Apply.
              </p>
              <label className="ghost-btn" style={{ display: "inline-flex", cursor: loading ? "not-allowed" : "pointer" }}>
                Replace Image
                <input
                  type="file"
                  accept="image/png,image/jpeg"
                  disabled={loading}
                  style={{ display: "none" }}
                  data-testid="image-replace-input"
                  onChange={(e) => void handleReplaceImageFile(e.target.files?.[0] ?? null)}
                />
              </label>
              <p className="content-edit-note">
                PNG/JPG replacements are staged on the canvas first. WebP is disabled until decoder support is export-safe.
              </p>
              {stagedImageReplace?.contentObjectId === selectedObject.id && (
                <div className="callout callout--info" data-testid="image-replace-staged">
                  <div className="callout__body">
                    <span className="callout__title">Replacement staged</span>
                    Preview is visible on the canvas. Choose layout, then Apply or Cancel.
                    <div style={{ display: "flex", gap: 4, marginTop: 6, flexWrap: "wrap" }}>
                      <button
                        className="ghost-btn"
                        onClick={() => {
                          void (async () => {
                            await onApplyStagedImageReplace?.();
                            void loadObjects();
                            void loadHistory();
                            onContentEdited?.();
                          })();
                        }}
                        disabled={loading || !onApplyStagedImageReplace}
                        data-testid="image-replace-apply"
                      >
                        Apply Replacement
                      </button>
                      <button className="ghost-btn" onClick={() => onCancelStagedImageReplace?.()} disabled={loading || !onCancelStagedImageReplace} data-testid="image-replace-cancel">
                        Cancel
                      </button>
                    </div>
                  </div>
                </div>
              )}
              <button className="ghost-btn" onClick={() => void handleDeleteImage()} disabled={loading} data-testid="image-delete-button">
                Delete Image
              </button>
              <details>
                <summary>Edit Image Geometry</summary>
                <div className="content-edit-move-fields">
                  {(() => {
                    const [x0, y0, x1, y1] = imageRect ?? selectedObject.bbox;
                    const w = x1 - x0;
                    const h = y1 - y0;
                    const setRectPart = (patch: Partial<{ x: number; y: number; w: number; h: number }>) => {
                      const nextX = patch.x ?? x0;
                      const nextY = patch.y ?? y0;
                      const nextW = Math.max(1, patch.w ?? w);
                      const nextH = Math.max(1, patch.h ?? h);
                      setImageRect([nextX, nextY, nextX + nextW, nextY + nextH]);
                    };
                    return (
                      <>
                        <label>X <input type="number" value={x0.toFixed(1)} onChange={(e) => setRectPart({ x: parseFloat(e.target.value) || 0 })} data-testid="img-move-x" /></label>
                        <label>Y <input type="number" value={y0.toFixed(1)} onChange={(e) => setRectPart({ y: parseFloat(e.target.value) || 0 })} data-testid="img-move-y" /></label>
                        <label>Width <input type="number" value={w.toFixed(1)} onChange={(e) => setRectPart({ w: parseFloat(e.target.value) || 1 })} data-testid="img-move-w" /></label>
                        <label>Height <input type="number" value={h.toFixed(1)} onChange={(e) => setRectPart({ h: parseFloat(e.target.value) || 1 })} data-testid="img-move-h" /></label>
                        <label>Rotation <input type="number" value={0} disabled title="Custom arbitrary rotation is disabled: only export-safe 90/180 increments are available." data-testid="img-rotation" /></label>
                      </>
                    );
                  })()}
                  <button className="ghost-btn" onClick={() => void handleMoveImage()} disabled={loading} data-testid="image-geometry-apply">
                    Apply Move/Resize
                  </button>
                  <button className="ghost-btn" onClick={() => setImageRect(selectedObject.bbox)} disabled={loading} data-testid="image-geometry-cancel">
                    Cancel
                  </button>
                </div>
                <div style={{ display: "flex", gap: 4, alignItems: "center", marginTop: 4 }}>
                  <span style={{ fontSize: 11 }}>Replace fit:</span>
                  {(["fit", "fill", "stretch"] as const).map((mode) => (
                    <button
                      key={mode}
                      className={`ghost-btn ${imageFitMode === mode ? "ghost-btn--active" : ""}`}
                      onClick={() => updateImageFitMode(mode)}
                      disabled={loading}
                      data-testid={`image-fit-${mode}`}
                      title={`${mode} replacement is export-safe; the backend covers the old bbox and draws the replacement with this layout.`}
                    >
                      {mode[0].toUpperCase() + mode.slice(1)}
                    </button>
                  ))}
                  <span className="text-muted" style={{ fontSize: 11 }}>
                    Layout applies to the next Replace Image operation.
                  </span>
                </div>
                <label className="search-option-toggle" title="Preserve aspect ratio by using Fit when Stretch would distort the staged replacement.">
                  <input
                    type="checkbox"
                    checked={imagePreserveAspect}
                    onChange={(e) => updateImagePreserveAspect(e.target.checked)}
                    data-testid="image-preserve-aspect"
                  />
                  Preserve aspect ratio
                </label>
                <div style={{ display: "flex", gap: 4, alignItems: "center", marginTop: 4, flexWrap: "wrap" }}>
                  <span style={{ fontSize: 11 }}>Rotate:</span>
                  <button className="ghost-btn" onClick={() => void handleRotateImage(-90)} disabled={loading} data-testid="image-rotate-left">-90°</button>
                  <button className="ghost-btn" onClick={() => void handleRotateImage(90)} disabled={loading} data-testid="image-rotate-right">90°</button>
                  <button className="ghost-btn" onClick={() => void handleRotateImage(180)} disabled={loading} data-testid="image-rotate-180">180°</button>
                  <span className="text-muted" style={{ fontSize: 11 }}>
                    Custom angles disabled until preview/export parity is proven.
                  </span>
                </div>
                <p className="content-edit-note">
                  Uses safe visual method: old position is redacted and image is redrawn at the new position.
                </p>
              </details>
              <details>
                <summary>Crop</summary>
                <div className="content-edit-move-fields">
                  <label>
                    Inset %
                    <input
                      type="number"
                      min={0}
                      max={45}
                      value={cropInsetPct}
                      onChange={(e) => setCropInsetPct(Math.max(0, Math.min(45, parseFloat(e.target.value) || 0)))}
                      data-testid="image-crop-inset"
                    />
                  </label>
                  <button className="ghost-btn" onClick={() => void handleCropImage()} disabled={loading} data-testid="image-crop-apply">
                    Apply Crop
                  </button>
                  <span className="text-muted" style={{ fontSize: 11 }}>
                    Visual image crop covers the old bbox and redraws the cropped region into the original bbox.
                  </span>
                </div>
              </details>
              <button className="ghost-btn" disabled title="Adjustments are disabled until preview/export parity is implemented." data-testid="image-adjust-disabled">
                Adjustments
              </button>
            </div>
          )}
        </div>
      )}

      {/* Edit result feedback */}
      {editResult && (
        <div
          className={`content-edit-result ${editResult.success ? "content-edit-result--success" : "content-edit-result--fail"}`}
          data-testid="text-edit-result"
        >
          <strong>{editResult.success ? "✓ Edit applied" : "✗ Edit failed"}</strong>
          <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
            <span style={{ fontSize: 11 }}>Method:</span>
            {methodBadge(editResult.method)}
            <span style={{ fontSize: 11 }}>({formatMethod(editResult.method)})</span>
            {/* Phase 30D — verification status chip. */}
            {editResult.verification && (
              <span
                className={
                  editResult.verification === "passed"
                    ? "badge badge-info"
                    : editResult.verification === "failed"
                      ? "badge badge-major"
                      : "badge"
                }
                data-testid={`verification-${editResult.verification}`}
                title="Post-edit verification status (Phase 30D)"
              >
                verify: {editResult.verification}
              </span>
            )}
          </div>
          {editResult.warnings.map((w, i) => <p key={i} className="text-warn">{w}</p>)}
          {editResult.verification_warnings && editResult.verification_warnings.length > 0 && (
            <ul style={{ fontSize: 11, color: "#a40", paddingLeft: 14 }}>
              {editResult.verification_warnings.map((w, i) => (
                <li key={i}>{w}</li>
              ))}
            </ul>
          )}
        </div>
      )}

      {imageEditResult && (
        <div
          className={`content-edit-result ${imageEditResult.success ? "content-edit-result--success" : "content-edit-result--fail"}`}
          data-testid="image-edit-result"
        >
          <strong>{imageEditResult.success ? "✓ Image edit applied" : "✗ Image edit failed"}</strong>
          <div style={{ display: "flex", alignItems: "center", gap: 6, flexWrap: "wrap" }}>
            <span style={{ fontSize: 11 }}>Method:</span>
            {methodBadge(imageEditResult.method)}
            <span style={{ fontSize: 11 }}>Action: {imageEditResult.action}</span>
          </div>
          {imageEditResult.warnings.map((w, i) => <p key={i} className="text-warn">{w}</p>)}
        </div>
      )}

      {/* Phase 28D/E — block-level edit section */}
      {blocks.length > 0 && (
        <details open data-testid="content-edit-blocks">
          <summary>Text Blocks ({blocks.length})</summary>
          <ul className="content-edit-list">
            {blocks.map(blk => (
              <li
                key={blk.block_id}
                className={`content-edit-item ${selectedBlock?.block_id === blk.block_id ? "content-edit-item--selected" : ""}`}
                onClick={() => handleSelectBlock(blk)}
                data-testid={`block-row-${blk.block_id}`}
              >
                <span className="content-edit-item__text">
                  {blk.combined_text.split("\n")[0].slice(0, 60)}
                  {blk.combined_text.length > 60 ? "…" : ""}
                </span>
                {blk.all_native_editable
                  ? <span className="badge badge-info">Native ready</span>
                  : <span className="badge badge-warning">Visual reflow</span>}
              </li>
            ))}
          </ul>
        </details>
      )}

      {selectedBlock && (
        <div className="content-edit-inspector" data-testid="block-edit-inspector">
          <h5>Block: {selectedBlock.block_id}</h5>
          <div className="inspector-grid">
            <span>Lines</span><strong>{selectedBlock.member_ids.length}</strong>
            <span>Font size</span><strong>{selectedBlock.font_size.toFixed(1)}pt</strong>
            <span>BBox</span><strong>[{selectedBlock.bbox.map(v => v.toFixed(0)).join(", ")}]</strong>
          </div>
          {selectedBlock.diagnostics.map((d, i) => <p key={i} className="text-warn">{d}</p>)}
          <label>Block text:</label>
          <textarea
            value={blockEditText}
            onChange={(e) => setBlockEditText(e.target.value)}
            rows={Math.min(8, selectedBlock.member_ids.length + 1)}
            data-testid="block-edit-text"
          />
          <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
            <span style={{ fontSize: 11 }}>Strategy:</span>
            <select
              value={blockEditStrategy}
              onChange={(e) => setBlockEditStrategy(e.target.value as BlockEditStrategy)}
              data-testid="block-edit-strategy"
            >
              <option value="auto">Auto (engine picks)</option>
              <option value="native_multi_operator">Force native multi-operator</option>
              <option value="visual_reflow">Force safe visual reflow</option>
            </select>
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: 4 }}>
            <button
              className="ghost-btn"
              onClick={() => void handleApplyBlockEdit()}
              disabled={loading || blockEditText === selectedBlock.combined_text}
              data-testid="block-edit-apply"
            >
              Apply Block Edit
            </button>
            <button
              className="ghost-btn"
              onClick={() => setBlockEditText(selectedBlock.combined_text)}
              disabled={loading || blockEditText === selectedBlock.combined_text}
              data-testid="block-edit-cancel"
            >
              Cancel
            </button>
            {onRequestCanvasBlockEdit && (
              <button
                className="ghost-btn"
                onClick={() => onRequestCanvasBlockEdit(selectedBlock)}
                data-testid="block-edit-open-canvas"
                title="Open the inline block editor directly over the PDF page."
              >
                Edit on Canvas
              </button>
            )}
          </div>
          <p className="content-edit-note">
            Block edits preserve original fonts only when every line is encodeable in the original
            font and the line count matches. Otherwise the engine wraps the new text to the block
            bbox with Helvetica — this is NOT Acrobat-level paragraph reflow.
          </p>
        </div>
      )}

      {blockEditResult && (
        <div
          className={`content-edit-result ${blockEditResult.success ? "content-edit-result--success" : "content-edit-result--fail"}`}
          data-testid="block-edit-result"
        >
          <strong>{blockEditResult.success ? "✓ Block edit applied" : "✗ Block edit failed"}</strong>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ fontSize: 11 }}>Method:</span>
            {methodBadge(blockEditResult.method)}
            <span style={{ fontSize: 11 }}>({formatMethod(blockEditResult.method)})</span>
          </div>
          {blockEditResult.warnings.map((w, i) => <p key={i} className="text-warn">{w}</p>)}
        </div>
      )}

      {/* Edit history */}
      {history.length > 0 && (
        <details>
          <summary>Edit History ({history.length})</summary>
          <ul className="content-edit-history">
            {history.map(rec => (
              <li key={rec.edit_id}>
                <span>{rec.edit_type}</span>
                <span>{rec.before_summary} → {rec.after_summary}</span>
                <span className="content-edit-history__method">{formatMethod(rec.method)}</span>
                {rec.reversible && !rec.warnings.includes("REVERTED") && (
                  <button className="ghost-btn" onClick={() => void handleRevert(rec.edit_id)} disabled={loading}>
                    Revert
                  </button>
                )}
                {rec.warnings.includes("REVERTED") && (
                  <span className="badge badge-warning">Reverted</span>
                )}
                {!rec.reversible && !rec.warnings.includes("REVERTED") && (
                  <span className="badge" title="Snapshot expired or not available">No undo</span>
                )}
              </li>
            ))}
          </ul>
        </details>
      )}

      {/* Phase 29E — inline text editor floating panel. */}
      {inlineEditorOpen && selectedObject?.text_info && (() => {
        const preview: InlineMethodPreview = strategyHook.resolve(selectedObject, editText);
        const fontInfo = strategyHook.fontInfoFor(selectedObject);
        // Page height is unknown here in inspector context; we use a
        // safe constant so the editor floats inside the panel. The
        // viewer-overlay version of this would be wired via portal.
        // PDFs almost always have pageHeight >= bbox[3]; use a guess
        // 2 * bbox[3] to keep the editor near the bbox in the panel.
        const pageHeightPts = (selectedObject.bbox[3] + 100) * 2;
        return (
          <div
            data-testid="inline-text-editor-container"
            style={{
              position: "relative",
              border: "1px dashed #888",
              marginTop: 8,
              padding: 4,
              minHeight: 60,
              background: "var(--surface-hi, #f5f5f5)",
            }}
          >
            <p style={{ fontSize: 11, margin: 0 }}>Inline editor (preview):</p>
            <InlineTextEditor
              bbox={selectedObject.bbox}
              pageHeightPts={pageHeightPts}
              zoom={1}
              initialText={selectedObject.text_info.decoded_text}
              fontSize={selectedObject.text_info.font_size}
              methodPreview={preview}
              fontInfo={fontInfo ?? null}
              contentObject={selectedObject}
              onCancel={() => setInlineEditorOpen(false)}
              onApply={async (text) => {
                setEditText(text);
                setInlineEditorOpen(false);
                // Defer to the existing apply path so we use the same
                // backend safety gates.
                setLoading(true);
                setError(null);
                const result = await pdfApplyNativeTextEdit({
                  session_id: sessionId,
                  page_index: currentPageIndex,
                  content_object_id: selectedObject.id,
                  replacement_text: text,
                  preserve_style: true,
                });
                setLoading(false);
                if (result.ok) {
                  setEditResult(result.data);
                  if (result.data.success) {
                    void loadObjects();
                    void loadHistory();
                    onContentEdited?.();
                    onTextEdited?.({ method: result.data.method, pageIndex: currentPageIndex });
                  }
                } else {
                  setError(result.error.message);
                }
              }}
            />
          </div>
        );
      })()}
    </div>
  );
};

function formatPdfObjectType(type: string): string {
  switch (type) {
    case "text_span": return "Text";
    case "text_block": return "Text block";
    case "image_xobject": return "Image";
    case "path": return "Vector path";
    default: return type.replace(/_/g, " ");
  }
}
