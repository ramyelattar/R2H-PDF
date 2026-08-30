/**
 * Phase 32 — UI discoverability tests for the RightInspector.
 *
 * These verify the discoverability fixes:
 *   - every major tab is rendered in the DOM (no overflow clipping)
 *   - workflow shortcuts switch the active tab
 *   - the inspector responds to external `requestedTab` requests
 *   - the Edit tab is reachable and the Models tab works without a document
 *
 * Tab-content tests for individual panels live in their own feature suites
 * (e.g. `content-edit/InlineTextEditor.test.tsx`). This file owns the
 * inspector chrome only.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";

// Mock the Tauri IPC bridge so the inspector and its child panels never
// hit a missing `window.__TAURI__` global in happy-dom.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("invoke not stubbed in test")),
}));

import { RightInspector, type InspectorTab } from "./RightInspector";
import type { WorkstationState } from "../../state/useWorkstationState";
import type { UiPreferences } from "../../types/shell";

const defaultPrefs: UiPreferences = {
  compactDensity: true,
  showRightInspector: true,
  showLeftPanel: true,
  restoreLastSession: true,
  telemetryMode: "standard",
  aiEnabled: true,
  aiEndpoint: "http://localhost:11434",
  aiModel: "",
  aiSummarizeEnabled: false,
  aiQaEnabled: false,
  aiAnnotationSuggestEnabled: false,
  aiEntityExtractEnabled: false,
};

type ActiveTab = WorkstationState["tabs"][number];

const makeBackendTab = (overrides: Partial<ActiveTab> = {}): ActiveTab => ({
  id: "doc-session-test-1",
  title: "Test.pdf",
  kind: "pdf",
  workspaceId: "ws-1",
  sourcePath: "/tmp/test.pdf",
  pinned: false,
  dirty: false,
  page: 1,
  totalPages: 3,
  zoom: 100,
  loadState: "ready",
  ...overrides,
});

interface RenderOpts {
  activeTab?: ActiveTab | null;
  requestedTab?: InspectorTab | null;
  onTabChange?: (tab: InspectorTab) => void;
}

const renderInspector = (opts: RenderOpts = {}) => {
  const onClose = vi.fn();
  render(
    <RightInspector
      activeTab={opts.activeTab ?? null}
      currentMode="read"
      preferences={defaultPrefs}
      editorObjects={[]}
      searchResults={[]}
      searchBusy={false}
      searchMessage={null}
      searchMatchIndex={0}
      searchCaseSensitive={false}
      searchWholeWords={false}
      searchUseRegex={false}
      onSearchCaseSensitiveChange={() => {}}
      onSearchWholeWordsChange={() => {}}
      onSearchUseRegexChange={() => {}}
      onJumpToSearchMatch={() => {}}
      requestedTab={opts.requestedTab ?? null}
      onTabChange={opts.onTabChange}
      onClose={onClose}
    />,
  );
  return { onClose };
};

const REQUIRED_TABS: InspectorTab[] = [
  "properties",
  "annotations",
  "objects",
  "search",
  "edit",
  "export",
  "engineering",
  "report",
  "ai",
  "models",
  "ocr",
  "compare",
  "forms",
  "organizer",
  "sign-stamp",
  "local-generation",
];

describe("Phase 32 — RightInspector tab discoverability", () => {
  it("renders every required tab button (no horizontal overflow can hide them)", () => {
    renderInspector({ activeTab: makeBackendTab() });
    for (const tab of REQUIRED_TABS) {
      const btn = screen.getByTestId(`inspector-tab-${tab}`);
      expect(btn, `tab ${tab} should render`).toBeTruthy();
      // The element must live inside the wrapping tab strip — that strip is
      // configured to wrap rather than overflow, so every button is visible.
      const strip = btn.closest('[data-testid="inspector-tabs"]');
      expect(strip, `tab ${tab} should sit inside .inspector-tabs`).not.toBeNull();
    }
  });

  it("uses the wrapping tab variant so right-hand tabs cannot be clipped off-screen", () => {
    renderInspector({ activeTab: makeBackendTab() });
    const strip = screen.getByTestId("inspector-tabs");
    expect(strip.className).toContain("inspector-tabs--wrap");
  });

  it("renders the Models tab even when no document is open", () => {
    renderInspector({ activeTab: null });
    const modelsTab = screen.getByTestId("inspector-tab-models");
    expect(modelsTab).toBeTruthy();
    // Models works without a document — it must NOT be marked disabled.
    expect(modelsTab.getAttribute("aria-disabled")).toBe("false");
  });

  it("marks doc-dependent tabs as aria-disabled when no document is open, but they still render", () => {
    renderInspector({ activeTab: null });
    for (const tab of ["edit", "export", "ai", "ocr", "compare", "forms", "organizer", "sign-stamp", "local-generation"] as InspectorTab[]) {
      const btn = screen.getByTestId(`inspector-tab-${tab}`);
      expect(btn).toBeTruthy();
      expect(btn.getAttribute("aria-disabled")).toBe("true");
      expect((btn as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it("Export, AI and Models tabs are reachable via direct clicks", () => {
    renderInspector({ activeTab: makeBackendTab() });
    for (const tab of ["export", "ai", "models"] as InspectorTab[]) {
      const btn = screen.getByTestId(`inspector-tab-${tab}`);
      fireEvent.click(btn);
      expect(btn.getAttribute("aria-selected")).toBe("true");
    }
  });
});

describe("Phase 32 — RightInspector workflow shortcuts", () => {
  it("renders the Workflows row with all common entry points", () => {
    renderInspector({ activeTab: makeBackendTab() });
    expect(screen.getByTestId("inspector-workflows")).toBeTruthy();
    for (const id of [
      "workflow-edit",
      "workflow-export",
      "workflow-ai",
    ]) {
      expect(screen.getByTestId(id), `${id} workflow should render`).toBeTruthy();
    }
  });

  it("clicking the Export workflow shortcut activates the Export tab", () => {
    renderInspector({ activeTab: makeBackendTab() });
    fireEvent.click(screen.getByTestId("workflow-export"));
    const exportTab = screen.getByTestId("inspector-tab-export");
    expect(exportTab.getAttribute("aria-selected")).toBe("true");
  });

  it("clicking the Edit workflow shortcut activates the Edit tab", () => {
    renderInspector({ activeTab: makeBackendTab() });
    fireEvent.click(screen.getByTestId("workflow-edit"));
    expect(screen.getByTestId("inspector-tab-edit").getAttribute("aria-selected")).toBe("true");
  });


  it("clicking the AI workflow shortcut activates the AI tab", () => {
    renderInspector({ activeTab: makeBackendTab() });
    fireEvent.click(screen.getByTestId("workflow-ai"));
    expect(screen.getByTestId("inspector-tab-ai").getAttribute("aria-selected")).toBe("true");
  });

  it("the inspector exposes an Export shortcut in the title bar", () => {
    renderInspector({ activeTab: makeBackendTab() });
    const shortcut = screen.getByTestId("inspector-export-shortcut");
    fireEvent.click(shortcut);
    expect(screen.getByTestId("inspector-tab-export").getAttribute("aria-selected")).toBe("true");
  });
});

describe("Phase 32 — RightInspector external tab requests", () => {
  it("honors `requestedTab` so the TopBar Export button can drive the inspector", () => {
    const onTabChange = vi.fn();
    renderInspector({
      activeTab: makeBackendTab(),
      requestedTab: "export",
      onTabChange,
    });
    // The mount-time effect should switch the tab to export and emit a
    // change notification so the parent can clear its one-shot request.
    expect(screen.getByTestId("inspector-tab-export").getAttribute("aria-selected")).toBe("true");
    expect(onTabChange).toHaveBeenCalledWith("export");
  });

  it("emits onTabChange when the user clicks a tab directly", () => {
    const onTabChange = vi.fn();
    renderInspector({ activeTab: makeBackendTab(), onTabChange });
    fireEvent.click(screen.getByTestId("inspector-tab-ai"));
    expect(onTabChange).toHaveBeenCalledWith("ai");
  });
});
