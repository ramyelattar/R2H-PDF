import { useEffect, useState } from "react";
import type { WorkstationState } from "../../state/useWorkstationState";
import { R2H_FEATURE_OWNERSHIP, type R2hFeatureTabId } from "../../state/r2hFeatureOwnership";
import type { UiPreferences } from "../../types/shell";
import {
  annotDelete,
  annotList,
  type Annotation,
  type SearchMatch,
} from "../../lib/ipc";
import { ExportPanel } from "../../features/export";
import type { EditorObject } from "../../features/pdf-editor/types";
import { RagPanel } from "../../features/rag";
import { AiActionPanel } from "../../features/ai-actions";
import { EngineeringPanel } from "../../features/engineering";
import { ReviewPanel } from "../../features/review";
import { ReportExportPanel } from "../../features/reports";
import { ModelManagerPanel } from "../../features/model-manager";
import { ContentEditPanel } from "../../features/content-edit";
import { ObjectsPanel } from "../../features/objects";
import { OcrPanel } from "../../features/ocr";
import { ComparePanel } from "../../features/compare";
import { FormsPanel } from "../../features/forms";
import { PageOrganizer } from "../../features/page-organizer/PageOrganizer";
import { SignStampPanel } from "../../features/sign-stamp/SignStampPanel";
import { LocalAiPanel } from "../../features/ai-local/LocalAiPanel";
import type { StagedImageReplacement } from "../../features/content-edit/imageLayout";

export type InspectorTab =
  | "properties"
  | "annotations"
  | "objects"
  | "search"
  | "edit"
  | "export"
  | "engineering"
  | "report"
  | "ai"
  | "models"
  | R2hFeatureTabId;

/**
 * Phase 32 — UI discoverability. Tabs are defined declaratively so the list
 * order is the only thing the layout cares about, and tests can introspect
 * the full set of tabs without scraping JSX. `requiresDoc` flags the tabs
 * whose panels need an open PDF session. Disabled tabs are native-disabled
 * controls and are also guarded in the shared tab-change path.
 */
const TAB_DEFS: ReadonlyArray<{
  id: InspectorTab;
  short: string;
  long: string;
  requiresDoc: boolean;
}> = [
  { id: "properties", short: "Props", long: "Properties", requiresDoc: true },
  { id: "annotations", short: "Annot", long: "Annotations", requiresDoc: true },
  { id: "objects", short: "Objects", long: "Objects", requiresDoc: true },
  { id: "search", short: "Search", long: "Search", requiresDoc: false },
  { id: "edit", short: "Edit", long: "Edit Content", requiresDoc: true },
  { id: "export", short: "Export", long: "Export", requiresDoc: true },
  { id: "engineering", short: "Eng", long: "Engineering", requiresDoc: true },
  { id: "report", short: "Report", long: "Report", requiresDoc: true },
  { id: "ai", short: "AI", long: "Ask AI", requiresDoc: true },
  // Models is the one tab that runs entirely without a document — keep it
  // first-class so users can manage local models even before opening a PDF.
  { id: "models", short: "Models", long: "Models", requiresDoc: false },
  ...R2H_FEATURE_OWNERSHIP.map((feature) => ({
    id: feature.tabId,
    short: feature.tabShortLabel,
    long: feature.tabLongLabel,
    requiresDoc: feature.requiresDocument,
  })),
];

interface WorkflowShortcut {
  id: string;
  label: string;
  target: InspectorTab;
  testId: string;
}

const WORKFLOWS: ReadonlyArray<WorkflowShortcut> = [
  { id: "edit", label: "Edit PDF", target: "edit", testId: "workflow-edit" },
  { id: "export", label: "Export PDF", target: "export", testId: "workflow-export" },
  { id: "ai", label: "Ask AI", target: "ai", testId: "workflow-ai" },
];

