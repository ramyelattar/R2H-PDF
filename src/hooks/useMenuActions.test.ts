import { renderHook, act } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useMenuActions } from "./useMenuActions";
import { commandCatalog } from "../state/shellCatalog";

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    close: vi.fn(),
    isFullscreen: vi.fn().mockResolvedValue(false),
    setFullscreen: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../lib/ipc", () => ({
  closeSession: vi.fn(),
  editRedo: vi.fn(),
  editUndo: vi.fn(),
  searchClearIndex: vi.fn(),
}));

const makeDeps = (activeTab: Parameters<typeof useMenuActions>[0]["activeTab"] = null) => ({
  activeTab,
  diagnosticsOpen: false,
  closeTab: vi.fn(),
  nextTab: vi.fn(),
  setCurrentMode: vi.fn(),
  setCommandPaletteOpen: vi.fn(),
  setPreferencesOpen: vi.fn(),
  setDiagnosticsOpen: vi.fn(),
  setIsLeftSidebarOpen: vi.fn(),
  setIsRightSidebarOpen: vi.fn(),
  isLeftSidebarOpen: false,
  isRightSidebarOpen: true,
  appendDiagnostic: vi.fn(),
  requestExport: vi.fn(),
  requestInspectorTab: vi.fn(),
} satisfies Parameters<typeof useMenuActions>[0]);

const makeBackendTab = (): NonNullable<Parameters<typeof useMenuActions>[0]["activeTab"]> => ({
  id: "doc-session-menu-test",
  title: "Test.pdf",
  kind: "pdf",
  workspaceId: "ws-1",
  sourcePath: "C:/docs/Test.pdf",
  pinned: false,
  dirty: false,
  page: 1,
  totalPages: 1,
  zoom: 100,
  loadState: "ready",
});

describe("useMenuActions R2H feature command boundary", () => {
  it("rejects document commands before routing when no backend PDF is active", () => {
    const deps = makeDeps();
    const { result } = renderHook(() => useMenuActions(deps));
    const command = commandCatalog.find((entry) => entry.id === "cmd.open.ocr");
    if (!command) throw new Error("OCR command is missing from the production catalog.");

    act(() => result.current.executeCommand(command));

    expect(deps.requestInspectorTab).not.toHaveBeenCalled();
    expect(deps.setCommandPaletteOpen).not.toHaveBeenCalled();
    expect(deps.appendDiagnostic).toHaveBeenCalledWith(expect.objectContaining({
      level: "WARN",
      message: "Open OCR requires an active PDF document.",
    }));
  });

  it("routes each document command to the canonical R2H inspector tab for an active PDF", () => {
    const deps = makeDeps(makeBackendTab());
    const { result } = renderHook(() => useMenuActions(deps));
    const expectedRoutes = [
      ["cmd.open.ocr", "ocr"],
      ["cmd.open.compare", "compare"],
      ["cmd.open.forms", "forms"],
      ["cmd.open.pageOrganizer", "organizer"],
      ["cmd.open.signStamp", "sign-stamp"],
      ["cmd.open.localGeneration", "local-generation"],
    ] as const;

    for (const [commandId, tabId] of expectedRoutes) {
      const command = commandCatalog.find((entry) => entry.id === commandId);
      if (!command) throw new Error(`Missing command ${commandId}.`);
      act(() => result.current.executeCommand(command));
      expect(deps.requestInspectorTab).toHaveBeenLastCalledWith(tabId);
    }
  });
});
