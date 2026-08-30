import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useWorkstationState } from "./useWorkstationState";
import type { DocumentTab, ShellSessionSnapshot } from "../types/shell";

describe("useWorkstationState actions", () => {
  const makeTab = (overrides: Partial<DocumentTab> = {}): DocumentTab => ({
    id: "tab-1",
    title: "Test",
    kind: "pdf",
    workspaceId: "ws-1",
    sourcePath: "test.pdf",
    pinned: false,
    dirty: false,
    page: 1,
    totalPages: 5,
    zoom: 100,
    loadState: "ready",
    ...overrides,
  });

  it("openTab adds a tab and activates it", () => {
    const { result } = renderHook(() => useWorkstationState());
    const tab = makeTab({ id: "tab-1" });
    act(() => {
      result.current.actions.openTab(tab);
    });
    expect(result.current.state.tabs).toContainEqual(
      expect.objectContaining({ id: "tab-1" }),
    );
    expect(result.current.state.activeTabId).toBe("tab-1");
  });

  it("openTab does not duplicate an existing tab, just activates it", () => {
    const { result } = renderHook(() => useWorkstationState());
    const tab = makeTab({ id: "tab-1" });
    act(() => {
      result.current.actions.openTab(tab);
      result.current.actions.openTab(makeTab({ id: "tab-2", sourcePath: "b.pdf" }));
    });
    // Re-open tab-1
    act(() => {
      result.current.actions.openTab(tab);
    });
    expect(result.current.state.tabs.filter((t) => t.id === "tab-1")).toHaveLength(1);
    expect(result.current.state.activeTabId).toBe("tab-1");
  });

  it("closeTab removes the tab and picks adjacent", () => {
    const { result } = renderHook(() => useWorkstationState());
    act(() => {
      result.current.actions.openTab(
        makeTab({ id: "t1", title: "T1", sourcePath: "a.pdf" }),
      );
      result.current.actions.openTab(
        makeTab({ id: "t2", title: "T2", sourcePath: "b.pdf" }),
      );
    });
    // t2 is active (last opened)
    expect(result.current.state.activeTabId).toBe("t2");
    act(() => {
      result.current.actions.closeTab("t2");
    });
    expect(result.current.state.tabs.find((t) => t.id === "t2")).toBeUndefined();
    expect(result.current.state.activeTabId).toBe("t1");
  });

  it("closeTab sets activeTabId to null when last tab is closed", () => {
    const { result } = renderHook(() => useWorkstationState());
    act(() => {
      result.current.actions.openTab(makeTab({ id: "only" }));
    });
    act(() => {
      result.current.actions.closeTab("only");
    });
    expect(result.current.state.tabs).toHaveLength(0);
    expect(result.current.state.activeTabId).toBeNull();
  });

  it("setTabLoadState updates the correct tab", () => {
    const { result } = renderHook(() => useWorkstationState());
    act(() => {
      result.current.actions.openTab(
        makeTab({ id: "t1", title: "T1", sourcePath: "a.pdf" }),
      );
    });
    act(() => {
      result.current.actions.setTabLoadState("t1", "error", "File not found");
    });
    const tab = result.current.state.tabs.find((t) => t.id === "t1");
    expect(tab?.loadState).toBe("error");
    expect(tab?.errorMessage).toBe("File not found");
  });

  it("setTabLoadState does not affect other tabs", () => {
    const { result } = renderHook(() => useWorkstationState());
    act(() => {
      result.current.actions.openTab(
        makeTab({ id: "t1", sourcePath: "a.pdf" }),
      );
      result.current.actions.openTab(
        makeTab({ id: "t2", sourcePath: "b.pdf" }),
      );
    });
    act(() => {
      result.current.actions.setTabLoadState("t1", "error", "Oops");
    });
    const t2 = result.current.state.tabs.find((t) => t.id === "t2");
    expect(t2?.loadState).toBe("ready");
    expect(t2?.errorMessage).toBeUndefined();
  });

  it("restoreSession restores all fields", () => {
    const { result } = renderHook(() => useWorkstationState());
    const snapshot: ShellSessionSnapshot = {
      activeWorkspaceId: "ws-litigation-alpha",
      openTabs: [
        makeTab({ id: "restored-1", title: "Restored" }),
        makeTab({ id: "restored-2", title: "Restored 2", sourcePath: "x.pdf" }),
      ],
      activeTabId: "restored-2",
      currentMode: "annotate",
      isLeftSidebarOpen: true,
      isRightSidebarOpen: true,
      diagnosticsOpen: true,
      preferences: {
        compactDensity: false,
        showRightInspector: false,
        showLeftPanel: false,
        restoreLastSession: false,
        telemetryMode: "verbose",
        aiEnabled: true,
        aiEndpoint: "http://localhost:11434",
        aiModel: "llama3",
        aiSummarizeEnabled: true,
        aiQaEnabled: true,
        aiAnnotationSuggestEnabled: true,
        aiEntityExtractEnabled: true,
      },
    };

    act(() => {
      result.current.actions.restoreSession(snapshot);
    });

    expect(result.current.state.activeWorkspaceId).toBe("ws-litigation-alpha");
    expect(result.current.state.tabs).toHaveLength(2);
    expect(result.current.state.activeTabId).toBe("restored-2");
    expect(result.current.state.currentMode).toBe("annotate");
    expect(result.current.state.isLeftSidebarOpen).toBe(true);
    expect(result.current.state.isRightSidebarOpen).toBe(true);
    expect(result.current.state.diagnosticsOpen).toBe(true);
    expect(result.current.state.preferences.aiEnabled).toBe(true);
    expect(result.current.state.preferences.telemetryMode).toBe("verbose");
  });
});