interface RightInspectorProps {
  activeTab: WorkstationState["tabs"][number] | null;
  currentMode: WorkstationState["currentMode"];
  preferences: UiPreferences;
  editorObjects: EditorObject[];
  searchResults: SearchMatch[];
  searchBusy: boolean;
  searchMessage: string | null;
  searchMatchIndex: number;
  searchCaseSensitive: boolean;
  searchWholeWords: boolean;
  searchUseRegex: boolean;
  onSearchCaseSensitiveChange: (value: boolean) => void;
  onSearchWholeWordsChange: (value: boolean) => void;
  onSearchUseRegexChange: (value: boolean) => void;
  onJumpToSearchMatch: (match: SearchMatch, index: number) => void;
  onNavigateToPage?: (pageIndex: number) => void;
  onApplyAiActions?: (actions: import("../../features/ai-actions/types").AiProposedAction[]) => void;
  onContentEdited?: () => void;
  /** Phase 28F — notify parent so search/RAG cache can be invalidated. */
  onTextEdited?: (info: { method: import("../../lib/ipc").EditMethodValue; pageIndex: number }) => void;
  /** Phase 28F — true when text edits happened after the last RAG index build. */
  indexStale?: boolean;
  /** Phase 28F — called by RagPanel after a rebuild to clear the flag. */
  onIndexRefreshed?: () => void;
  /** Phase 30C — experimental ToUnicode native edit feature flag. */
  enableExperimentalToUnicode?: boolean;
  onToggleExperimentalToUnicode?: (next: boolean) => void;
  /** Phase 30E — open the canvas-overlay block editor for a block. */
  onRequestCanvasBlockEdit?: (block: import("../../lib/ipc").TextBlock) => void;
  onStageImageReplace?: (draft: StagedImageReplacement) => void;
  stagedImageReplace?: StagedImageReplacement | null;
  onUpdateStagedImageReplace?: (patch: Partial<Pick<StagedImageReplacement, "layoutMode" | "preserveAspect">>) => void;
  onApplyStagedImageReplace?: () => void | Promise<void>;
  onCancelStagedImageReplace?: () => void;
  /** Phase 25A — Objects panel hooks. */
  onSelectOverlay?: (id: import("../../features/pdf-editor/types").EditorObjectId) => void;
  onPatchOverlay?: (
    id: import("../../features/pdf-editor/types").EditorObjectId,
    patch: import("../../features/pdf-editor/types").EditorObjectPatch,
  ) => void;
  onDeleteOverlay?: (id: import("../../features/pdf-editor/types").EditorObjectId) => void;
  /** Phase 25B/C — generic overlay add (used by Sign & Stamp panel). */
  onAddOverlay?: (object: EditorObject) => void;
  onAddEditorObjects?: (objects: EditorObject[]) => void;
  /** OCR acceptance is owned by App so it can use the canonical project_save flow. */
  onAcceptOcrOverlays?: (
    result: import("../../features/ocr/types").OcrPageResult,
    overlays: import("../../lib/ipc").OcrOverlaySpec[],
  ) => Promise<boolean> | boolean;
  onAcceptGeneratedText?: (text: string) => Promise<boolean>;
  onLibraryUpdated?: (snapshot: import("../../types/shell").LibrarySnapshot) => void;
  onReviewCompleted?: (sourcePath: string, reviewId: string) => Promise<void> | void;
  onCompareRefreshed?: () => void;
  pageOrganizer?: {
    selectedPages: number[];
    busy: boolean;
    selectPage: (pageIndex: number, mode?: "replace" | "toggle") => void;
    rotatePage: (pageIndex: number, degrees: number) => Promise<boolean>;
    deletePage: (pageIndex: number) => Promise<boolean>;
    insertBlankPage: (atIndex: number) => Promise<boolean>;
    movePage: (fromIndex: number, toIndex: number) => Promise<boolean>;
  };
  appendDiagnostic?: import("../../lib/diagnostics").AppendDiagnostic;
  /**
   * Phase 32 — when set, the inspector switches to this tab. The parent
   * should clear it via `onTabChange` after the inspector reports the
   * switch so subsequent external requests still trigger updates.
   */
  requestedTab?: InspectorTab | null;
  /** Phase 32 — emitted whenever the active inspector tab changes. */
  onTabChange?: (next: InspectorTab) => void;
  onClose: () => void;
}

