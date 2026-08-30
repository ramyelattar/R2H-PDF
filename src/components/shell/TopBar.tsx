import { useState } from "react";
import type { WorkstationState } from "../../state/useWorkstationState";
import type { ReaderMode } from "../../types/shell";

const ZOOM_PRESETS = [25, 50, 75, 100, 125, 150, 175, 200, 300, 400];

interface TopBarProps {
  state: WorkstationState;
  activeTab: WorkstationState["tabs"][number] | null;
  searchQuery: string;
  fitMode: "page" | "width";
  onSearchQueryChange: (value: string) => void;
  onSearchSubmit: () => void;
  onOpenFile: () => void;
  onPreviousPage: () => void;
  onNextPage: () => void;
  onGoToPage: (page: number) => void;
  onZoomOut: () => void;
  onZoomIn: () => void;
  onSetZoomPreset: (zoom: number) => void;
  onSetFitMode: (fit: "page" | "width") => void;
  onSetMode: (mode: ReaderMode) => void;
  onToggleLeftSidebar: () => void;
  onToggleThumbnails: () => void;
  onToggleRightSidebar: () => void;
  onShowDocumentProperties: () => void;
  /**
   * Phase 32 — opens the right inspector on the Export tab. Visible in the
   * TopBar so users can reach Export without scanning the wrapping tab bar.
   */
  onShowExport: () => void;
  showThumbnails: boolean;
}

