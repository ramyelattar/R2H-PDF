import { useState, type PointerEvent } from "react";
import type { WorkstationState } from "../../state/useWorkstationState";
import type { RenderResponse, SearchMatch, ContentObject, FontResourceInfo } from "../../lib/ipc";
import { PdfCanvasViewer } from "../viewer/PdfCanvasViewer";
import { EditableOverlay } from "../../features/pdf-editor/EditableOverlay";
import type { EditorObject, EditorObjectId, EditorObjectPatch, EditorRect, EditorTool } from "../../features/pdf-editor/types";
import { InlineTextEditor, type InlineMethodPreview } from "../../features/content-edit/InlineTextEditor";
import { InlineBlockEditor } from "../../features/content-edit/InlineBlockEditor";
import { projectPdfBboxToScreen } from "../../features/content-edit/canvasHitTest";
import type { TextBlock, BlockEditStrategy, BlockOverflowPolicy } from "../../lib/ipc";
import { computeImageLayout, computeInsetCropRect, type PdfRect, type StagedImageReplacement } from "../../features/content-edit/imageLayout";
import { WelcomeScreen } from "./WelcomeScreen";
import type { InspectorTab } from "./RightInspector";

interface EditorOverlayProps {
  objects: EditorObject[];
  selectedIds: EditorObjectId[];
  activeTool: EditorTool;
  onSelectObject: (id: EditorObjectId) => void;
  onClearSelection: () => void;
  onMoveObject: (id: EditorObjectId, rect: EditorRect) => void;
  onResizeObject: (id: EditorObjectId, rect: EditorRect) => void;
  onCreateObject: (pdfRect: EditorRect) => void;
  onPatchObject?: (id: EditorObjectId, patch: EditorObjectPatch) => void;
}

/** Phase 30A — canvas-overlay inline text editor props. When set, an
 *  InlineTextEditor floats directly over the rendered page at the bbox
 *  of the source content object. */
export interface CanvasInlineEditProps {
  contentObject: ContentObject;
  pageHeightPts: number;
  methodPreview: InlineMethodPreview;
  fontInfo: FontResourceInfo | null;
  onApply: (replacement: string) => void | Promise<void>;
  onCancel: () => void;
}

interface CenterWorkspaceProps {
  activeTab: WorkstationState["tabs"][number] | null;
  leftInset: number;
  rightInset: number;
  onRenderError: (message: string) => void;
  onRenderSuccess?: (render: RenderResponse) => void;
  searchMatches?: SearchMatch[];
  activeMatchIndex?: number;
  onReopenFile?: () => void;
  onOpenBentoPdf?: () => void;
  editor?: EditorOverlayProps;
  renderVersion?: number;
  inlineEdit?: CanvasInlineEditProps | null;
  /** Phase 31B — true when the user is in Edit Content mode but the
   *  current page is rotated; the inline editor is disabled and a
   *  notice is rendered over the canvas pointing the user at the
   *  side-panel editor. */
  rotatedPageNotice?: boolean;
  /** Phase 30E — inline block editor that floats over the block bbox. */
  inlineBlockEdit?: {
    block: TextBlock;
    pageHeightPts: number;
    onApply: (replacement: string, strategy: BlockEditStrategy, overflowPolicy: BlockOverflowPolicy) => void | Promise<void>;
    onCancel: () => void;
  } | null;
  /** Phase 30F — hit-test overlay: when set, render an invisible hit-test
   *  layer over native text spans that fires `onTextSpanDoubleClick` on
   *  double-click. */
  textSpanHitTest?: {
    contentObjects: ContentObject[];
    pageHeightPts: number;
    selectedObjectId?: string | null;
    onTextSpanDoubleClick: (obj: ContentObject) => void;
    onTextSpanSelect?: (obj: ContentObject) => void;
  } | null;
  imageHitTest?: {
    contentObjects: ContentObject[];
    pageHeightPts: number;
    selectedObjectId?: string | null;
    onImageSelect: (obj: ContentObject) => void;
    onImageEditApply: (obj: ContentObject, bbox: [number, number, number, number]) => void | Promise<void>;
    onImageCropApply?: (obj: ContentObject, cropRect: [number, number, number, number]) => void | Promise<void>;
    onImageRotate?: (obj: ContentObject, degrees: -90 | 90 | 180 | 270) => void | Promise<void>;
    onImageReplace?: (obj: ContentObject) => void;
    replacementDraft?: StagedImageReplacement | null;
    onImageReplaceApply?: () => void | Promise<void>;
    onImageReplaceCancel?: () => void;
    onImageDelete?: (obj: ContentObject) => void | Promise<void>;
  } | null;
  /** Phase UX — show the right inspector on a specific tab. The welcome
   *  screen uses this for "Validate Local AI" and for workflow cards that
   *  pre-select a tab after the document opens. */
  onShowInspectorTab?: (tab: InspectorTab) => void;
  /** Phase UX — record which workflow the user picked on the welcome
   *  screen so the parent can switch the inspector after the PDF loads. */
  onPickWorkflow?: (tab: InspectorTab) => void;
}