export const RightInspector = ({
  activeTab,
  currentMode,
  editorObjects,
  searchResults,
  searchBusy,
  searchMessage,
  searchMatchIndex,
  searchCaseSensitive,
  searchWholeWords,
  searchUseRegex,
  onSearchCaseSensitiveChange,
  onSearchWholeWordsChange,
  onSearchUseRegexChange,
  onJumpToSearchMatch,
  onNavigateToPage,
  onApplyAiActions,
  onContentEdited,
  onTextEdited,
  indexStale,
  onIndexRefreshed,
  enableExperimentalToUnicode,
  onToggleExperimentalToUnicode,
  onRequestCanvasBlockEdit,
  onStageImageReplace,
  stagedImageReplace,
  onUpdateStagedImageReplace,
  onApplyStagedImageReplace,
  onCancelStagedImageReplace,
  onSelectOverlay,
  onPatchOverlay,
  onDeleteOverlay,
  onAddOverlay,
  onAddEditorObjects,
  onAcceptOcrOverlays,
  onAcceptGeneratedText,
  onLibraryUpdated,
  onReviewCompleted,
  onCompareRefreshed,
  pageOrganizer,
  appendDiagnostic,
  requestedTab,
  onTabChange,
  onClose,
}: RightInspectorProps) => {
  // Fallback so child panels always receive a callable diagnostic sink. When
  // `appendDiagnostic` isn't wired from the parent (legacy callers, tests),
  // entries are at least surfaced to the developer console rather than
  // silently dropped.
  const diag: import("../../lib/diagnostics").AppendDiagnostic = appendDiagnostic ?? ((entry) => {
    console.info(`[diag:${entry.level}] (${entry.source}) ${entry.message}`);
  });
  const [tab, setTabRaw] = useState<InspectorTab>("properties");
  const [annotations, setAnnotations] = useState<Annotation[]>([]);
  const [annotBusy, setAnnotBusy] = useState(false);
  const [annotError, setAnnotError] = useState<string | null>(null);

  const isBackendSession =
    activeTab?.kind === "pdf" && activeTab.id.startsWith("doc-session-");

  const canAccessTab = (next: InspectorTab) => {
    const definition = TAB_DEFS.find((entry) => entry.id === next);
    return !definition?.requiresDoc || isBackendSession;
  };

  /**
   * Phase 32 — single entry point for tab changes so the workflows row, the
   * tab bar, and external requests (e.g. the TopBar Export button) all go
   * through the same emitter.
   */
  const setTab = (next: InspectorTab) => {
    if (!canAccessTab(next)) {
      diag({
        level: "WARN",
        source: "ui",
        message: `${TAB_DEFS.find((entry) => entry.id === next)?.long ?? next} requires an active PDF document.`,
      });
      return;
    }
    setTabRaw(next);
    onTabChange?.(next);
  };

  // Apply external tab requests (e.g. the TopBar "Export" button).
  useEffect(() => {
    if (requestedTab && requestedTab !== tab && canAccessTab(requestedTab)) {
      setTabRaw(requestedTab);
      onTabChange?.(requestedTab);
    }
    // We intentionally depend only on requestedTab — the parent clears it
    // after consuming, which retriggers this effect on the next request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestedTab]);

  const activeTabId = activeTab?.id;

  useEffect(() => {
    if (!isBackendSession || tab !== "annotations" || !activeTabId) return;
    let canceled = false;
    setAnnotBusy(true);
    setAnnotError(null);
    annotList(activeTabId, null).then((result) => {
      if (canceled) return;
      setAnnotBusy(false);
      if (!result.ok) { setAnnotError(result.error.message); return; }
      setAnnotations(result.data.annotations);
    });
    return () => { canceled = true; };
  }, [activeTabId, tab, isBackendSession]);

  const handleDeleteAnnotation = async (annotationId: string) => {
    if (!isBackendSession) return;
    const result = await annotDelete(activeTab!.id, annotationId);
    if (result.ok) {
      setAnnotations((prev) => prev.filter((a) => a.id !== annotationId));
    } else {
      setAnnotError(result.error.message);
      diag({ level: "ERROR", source: "ipc", message: `Annotation delete failed: ${result.error.message}` });
    }
  };

  const formatAnnotType = (type: string) => type.replace(/([A-Z])/g, " $1").trim();

  const formatTs = (iso: string) => {
    try {
      return new Date(iso).toLocaleString(undefined, {
        month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
      });
    } catch { return iso; }
  };

  return (
    <aside className="right-inspector" data-testid="right-inspector">
      <div className="inspector-titlebar">
        <span className="inspector-title">Inspector</span>
        <button
          className="ghost-btn inspector-export-shortcut"
          onClick={() => setTab("export")}
          disabled={!canAccessTab("export")}
          title="Jump to Export"
          data-testid="inspector-export-shortcut"
        >
          ⤓ Export
        </button>
        <button className="ghost-btn" onClick={onClose} title="Close">✕</button>
      </div>

      {/* Phase 32 — Workflows row keeps the common entry points one click
          away even when the user hasn't yet noticed the wrapping tab bar. */}
      <div
        className="inspector-workflows"
        role="group"
        aria-label="Workflows"
        data-testid="inspector-workflows"
      >
        <span className="inspector-workflows__label">Workflows:</span>
        {WORKFLOWS.map((w) => (
          <button
            key={w.id}
            className={`ghost-btn inspector-workflow-btn ${tab === w.target ? "is-active" : ""}`}
            onClick={() => setTab(w.target)}
            disabled={!canAccessTab(w.target)}
            data-testid={w.testId}
            title={`Switch to ${w.label}`}
          >
            {w.label}
          </button>
        ))}
      </div>

      <div
        className="inspector-tabs inspector-tabs--wrap"
        role="tablist"
        aria-label="Inspector tabs"
        data-testid="inspector-tabs"
      >
        {TAB_DEFS.map(({ id, short, long, requiresDoc }) => {
          const disabled = requiresDoc && (!activeTab || !isBackendSession);
          return (
            <button
              key={id}
              role="tab"
              aria-selected={tab === id}
              aria-disabled={disabled}
              disabled={disabled}
              className={`inspector-tab ${tab === id ? "inspector-tab--active" : ""} ${disabled ? "inspector-tab--disabled" : ""}`}
              onClick={() => setTab(id)}
              data-testid={`inspector-tab-${id}`}
              title={disabled ? `${long} — open a PDF to enable` : long}
            >
              {short}
            </button>
          );
        })}
      </div>

      <div className="inspector-body">
        {(() => {
          const def = TAB_DEFS.find((d) => d.id === tab);
          if (!def) return null;
          return (
            <div className="inspector-panel-heading" data-testid="inspector-panel-heading">
              <span className="section-eyebrow">Panel</span>
              <h3 className="inspector-panel-heading__title">{def.long}</h3>
            </div>
          );
        })()}
        {tab === "properties" && (
          <>
            {!activeTab ? (
              <div className="empty-state" data-testid="empty-state-properties">
                <span className="empty-state__icon" aria-hidden="true">📄</span>
                <p className="empty-state__title">No document open</p>
                <p className="empty-state__hint">Open a PDF to inspect its properties, permissions, and session.</p>
              </div>
            ) : (
              <>
                <div className="inspector-section">
                  <h4 className="inspector-section-title">Document</h4>
                  <div className="inspector-grid">
                    <span>Title</span>
                    <strong title={activeTab.title}>{activeTab.title || "—"}</strong>
                    <span>Pages</span>
                    <strong>{activeTab.totalPages || "—"}</strong>
                    <span>Current</span>
                    <strong>{activeTab.page || "—"}</strong>
                    <span>Zoom</span>
                    <strong>{activeTab.zoom}%</strong>
                    {activeTab.rotation !== undefined && (
                      <>
                        <span>Rotation</span>
                        <strong>{activeTab.rotation}°</strong>
                      </>
                    )}
                    <span>State</span>
                    <strong className={activeTab.loadState === "error" ? "text-error" : ""}>
                      {activeTab.loadState.toUpperCase()}
                    </strong>
                    {activeTab.dirty && (
                      <>
                        <span>Modified</span>
                        <strong className="text-warn">Unsaved</strong>
                      </>
                    )}
                  </div>
                </div>

                {activeTab.permissions && (
                  <div className="inspector-section">
                    <h4 className="inspector-section-title">Permissions</h4>
                    <div className="permissions-row">
                      {(
                        [
                          ["Print", activeTab.permissions.canPrint],
                          ["Copy", activeTab.permissions.canCopy],
                          ["Edit", activeTab.permissions.canEdit],
                          ["Annotate", activeTab.permissions.canAnnotate],
                        ] as [string, boolean][]
                      ).map(([label, allowed]) => (
                        <span key={label} className={`perm-badge ${allowed ? "perm-badge--allow" : "perm-badge--deny"}`}>
                          {allowed ? "✓" : "✗"} {label}
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                <div className="inspector-section">
                  <h4 className="inspector-section-title">Session</h4>
                  <div className="inspector-grid">
                    <span>Mode</span>
                    <strong>{currentMode.toUpperCase()}</strong>
                    <span>Workspace</span>
                    <strong>{activeTab.workspaceId}</strong>
                    <span>File</span>
                    <strong className="path-mono" title={activeTab.sourcePath}>
                      {activeTab.sourcePath.split(/[/\\]/).pop()}
                    </strong>
                  </div>
                </div>
              </>
            )}
          </>
        )}

        {tab === "annotations" && (
          <>
            {!isBackendSession ? (
              <div className="empty-state">
                <span className="empty-state__icon" aria-hidden="true">✎</span>
                <p className="empty-state__title">No document open</p>
                <p className="empty-state__hint">Open a PDF to view and manage annotations.</p>
              </div>
            ) : annotBusy ? (
              <div className="loading-bar" />
            ) : annotError ? (
              <div className="callout callout--danger">{annotError}</div>
            ) : annotations.length === 0 ? (
              <div className="empty-state">
                <span className="empty-state__icon" aria-hidden="true">✎</span>
                <p className="empty-state__title">No annotations yet</p>
                <p className="empty-state__hint">Switch to Annotate mode and mark up the page to see entries here.</p>
              </div>
            ) : (
              <ul className="annot-list">
                {annotations.map((annot) => (
                  <li key={annot.id} className="annot-item">
                    <div className="annot-item__header">
                      <span
                        className="annot-color-dot"
                        style={{
                          background: `rgba(${annot.color.r},${annot.color.g},${annot.color.b},${annot.color.a / 255})`,
                        }}
                      />
                      <span className="annot-type">{formatAnnotType(annot.annot_type)}</span>
                      <span className="annot-page">p.{annot.page_index + 1}</span>
                      <button
                        className="annot-delete-btn"
                        title="Delete annotation"
                        onClick={() => void handleDeleteAnnotation(annot.id)}
                      >
                        ✕
                      </button>
                    </div>
                    {annot.contents && <p className="annot-contents">{annot.contents}</p>}
                    <div className="annot-meta">
                      {annot.author && <span>{annot.author}</span>}
                      <span>{formatTs(annot.modified_at)}</span>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </>
        )}

        {tab === "search" && (
          <>
            <div className="search-options-row">
              <label className="search-option-toggle" title="Case sensitive">
                <input
                  type="checkbox"
                  checked={searchCaseSensitive}
                  onChange={(e) => onSearchCaseSensitiveChange(e.target.checked)}
                />
                <span>Aa</span>
              </label>
              <label className="search-option-toggle" title="Whole words">
                <input
                  type="checkbox"
                  checked={searchWholeWords}
                  onChange={(e) => onSearchWholeWordsChange(e.target.checked)}
                />
                <span>W</span>
              </label>
              <label className="search-option-toggle" title="Use regular expression">
                <input
                  type="checkbox"
                  checked={searchUseRegex}
                  onChange={(e) => onSearchUseRegexChange(e.target.checked)}
                />
                <span>.*</span>
              </label>
            </div>
            {searchBusy && <div className="loading-bar" />}
            {searchMessage && <p className="empty-text">{searchMessage}</p>}
            {searchResults.length > 0 && (
              <>
                <div className="search-nav-row">
                  <span className="search-count">
                    {searchMatchIndex + 1} / {searchResults.length}
                  </span>
                  <button
                    className="ghost-btn"
                    onClick={() => {
                      const prev = (searchMatchIndex - 1 + searchResults.length) % searchResults.length;
                      onJumpToSearchMatch(searchResults[prev], prev);
                    }}
                  >
                    ↑ Prev
                  </button>
                  <button
                    className="ghost-btn"
                    onClick={() => {
                      const next = (searchMatchIndex + 1) % searchResults.length;
                      onJumpToSearchMatch(searchResults[next], next);
                    }}
                  >
                    ↓ Next
                  </button>
                </div>
                <ul className="search-list">
                  {searchResults.slice(0, 100).map((match, idx) => (
                    <li key={`${match.page_index}-${match.match_index}`}>
                      <button
                        className={`search-match-btn ${idx === searchMatchIndex ? "search-match-btn--active" : ""}`}
                        onClick={() => onJumpToSearchMatch(match, idx)}
                      >
                        <span className="search-match-page">p.{match.page_index + 1}</span>
                        <span className="search-match-snippet">{match.snippet}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </>
        )}


        {tab === "export" && activeTab && activeTab.kind === "pdf" && activeTab.id.startsWith("doc-session-") && (
          <ExportPanel
            sessionId={activeTab.id}
            sourcePath={activeTab.sourcePath}
            objects={editorObjects}
            appendDiagnostic={diag}
            onLibraryUpdated={onLibraryUpdated}
          />
        )}

        {tab === "ocr" && isBackendSession && activeTab && (
          <OcrPanel
            sessionId={activeTab.id}
            currentPageIndex={Math.max(activeTab.page - 1, 0)}
            totalPages={activeTab.totalPages ?? 0}
            appendDiagnostic={diag}
            onAcceptOcrOverlays={onAcceptOcrOverlays}
          />
        )}

        {tab === "compare" && isBackendSession && activeTab && (
          <ComparePanel
            sessionId={activeTab.id}
            sourcePath={activeTab.sourcePath}
            onNavigateToPage={onNavigateToPage}
            onAddEditorObjects={onAddEditorObjects}
            onCompareRefreshed={onCompareRefreshed}
          />
        )}

        {tab === "forms" && isBackendSession && activeTab && (
          <FormsPanel
            sessionId={activeTab.id}
            pageIndex={Math.max(activeTab.page - 1, 0)}
            onNavigateToPage={onNavigateToPage}
            onContentEdited={onContentEdited}
          />
        )}

        {tab === "organizer" && isBackendSession && activeTab && pageOrganizer && (
          <PageOrganizer
            sessionId={activeTab.id}
            totalPages={activeTab.totalPages ?? 0}
            selectedPages={pageOrganizer.selectedPages}
            busy={pageOrganizer.busy}
            editorObjects={new Map(editorObjects.map((object) => [object.id, object] as const))}
            onSelectPage={(pageIndex) => pageOrganizer.selectPage(pageIndex)}
            onRotateCW={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              if (pageIndex !== undefined) void pageOrganizer.rotatePage(pageIndex, 90);
            }}
            onRotateCCW={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              if (pageIndex !== undefined) void pageOrganizer.rotatePage(pageIndex, -90);
            }}
            onDeletePage={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              if (pageIndex !== undefined) void pageOrganizer.deletePage(pageIndex);
            }}
            onInsertBlank={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              void pageOrganizer.insertBlankPage(pageIndex === undefined ? activeTab.totalPages : pageIndex + 1);
            }}
            onMoveUp={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              if (pageIndex !== undefined && pageIndex > 0) {
                void pageOrganizer.movePage(pageIndex, pageIndex - 1);
              }
            }}
            onMoveDown={() => {
              const pageIndex = pageOrganizer.selectedPages[0];
              if (pageIndex !== undefined && pageIndex < activeTab.totalPages - 1) {
                void pageOrganizer.movePage(pageIndex, pageIndex + 1);
              }
            }}
            onClose={() => setTab("properties")}
          />
        )}

        {tab === "sign-stamp" && isBackendSession && activeTab && onAddOverlay && (
          <SignStampPanel
            sessionId={activeTab.id}
            activePageIndex={Math.max(activeTab.page - 1, 0)}
            onAddOverlay={onAddOverlay}
          />
        )}

        {tab === "sign-stamp" && isBackendSession && !onAddOverlay && (
          <div className="callout callout--danger">Sign &amp; Stamp is unavailable because the editor persistence sink is not wired.</div>
        )}

        {tab === "local-generation" && isBackendSession && activeTab && (
          <LocalAiPanel
            sessionId={activeTab.id}
            documentTitle={activeTab.title}
            onAcceptGeneratedText={onAcceptGeneratedText}
          />
        )}

        {tab === "engineering" && activeTab && activeTab.kind === "pdf" && activeTab.id.startsWith("doc-session-") && (
          <EngineeringPanel
            sessionId={activeTab.id}
            onNavigateToPage={onNavigateToPage}
          />
        )}

        {tab === "report" && activeTab && activeTab.kind === "pdf" && activeTab.id.startsWith("doc-session-") && (
          <ReportExportPanel sessionId={activeTab.id} />
        )}

        {tab === "edit" && activeTab && activeTab.kind === "pdf" && activeTab.id.startsWith("doc-session-") && (
          <ContentEditPanel
            sessionId={activeTab.id}
            currentPageIndex={Math.max(activeTab.page - 1, 0)}
            onContentEdited={onContentEdited}
            onAddOverlay={onAddOverlay}
            onTextEdited={onTextEdited}
            enableExperimentalToUnicode={enableExperimentalToUnicode}
            onToggleExperimentalToUnicode={onToggleExperimentalToUnicode}
            onRequestCanvasBlockEdit={onRequestCanvasBlockEdit}
            onStageImageReplace={onStageImageReplace}
            stagedImageReplace={stagedImageReplace}
            onUpdateStagedImageReplace={onUpdateStagedImageReplace}
            onApplyStagedImageReplace={onApplyStagedImageReplace}
            onCancelStagedImageReplace={onCancelStagedImageReplace}
          />
        )}

        {tab === "edit" && (!activeTab || activeTab.kind !== "pdf" || !activeTab.id.startsWith("doc-session-")) && (
          <div className="empty-state">
            <span className="empty-state__icon" aria-hidden="true">✎</span>
            <p className="empty-state__title">No document open</p>
            <p className="empty-state__hint">Open a PDF to edit text and images natively or with a safe visual replacement.</p>
          </div>
        )}



        {tab === "objects" && activeTab && activeTab.kind === "pdf" && activeTab.id.startsWith("doc-session-") && (
          <ObjectsPanel
            sessionId={activeTab.id}
            overlayObjects={editorObjects}
            activePageIndex={Math.max(activeTab.page - 1, 0)}
            totalPages={activeTab.totalPages ?? 0}
            onNavigateToPage={onNavigateToPage}
            onSelectOverlay={onSelectOverlay}
            onPatchOverlay={onPatchOverlay}
            onDeleteOverlay={onDeleteOverlay}
          />
        )}

        {tab === "objects" && (!activeTab || activeTab.kind !== "pdf" || !activeTab.id.startsWith("doc-session-")) && (
          <div className="empty-state">
            <span className="empty-state__icon" aria-hidden="true">◫</span>
            <p className="empty-state__title">No document open</p>
            <p className="empty-state__hint">Open a PDF to inspect every object on every page.</p>
          </div>
        )}


        {tab === "ai" && activeTab && (
          <>
            <ReviewPanel
              sessionId={activeTab.id}
              onNavigateToPage={onNavigateToPage}
              onReviewCompleted={(reviewId) => onReviewCompleted?.(activeTab.sourcePath, reviewId)}
              onApplySuggestedActions={(suggested) => {
                // Adapt ReviewSuggestedAction → AiProposedAction so the
                // shared editor-overlay pipeline (onApplyAiActions) can
                // place the suggestions as real overlay objects.
                const adapted = suggested.map((s, i) => ({
                  action_id: s.action_id || `review-${Date.now()}-${i}`,
                  action_type: (s.action_type as import("../../features/ai-actions/types").AiActionType) || "add_comment",
                  session_id: activeTab.id,
                  page_index: s.page_index,
                  rect: { x: 72, y: 72 + i * 30, width: 240, height: 24 },
                  text: s.text || s.reason,
                  reason: s.reason,
                  confidence: s.confidence,
                  source_citations: s.citations,
                  status: "proposed" as const,
                  created_at: Date.now(),
                }));
                onApplyAiActions?.(adapted);
                diag({ level: "INFO", source: "ui", message: `Routed ${adapted.length} review suggestions through the editor overlay.` });
              }}
            />
            <AiActionPanel
              sessionId={activeTab.id}
              pageCount={activeTab.totalPages ?? 0}
              appendDiagnostic={diag}
              onApplyActions={(actions) => onApplyAiActions?.(actions)}
            />
            <RagPanel
              sessionId={activeTab.id}
              appendDiagnostic={diag}
              getPageTexts={async () => {
                // Fetch both native text and (best-effort) cached OCR text so
                // the RAG index can include OCR'd scanned pages. OCR is
                // optional — if unavailable or empty, native text is used
                // alone.
                const ipc = await import("../../lib/ipc");
                const nativeRes = await ipc.docExtractAllText(activeTab.id);
                if (!nativeRes.ok) return [];
                const ocrMap = new Map<number, string>();
                const ocrRes = await ipc.ocrGetAllTexts(activeTab.id);
                if (ocrRes.ok) {
                  for (const [pageIndex, text] of ocrRes.data) {
                    if (text && text.trim().length > 0) ocrMap.set(pageIndex, text);
                  }
                }
                return nativeRes.data.map((text, i) => ({
                  page_index: i,
                  native_text: text,
                  ocr_text: ocrMap.get(i) ?? null,
                }));
              }}
              onNavigateToPage={onNavigateToPage}
              indexStale={indexStale}
              onIndexRefreshed={onIndexRefreshed}
            />
          </>
        )}

        {tab === "ai" && !activeTab && (
          <div className="empty-state">
            <span className="empty-state__icon" aria-hidden="true">✶</span>
            <p className="empty-state__title">No document open</p>
            <p className="empty-state__hint">Open a PDF to use Review, Ask AI, and the action suggester. Local AI is offline by default.</p>
          </div>
        )}

        {tab === "models" && (
          <ModelManagerPanel />
        )}
      </div>
    </aside>
  );
};