export const TopBar = ({
  state,
  activeTab,
  searchQuery,
  fitMode,
  onSearchQueryChange,
  onSearchSubmit,
  onOpenFile,
  onPreviousPage,
  onNextPage,
  onGoToPage,
  onZoomOut,
  onZoomIn,
  onSetZoomPreset,
  onSetFitMode,
  onSetMode,
  onToggleLeftSidebar,
  onToggleThumbnails,
  onToggleRightSidebar,
  onShowDocumentProperties,
  onShowExport,
  showThumbnails,
}: TopBarProps) => {
  const [pageInputValue, setPageInputValue] = useState("");
  const [editingPage, setEditingPage] = useState(false);

  const currentPage = activeTab?.page ?? 1;
  const totalPages = activeTab?.totalPages ?? 0;
  const currentZoom = activeTab?.zoom ?? 100;

  const commitPageJump = () => {
    const n = parseInt(pageInputValue, 10);
    if (!isNaN(n) && n >= 1 && n <= totalPages) {
      onGoToPage(n);
    }
    setEditingPage(false);
    setPageInputValue("");
  };

  return (
    <header className="top-bar">
      {/* Left group — sidebar toggles + open */}
      <div className="top-bar__group">
        <button
          className={`ghost-btn top-bar__icon-btn ${state.isLeftSidebarOpen ? "is-active" : ""}`}
          onClick={onToggleLeftSidebar}
          title="Toggle Document Browser"
        >
          ☰
        </button>
        <button
          className={`ghost-btn top-bar__icon-btn ${showThumbnails ? "is-active" : ""}`}
          onClick={onToggleThumbnails}
          title="Toggle Page Thumbnails"
        >
          ⊞
        </button>
        <button className="ghost-btn" onClick={onOpenFile} title="Open PDF (Ctrl+O)">
          Open
        </button>
      </div>

      <div className="top-bar__divider" />

      {/* Page navigation */}
      <div className="top-bar__group">
        <button
          className="ghost-btn top-bar__icon-btn"
          onClick={onPreviousPage}
          disabled={!activeTab || currentPage <= 1}
          title="Previous Page (←)"
        >
          ‹
        </button>

        {editingPage ? (
          <input
            className="compact-input top-bar__page-input"
            type="number"
            min={1}
            max={totalPages}
            value={pageInputValue}
            autoFocus
            onChange={(e) => setPageInputValue(e.target.value)}
            onBlur={commitPageJump}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitPageJump();
              if (e.key === "Escape") { setEditingPage(false); setPageInputValue(""); }
            }}
          />
        ) : (
          <button
            className="top-bar__page-display"
            title="Click to jump to page"
            onClick={() => {
              setEditingPage(true);
              setPageInputValue(String(currentPage));
            }}
          >
            <span className="top-bar__page-current">{activeTab ? currentPage : "–"}</span>
            <span className="top-bar__page-sep">/</span>
            <span className="top-bar__page-total">{activeTab ? totalPages || "–" : "–"}</span>
          </button>
        )}

        <button
          className="ghost-btn top-bar__icon-btn"
          onClick={onNextPage}
          disabled={!activeTab || currentPage >= totalPages}
          title="Next Page (→)"
        >
          ›
        </button>
      </div>

      <div className="top-bar__divider" />

      {/* Zoom controls */}
      <div className="top-bar__group">
        <button
          className="ghost-btn top-bar__icon-btn"
          onClick={onZoomOut}
          disabled={!activeTab || currentZoom <= 25}
          title="Zoom Out"
        >
          −
        </button>

        <select
          className="top-bar__zoom-select"
          value={ZOOM_PRESETS.includes(currentZoom) ? currentZoom : ""}
          onChange={(e) => {
            const val = parseInt(e.target.value, 10);
            if (!isNaN(val)) onSetZoomPreset(val);
          }}
          title="Zoom Level"
        >
          {!ZOOM_PRESETS.includes(currentZoom) && (
            <option value="">{currentZoom}%</option>
          )}
          {ZOOM_PRESETS.map((z) => (
            <option key={z} value={z}>{z}%</option>
          ))}
        </select>

        <button
          className="ghost-btn top-bar__icon-btn"
          onClick={onZoomIn}
          disabled={!activeTab || currentZoom >= 400}
          title="Zoom In"
        >
          +
        </button>

        <button
          className={`ghost-btn ${fitMode === "page" ? "is-active" : ""}`}
          onClick={() => onSetFitMode("page")}
          title="Fit Page"
        >
          Fit
        </button>
        <button
          className={`ghost-btn ${fitMode === "width" ? "is-active" : ""}`}
          onClick={() => onSetFitMode("width")}
          title="Fit Width"
        >
          Width
        </button>
      </div>

      <div className="top-bar__divider" />

      {/* Search */}
      <div className="top-bar__group">
        <input
          className="compact-input top-bar__search-input"
          type="search"
          placeholder="Search document…"
          value={searchQuery}
          onChange={(e) => onSearchQueryChange(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") onSearchSubmit(); }}
        />
        <button className="ghost-btn" onClick={onSearchSubmit} title="Search (Enter)">
          🔍
        </button>
      </div>

      <div className="top-bar__divider" />

      {/* View mode */}
      <div className="top-bar__group">
        {(["read", "annotate", "edit-content", "review"] as const).map((mode) => (
          <button
            key={mode}
            className={`ghost-btn ${state.currentMode === mode ? "is-active" : ""}`}
            onClick={() => onSetMode(mode)}
            title={`${mode.charAt(0).toUpperCase() + mode.slice(1)} mode`}
          >
            {mode.charAt(0).toUpperCase() + mode.slice(1)}
          </button>
        ))}
      </div>

      {/* Right group */}
      <div className="top-bar__group top-bar__group--right">
        <button
          className="ghost-btn top-bar__export-btn"
          onClick={onShowExport}
          title="Export — open the Export panel"
          data-testid="topbar-export-btn"
        >
          ⤓ Export
        </button>
        {activeTab && (
          <button
            className="ghost-btn top-bar__icon-btn"
            onClick={onShowDocumentProperties}
            title="Document Properties"
          >
            ⓘ
          </button>
        )}
        <button
          className={`ghost-btn top-bar__icon-btn ${state.isRightSidebarOpen ? "is-active" : ""}`}
          onClick={onToggleRightSidebar}
          title="Toggle Inspector"
        >
          ⊟
        </button>
      </div>

      <div className="top-bar__brand" data-testid="top-bar-brand" title="R2H PDF AI Workstation">
        <span className="top-bar__brand-dot" aria-hidden="true" />
        <div className="top-bar__brand-text">
          <span className="top-bar__brand-name">R2H PDF</span>
          <span className="top-bar__brand-workspace">
            {state.workspaces.find((ws) => ws.id === state.activeWorkspaceId)?.name ??
              "No workspace"}
          </span>
        </div>
      </div>
    </header>
  );
};