export const CenterWorkspace = ({
  activeTab,
  leftInset,
  rightInset,
  onRenderError,
  onRenderSuccess,
  searchMatches,
  activeMatchIndex,
  onReopenFile,
  onOpenBentoPdf,
  editor,
  renderVersion,
  inlineEdit,
  inlineBlockEdit,
  textSpanHitTest,
  imageHitTest,
  rotatedPageNotice,
  onShowInspectorTab,
  onPickWorkflow,
}: CenterWorkspaceProps) => {
  const [imageDraft, setImageDraft] = useState<{
    id: string;
    bbox: [number, number, number, number];
    mode: "move" | "resize" | null;
    startClient: [number, number];
    startBbox: [number, number, number, number];
  } | null>(null);
  const [imageCropDraft, setImageCropDraft] = useState<{
    id: string;
    cropRect: PdfRect;
    activeHandle: "tl" | "tr" | "bl" | "br" | null;
    startClient: [number, number];
    startCrop: PdfRect;
  } | null>(null);

  const renderContent = () => {
    if (!activeTab) {
      return (
        <WelcomeScreen
          onOpenFile={onReopenFile ?? (() => {})}
            onOpenBentoPdf={onOpenBentoPdf ?? (() => {})}
          onShowInspectorTab={onShowInspectorTab ?? (() => {})}
          onPickWorkflow={onPickWorkflow}
        />
      );
    }

    if (activeTab.loadState === "loading") {
      return (
        <div className="workspace-state workspace-state--loading workspace-state--flush">
          <h2>Preparing workspace</h2>
          <p>Streaming pages, index, and annotation metadata…</p>
          <div className="loading-bar" />
        </div>
      );
    }

    if (activeTab.loadState === "error") {
      return (
        <div className="workspace-state workspace-state--error workspace-state--flush">
          <h2>Workspace load failed</h2>
          <p>{activeTab.errorMessage ?? "The active tab encountered a loading error."}</p>
          {onReopenFile && (
            <button className="reopen-file-btn" onClick={onReopenFile}>
              Reopen File
            </button>
          )}
        </div>
      );
    }

    switch (activeTab.kind) {
      case "review":
      case "handoff":
      case "export":
        return (
          <div className="workspace-state workspace-state--error workspace-state--flush">
            <h2>Workflow tab is unavailable</h2>
            <p>This legacy shell tab is not connected to a durable production workflow. Open a PDF and use the document inspector for supported actions.</p>
          </div>
        );
      case "pdf":
      default:
      {
        const isBackendSession = activeTab.id.startsWith("doc-session-");
        const zoomFactor = Math.max(0.25, activeTab.zoom / 100);
        return (
          <div className="workspace-canvas workspace-state--flush">
            {isBackendSession ? (
              <div style={{ position: "relative", display: "inline-block" }}>
                <PdfCanvasViewer
                  sessionId={activeTab.id}
                  pageIndex={Math.max(activeTab.page - 1, 0)}
                  zoomPercent={activeTab.zoom}
                  onRenderError={onRenderError}
                  onRenderSuccess={onRenderSuccess}
                  searchMatches={searchMatches}
                  activeMatchIndex={activeMatchIndex}
                  renderVersion={renderVersion}
                />
                {editor && (
                  <EditableOverlay
                    objects={editor.objects}
                    selectedIds={editor.selectedIds}
                    activeTool={editor.activeTool}
                    page={{ widthPts: 612, heightPts: 792 }}
                    view={{ zoom: zoomFactor }}
                    onSelectObject={editor.onSelectObject}
                    onClearSelection={editor.onClearSelection}
                    onMoveObject={editor.onMoveObject}
                    onResizeObject={editor.onResizeObject}
                    onCreateObject={editor.onCreateObject}
                    onPatchObject={editor.onPatchObject}
                  />
                )}
                {/* Phase 31B — rotated page notice (canvas inline edits
                    disabled — side-panel editor remains available). */}
                {rotatedPageNotice && (
                  <div
                    data-testid="rotated-page-notice"
                    style={{
                      position: "absolute",
                      top: 8,
                      right: 8,
                      zIndex: 70,
                      background: "rgba(204,136,0,0.95)",
                      color: "#fff",
                      padding: "4px 8px",
                      borderRadius: 4,
                      fontSize: 11,
                      maxWidth: 260,
                      boxShadow: "0 2px 6px rgba(0,0,0,0.2)",
                    }}
                  >
                    Canvas inline editing is disabled on rotated pages. Use the side-panel editor instead.
                  </div>
                )}
                {/* Acrobat-style canvas hit-test overlay for native text
                    spans. Renders one transparent box per text object in
                    PDF user-space; on hover the box is outlined and the
                    cursor turns into a text-edit caret; single-click
                    selects, double-click opens the inline editor over
                    the text. This is the primary editing path — the
                    side-panel list is just an advanced fallback. */}
                {textSpanHitTest && (
                  <div
                    data-testid="canvas-text-hit-test"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: "100%",
                      pointerEvents: "none",
                      zIndex: 18,
                    }}
                  >
                    {textSpanHitTest.contentObjects
                      .filter((o) => o.object_type === "text_span" || o.object_type === "text_block")
                      .map((o) => {
                        const { left, top, width, height } = projectPdfBboxToScreen(
                          o.bbox,
                          textSpanHitTest.pageHeightPts,
                          zoomFactor,
                        );
                        const isSelected = textSpanHitTest.selectedObjectId === o.id;
                        const isReadOnly = o.editable_level === "read_only";
                        const isVisualOnly = o.editable_level === "visual_patch_only";
                        const isGarbled =
                          o.text_info?.decoding_quality === "garbled";
                        const titleParts: string[] = [];
                        if (isReadOnly) titleParts.push("Read-only text — cannot be edited.");
                        else if (isGarbled) titleParts.push("Text encoding could not be decoded safely. Double-click is disabled.");
                        else if (isVisualOnly) titleParts.push("Double-click to edit (visual replacement — original font not preserved).");
                        else titleParts.push("Double-click to edit text in place.");
                        if (o.text_info?.font_name) titleParts.push(`Font: ${o.text_info.font_name}`);
                        return (
                          <div
                            key={o.id}
                            data-testid={`text-hit-${o.id}`}
                            data-source-id={o.id}
                            data-editable-level={o.editable_level}
                            className={`canvas-text-hit ${isSelected ? "canvas-text-hit--selected" : ""}`}
                            title={titleParts.join("\n")}
                            style={{
                              position: "absolute",
                              left,
                              top,
                              width,
                              height,
                              cursor: isReadOnly || isGarbled ? "not-allowed" : "text",
                              pointerEvents: "auto",
                              // A persistent low-contrast outline is shown on
                              // every editable text in edit-content mode so
                              // the user can see at a glance what's clickable.
                              // The selected box gets a stronger blue outline.
                              outline: isSelected
                                ? "2px solid rgba(116,162,255,0.95)"
                                : isReadOnly || isGarbled
                                  ? "1px dashed rgba(180,80,80,0.35)"
                                  : "1px dashed rgba(116,162,255,0.30)",
                              outlineOffset: 0,
                              background: isSelected
                                ? "rgba(116,162,255,0.14)"
                                : "transparent",
                              transition: "outline-color 80ms linear, background 80ms linear",
                            }}
                            onMouseEnter={(e) => {
                              const el = e.currentTarget as HTMLDivElement;
                              if (!isSelected) {
                                el.style.background = isReadOnly || isGarbled
                                  ? "rgba(180,80,80,0.07)"
                                  : "rgba(116,162,255,0.10)";
                                el.style.outline = isReadOnly || isGarbled
                                  ? "1px solid rgba(180,80,80,0.60)"
                                  : "1px solid rgba(116,162,255,0.70)";
                              }
                            }}
                            onMouseLeave={(e) => {
                              const el = e.currentTarget as HTMLDivElement;
                              if (!isSelected) {
                                el.style.background = "transparent";
                                el.style.outline = isReadOnly || isGarbled
                                  ? "1px dashed rgba(180,80,80,0.35)"
                                  : "1px dashed rgba(116,162,255,0.30)";
                              }
                            }}
                            onClick={(e) => {
                              e.stopPropagation();
                              textSpanHitTest.onTextSpanSelect?.(o);
                            }}
                            onDoubleClick={(e) => {
                              e.stopPropagation();
                              // Read-only / garbled text must not open the
                              // inline editor — the inline editor itself
                              // would also refuse, but we save the user a
                              // click here.
                              if (isReadOnly || isGarbled) {
                                textSpanHitTest.onTextSpanSelect?.(o);
                                return;
                              }
                              textSpanHitTest.onTextSpanDoubleClick(o);
                            }}
                            onKeyDown={(e) => {
                              if (e.key === "Enter" && isSelected && !isReadOnly && !isGarbled) {
                                e.preventDefault();
                                textSpanHitTest.onTextSpanDoubleClick(o);
                              }
                            }}
                            role="button"
                            tabIndex={0}
                            aria-label={
                              o.text_info?.decoded_text && !isGarbled
                                ? `Edit text: ${o.text_info.decoded_text.slice(0, 40)}`
                                : "Text object on page"
                            }
                          >
                            {isSelected && (
                              <div
                                data-testid={`text-object-toolbar-${o.id}`}
                                style={{
                                  position: "absolute",
                                  left: 0,
                                  top: Math.min(-30, height + 6),
                                  display: "flex",
                                  gap: 4,
                                  background: "rgba(20,28,36,0.92)",
                                  color: "#fff",
                                  padding: 3,
                                  borderRadius: 3,
                                  fontSize: 11,
                                  whiteSpace: "nowrap",
                                }}
                                onClick={(e) => e.stopPropagation()}
                                onPointerDown={(e) => e.stopPropagation()}
                              >
                                <button
                                  className="ghost-btn"
                                  data-testid={`text-toolbar-edit-${o.id}`}
                                  disabled={isReadOnly || isGarbled}
                                  title={isReadOnly || isGarbled ? "Text is not safely editable." : "Edit text inline"}
                                  onClick={() => textSpanHitTest.onTextSpanDoubleClick(o)}
                                >
                                  Edit
                                </button>
                                <button
                                  className="ghost-btn"
                                  data-testid={`text-toolbar-reflow-${o.id}`}
                                  disabled={o.object_type !== "text_block"}
                                  title={o.object_type === "text_block" ? "Open block reflow editor" : "Reflow is available for detected text blocks only."}
                                  onClick={() => textSpanHitTest.onTextSpanDoubleClick(o)}
                                >
                                  Reflow
                                </button>
                                <button className="ghost-btn" data-testid={`text-toolbar-find-${o.id}`} title="Use the Find / Replace panel in the right inspector.">
                                  Find & Replace
                                </button>
                                <button className="ghost-btn" disabled title="Select a history entry in the inspector to revert." data-testid={`text-toolbar-revert-${o.id}`}>
                                  Revert
                                </button>
                                <button className="ghost-btn" disabled title="More text actions are not available for this object." data-testid={`text-toolbar-more-${o.id}`}>
                                  More
                                </button>
                              </div>
                            )}
                          </div>
                        );
                      })}
                  </div>
                )}
                {imageHitTest && (
                  <div
                    data-testid="canvas-image-hit-test"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: "100%",
                      pointerEvents: "none",
                      zIndex: 20,
                    }}
                  >
                    {imageHitTest.contentObjects
                      .filter((o) => o.object_type === "image_xobject")
                      .map((o) => {
                        const draft = imageDraft?.id === o.id ? imageDraft.bbox : o.bbox;
                        const cropDraft = imageCropDraft?.id === o.id
                          ? imageCropDraft.cropRect
                          : computeInsetCropRect(o.bbox as PdfRect, 10);
                        const { left, top, width, height } = projectPdfBboxToScreen(
                          draft,
                          imageHitTest.pageHeightPts,
                          zoomFactor,
                        );
                        const cropScreen = projectPdfBboxToScreen(
                          cropDraft,
                          imageHitTest.pageHeightPts,
                          zoomFactor,
                        );
                        const isSelected = imageHitTest.selectedObjectId === o.id;
                        const isCropping = imageCropDraft?.id === o.id;
                        const replacementDraft = imageHitTest.replacementDraft?.contentObjectId === o.id
                          ? imageHitTest.replacementDraft
                          : null;
                        const replacementPreview = replacementDraft
                          ? computeImageLayout(
                            replacementDraft.bbox,
                            replacementDraft.layoutMode,
                            replacementDraft.naturalWidth,
                            replacementDraft.naturalHeight,
                          )
                          : null;
                        const replacementDrawScreen = replacementPreview
                          ? projectPdfBboxToScreen(replacementPreview.drawRect, imageHitTest.pageHeightPts, zoomFactor)
                          : null;
                        const replacementClipScreen = replacementPreview?.clipRect
                          ? projectPdfBboxToScreen(replacementPreview.clipRect, imageHitTest.pageHeightPts, zoomFactor)
                          : null;
                        const startDrag = (
                          e: PointerEvent<HTMLDivElement>,
                          mode: "move" | "resize",
                        ) => {
                          e.stopPropagation();
                          e.currentTarget.setPointerCapture(e.pointerId);
                          imageHitTest.onImageSelect(o);
                          setImageDraft({
                            id: o.id,
                            bbox: draft,
                            mode,
                            startClient: [e.clientX, e.clientY],
                            startBbox: draft,
                          });
                        };
                        const updateDrag = (e: PointerEvent<HTMLDivElement>) => {
                          if (!imageDraft || imageDraft.id !== o.id || !imageDraft.mode) return;
                          const dx = (e.clientX - imageDraft.startClient[0]) / zoomFactor;
                          const dyScreen = (e.clientY - imageDraft.startClient[1]) / zoomFactor;
                          const [x0, y0, x1, y1] = imageDraft.startBbox;
                          const w = x1 - x0;
                          const h = y1 - y0;
                          if (imageDraft.mode === "move") {
                            setImageDraft({ ...imageDraft, bbox: [x0 + dx, y0 - dyScreen, x1 + dx, y1 - dyScreen] });
                            return;
                          }
                          const newW = Math.max(4, w + dx);
                          let newH = Math.max(4, h + dyScreen);
                          if (e.shiftKey) {
                            const ratio = h / Math.max(1, w);
                            newH = newW * ratio;
                          }
                          setImageDraft({ ...imageDraft, bbox: [x0, y1 - newH, x0 + newW, y1] });
                        };
                        const endDrag = (e: PointerEvent<HTMLDivElement>) => {
                          if (imageDraft?.id === o.id) {
                            e.currentTarget.releasePointerCapture(e.pointerId);
                            setImageDraft({ ...imageDraft, mode: null });
                          }
                        };
                        const startCropMode = () => {
                          imageHitTest.onImageSelect(o);
                          setImageCropDraft({
                            id: o.id,
                            cropRect: computeInsetCropRect(o.bbox as PdfRect, 10),
                            activeHandle: null,
                            startClient: [0, 0],
                            startCrop: computeInsetCropRect(o.bbox as PdfRect, 10),
                          });
                        };
                        const startCropDrag = (
                          e: PointerEvent<HTMLDivElement>,
                          handle: "tl" | "tr" | "bl" | "br",
                        ) => {
                          e.stopPropagation();
                          e.currentTarget.setPointerCapture(e.pointerId);
                          setImageCropDraft({
                            id: o.id,
                            cropRect: cropDraft,
                            activeHandle: handle,
                            startClient: [e.clientX, e.clientY],
                            startCrop: cropDraft,
                          });
                        };
                        const updateCropDrag = (e: PointerEvent<HTMLDivElement>) => {
                          if (!imageCropDraft || imageCropDraft.id !== o.id || !imageCropDraft.activeHandle) return;
                          const dx = (e.clientX - imageCropDraft.startClient[0]) / zoomFactor;
                          const dyPdf = -(e.clientY - imageCropDraft.startClient[1]) / zoomFactor;
                          const next: PdfRect = [...imageCropDraft.startCrop] as PdfRect;
                          if (imageCropDraft.activeHandle.includes("l")) next[0] += dx;
                          if (imageCropDraft.activeHandle.includes("r")) next[2] += dx;
                          if (imageCropDraft.activeHandle.includes("b")) next[1] += dyPdf;
                          if (imageCropDraft.activeHandle.includes("t")) next[3] += dyPdf;
                          const minSize = 4;
                          next[0] = Math.max(o.bbox[0], Math.min(next[0], next[2] - minSize));
                          next[2] = Math.min(o.bbox[2], Math.max(next[2], next[0] + minSize));
                          next[1] = Math.max(o.bbox[1], Math.min(next[1], next[3] - minSize));
                          next[3] = Math.min(o.bbox[3], Math.max(next[3], next[1] + minSize));
                          setImageCropDraft({ ...imageCropDraft, cropRect: next });
                        };
                        const endCropDrag = (e: PointerEvent<HTMLDivElement>) => {
                          if (imageCropDraft?.id === o.id) {
                            e.currentTarget.releasePointerCapture(e.pointerId);
                            setImageCropDraft({ ...imageCropDraft, activeHandle: null });
                          }
                        };
                        return (
                          <div
                            key={o.id}
                            data-testid={`image-hit-${o.id}`}
                            data-source-id={o.id}
                            className={`canvas-image-hit ${isSelected ? "canvas-image-hit--selected" : ""}`}
                            title={[
                              "Image object",
                              `Dimensions: ${o.image_info?.width ?? 0}×${o.image_info?.height ?? 0}`,
                              "Drag to preview move. Drag handle to resize. Apply commits to PDF.",
                            ].join("\n")}
                            style={{
                              position: "absolute",
                              left,
                              top,
                              width,
                              height,
                              cursor: isSelected && !isCropping ? "move" : "pointer",
                              pointerEvents: "auto",
                              outline: isSelected
                                ? "2px solid rgba(30,160,140,0.95)"
                                : "1px dashed rgba(30,160,140,0.35)",
                              background: isSelected ? "rgba(30,160,140,0.10)" : "transparent",
                            }}
                            onClick={(e) => {
                              e.stopPropagation();
                              imageHitTest.onImageSelect(o);
                            }}
                            onDoubleClick={(e) => {
                              e.stopPropagation();
                              imageHitTest.onImageSelect(o);
                            }}
                            onPointerDown={(e) => {
                              if (!isCropping) startDrag(e, "move");
                            }}
                            onPointerMove={(e) => {
                              updateDrag(e);
                              updateCropDrag(e);
                            }}
                            onPointerUp={(e) => {
                              endDrag(e);
                              endCropDrag(e);
                            }}
                            role="button"
                            tabIndex={0}
                            aria-label={`Image object ${o.image_info?.width ?? 0} by ${o.image_info?.height ?? 0}`}
                          >
                            {isSelected && (
                              <>
                                {replacementDraft && replacementDrawScreen && (
                                  <div
                                    data-testid={`image-replace-preview-${o.id}`}
                                    style={{
                                      position: "absolute",
                                      left: (replacementClipScreen?.left ?? left) - left,
                                      top: (replacementClipScreen?.top ?? top) - top,
                                      width: replacementClipScreen?.width ?? width,
                                      height: replacementClipScreen?.height ?? height,
                                      overflow: "hidden",
                                      border: "2px solid #74a2ff",
                                      background: "rgba(116,162,255,0.10)",
                                      pointerEvents: "none",
                                    }}
                                  >
                                    <img
                                      src={replacementDraft.dataUrl}
                                      alt=""
                                      data-testid={`image-replace-preview-img-${o.id}`}
                                      style={{
                                        position: "absolute",
                                        left: replacementDrawScreen.left - (replacementClipScreen?.left ?? left),
                                        top: replacementDrawScreen.top - (replacementClipScreen?.top ?? top),
                                        width: replacementDrawScreen.width,
                                        height: replacementDrawScreen.height,
                                      }}
                                    />
                                  </div>
                                )}
                                {isCropping && (
                                  <div
                                    data-testid={`image-crop-preview-${o.id}`}
                                    style={{
                                      position: "absolute",
                                      left: cropScreen.left - left,
                                      top: cropScreen.top - top,
                                      width: cropScreen.width,
                                      height: cropScreen.height,
                                      border: "2px solid #ffd166",
                                      background: "rgba(255,209,102,0.12)",
                                      boxShadow: "0 0 0 9999px rgba(0,0,0,0.22)",
                                      pointerEvents: "none",
                                    }}
                                  />
                                )}
                                {isCropping && ([
                                  ["tl", cropScreen.left - left - 5, cropScreen.top - top - 5],
                                  ["tr", cropScreen.left - left + cropScreen.width - 5, cropScreen.top - top - 5],
                                  ["bl", cropScreen.left - left - 5, cropScreen.top - top + cropScreen.height - 5],
                                  ["br", cropScreen.left - left + cropScreen.width - 5, cropScreen.top - top + cropScreen.height - 5],
                                ] as const).map(([handle, hx, hy]) => (
                                  <div
                                    key={handle}
                                    data-testid={`image-crop-handle-${handle}-${o.id}`}
                                    title={`Crop ${handle}`}
                                    style={{
                                      position: "absolute",
                                      left: hx,
                                      top: hy,
                                      width: 10,
                                      height: 10,
                                      background: "#ffd166",
                                      border: "1px solid #111",
                                      cursor: `${handle === "tl" || handle === "br" ? "nwse" : "nesw"}-resize`,
                                      zIndex: 2,
                                    }}
                                    onPointerDown={(e) => startCropDrag(e, handle)}
                                  />
                                ))}
                                <div
                                  data-testid={`image-resize-${o.id}`}
                                  title="Resize image"
                                  style={{
                                    position: "absolute",
                                    right: -5,
                                    bottom: -5,
                                    width: 10,
                                    height: 10,
                                    background: "#1ea08c",
                                    border: "1px solid white",
                                    cursor: "nwse-resize",
                                  }}
                                  onPointerDown={(e) => startDrag(e, "resize")}
                                />
                                <div
                                  data-testid={`image-object-toolbar-${o.id}`}
                                  style={{
                                    position: "absolute",
                                    left: 0,
                                    top: -30,
                                    display: "flex",
                                    gap: 4,
                                    background: "rgba(20,28,36,0.92)",
                                    color: "#fff",
                                    padding: 3,
                                    borderRadius: 3,
                                    fontSize: 11,
                                  }}
                                  onPointerDown={(e) => e.stopPropagation()}
                                  onClick={(e) => e.stopPropagation()}
                                >
                                  <span className="badge badge-warning">Visual image replacement</span>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-apply-${o.id}`}
                                    onClick={() => {
                                      if (replacementDraft) void imageHitTest.onImageReplaceApply?.();
                                      else if (isCropping) void imageHitTest.onImageCropApply?.(o, cropDraft);
                                      else void imageHitTest.onImageEditApply(o, draft);
                                    }}
                                  >
                                    Apply
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-crop-${o.id}`}
                                    onClick={startCropMode}
                                    title={imageHitTest.onImageCropApply ? "Crop image with export-safe visual crop." : "Crop is unavailable: backend crop command is not wired."}
                                    disabled={!imageHitTest.onImageCropApply}
                                  >
                                    Crop
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-rotate-90-${o.id}`}
                                    onClick={() => void imageHitTest.onImageRotate?.(o, 90)}
                                    disabled={!imageHitTest.onImageRotate}
                                    title={imageHitTest.onImageRotate ? "Rotate 90 degrees." : "Rotation command is not wired."}
                                  >
                                    Rotate 90
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-rotate-180-${o.id}`}
                                    onClick={() => void imageHitTest.onImageRotate?.(o, 180)}
                                    disabled={!imageHitTest.onImageRotate}
                                    title={imageHitTest.onImageRotate ? "Rotate 180 degrees." : "Rotation command is not wired."}
                                  >
                                    180
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-cancel-${o.id}`}
                                    onClick={() => {
                                      setImageDraft(null);
                                      setImageCropDraft(null);
                                      if (replacementDraft) imageHitTest.onImageReplaceCancel?.();
                                    }}
                                  >
                                    Cancel
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-replace-${o.id}`}
                                    onClick={() => imageHitTest.onImageReplace?.(o)}
                                    disabled={!imageHitTest.onImageReplace}
                                    title={imageHitTest.onImageReplace ? "Open replacement flow in the inspector." : "Use Replace Image in the inspector."}
                                  >
                                    Replace
                                  </button>
                                  <button
                                    className="ghost-btn"
                                    data-testid={`image-delete-${o.id}`}
                                    onClick={() => void imageHitTest.onImageDelete?.(o)}
                                  >
                                    Delete
                                  </button>
                                </div>
                              </>
                            )}
                          </div>
                        );
                      })}
                  </div>
                )}
                {/* Phase 30E — canvas-overlay inline block editor. */}
                {inlineBlockEdit && (
                  <div
                    data-testid="canvas-inline-block-host"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: "100%",
                      pointerEvents: "none",
                      zIndex: 55,
                    }}
                  >
                    <div style={{ pointerEvents: "auto" }}>
                      <InlineBlockEditor
                        block={inlineBlockEdit.block}
                        pageHeightPts={inlineBlockEdit.pageHeightPts}
                        zoom={zoomFactor}
                        onApply={inlineBlockEdit.onApply}
                        onCancel={inlineBlockEdit.onCancel}
                      />
                    </div>
                  </div>
                )}
                {/* Phase 30A — canvas-overlay inline text editor. */}
                {inlineEdit && (
                  <div
                    data-testid="canvas-inline-edit-host"
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: "100%",
                      pointerEvents: "none",
                      zIndex: 60,
                    }}
                  >
                    <div style={{ pointerEvents: "auto" }}>
                      <InlineTextEditor
                        bbox={inlineEdit.contentObject.bbox}
                        pageHeightPts={inlineEdit.pageHeightPts}
                        zoom={zoomFactor}
                        initialText={inlineEdit.contentObject.text_info?.decoded_text ?? ""}
                        fontSize={inlineEdit.contentObject.text_info?.font_size ?? 12}
                        methodPreview={inlineEdit.methodPreview}
                        fontInfo={inlineEdit.fontInfo}
                        contentObject={inlineEdit.contentObject}
                        onApply={inlineEdit.onApply}
                        onCancel={inlineEdit.onCancel}
                      />
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <div className="workspace-state workspace-state--error workspace-state--flush">
                <h2>Document not attached to live backend session</h2>
                <p>
                  This tab came from local shell/session state and is not currently connected to a
                  Rust document session. Reopen the file to enable true rendering and search.
                </p>
              </div>
            )}
            <div className="workspace-canvas__meta">
              <span>{activeTab.sourcePath}</span>
              <span>
                Page {activeTab.page}/{activeTab.totalPages} · Zoom {activeTab.zoom}%
              </span>
            </div>
          </div>
        );
      }
    }
  };

  return (
    <section
      className="center-workspace"
      style={{
        paddingLeft: leftInset,
        paddingRight: rightInset,
      }}
    >
      <div className="workspace-body workspace-body--full">{renderContent()}</div>
    </section>
  );
};
