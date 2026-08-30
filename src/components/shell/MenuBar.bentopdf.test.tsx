import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MenuBar } from "./MenuBar";

const createProps = () => ({
  onOpenFile: vi.fn(),
  onOpenRecent: vi.fn(),
  onOpenBentoPdf: vi.fn(),
  onSave: vi.fn(),
  onSaveAs: vi.fn(),
  onExport: vi.fn(),
  canExport: true,
  onCloseDocument: vi.fn(),
  onExit: vi.fn(),
  onUndo: vi.fn(),
  onRedo: vi.fn(),
  onCut: vi.fn(),
  onCopy: vi.fn(),
  onPaste: vi.fn(),
  onFind: vi.fn(),
  onPreferences: vi.fn(),
  onZoomIn: vi.fn(),
  onZoomOut: vi.fn(),
  onFitPage: vi.fn(),
  onFitWidth: vi.fn(),
  onToggleLeftSidebar: vi.fn(),
  onToggleRightSidebar: vi.fn(),
  onFullScreen: vi.fn(),
});

describe("MenuBar BentoPDF entry", () => {
  it("opens the real BentoPDF callback and closes the File menu", () => {
    const props = createProps();
    render(<MenuBar {...props} />);

    fireEvent.click(screen.getByRole("button", { name: "File" }));
    fireEvent.click(screen.getByTestId("menu-open-bentopdf"));

    expect(props.onOpenBentoPdf).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId("menu-open-bentopdf")).toBeNull();
  });
});
