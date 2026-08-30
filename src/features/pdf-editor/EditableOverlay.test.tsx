import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { EditableOverlay } from "./EditableOverlay";
import type { StrikethroughObject, UnderlineObject } from "./types";

const PAGE = { widthPts: 612, heightPts: 792, pageIndex: 0 };
const VIEW = { zoom: 1, dpr: 1, scrollX: 0, scrollY: 0 };

function strike(meta: Record<string, unknown> = {}): StrikethroughObject {
  return {
    id: "s1", sessionId: "ss", pageIndex: 0, type: "strikethrough",
    rect: { x: 100, y: 600, width: 200, height: 30 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: meta,
    color: "#c00", contents: "removed text",
  };
}

function underline(meta: Record<string, unknown> = {}): UnderlineObject {
  return {
    id: "u1", sessionId: "ss", pageIndex: 0, type: "underline",
    rect: { x: 100, y: 600, width: 200, height: 30 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: meta,
    color: "#080", contents: "added text",
  };
}

describe("Phase 27C — EditableOverlay redline rendering", () => {
  it("uses heuristic rows when no line_bboxes are provided", () => {
    render(
      <EditableOverlay
        objects={[strike()]}
        selectedIds={[]}
        activeTool="select"
        page={PAGE}
        view={VIEW}
        onSelectObject={() => {}}
        onClearSelection={() => {}}
        onMoveObject={() => {}}
        onResizeObject={() => {}}
        onCreateObject={() => {}}
      />,
    );
    const el = screen.getByTestId("redline-strikethrough");
    expect(el.getAttribute("data-source")).toBe("heuristic");
    // 30pt height / 14pt rows ≈ 2 rows
    const rows = parseInt(el.getAttribute("data-rows") ?? "0", 10);
    expect(rows).toBeGreaterThanOrEqual(1);
  });

  it("uses line_bboxes when metadata provides them (strikethrough)", () => {
    render(
      <EditableOverlay
        objects={[strike({ line_bboxes: [[100, 612, 300, 624]] })]}
        selectedIds={[]}
        activeTool="select"
        page={PAGE}
        view={VIEW}
        onSelectObject={() => {}}
        onClearSelection={() => {}}
        onMoveObject={() => {}}
        onResizeObject={() => {}}
        onCreateObject={() => {}}
      />,
    );
    const el = screen.getByTestId("redline-strikethrough");
    expect(el.getAttribute("data-source")).toBe("line-bboxes");
    expect(el.getAttribute("data-rows")).toBe("1");
  });

  it("uses line_bboxes when metadata provides them (underline)", () => {
    render(
      <EditableOverlay
        objects={[underline({ line_bboxes: [[100, 612, 300, 622]] })]}
        selectedIds={[]}
        activeTool="select"
        page={PAGE}
        view={VIEW}
        onSelectObject={() => {}}
        onClearSelection={() => {}}
        onMoveObject={() => {}}
        onResizeObject={() => {}}
        onCreateObject={() => {}}
      />,
    );
    const el = screen.getByTestId("redline-underline");
    expect(el.getAttribute("data-source")).toBe("line-bboxes");
  });

  it("renders one line per line_bbox entry when multiple are supplied", () => {
    render(
      <EditableOverlay
        objects={[
          strike({
            line_bboxes: [
              [100, 624, 300, 632],
              [100, 612, 300, 620],
              [100, 600, 300, 608],
            ],
          }),
        ]}
        selectedIds={[]}
        activeTool="select"
        page={PAGE}
        view={VIEW}
        onSelectObject={() => {}}
        onClearSelection={() => {}}
        onMoveObject={() => {}}
        onResizeObject={() => {}}
        onCreateObject={() => {}}
      />,
    );
    const el = screen.getByTestId("redline-strikethrough");
    expect(el.getAttribute("data-rows")).toBe("3");
    expect(screen.queryByTestId("redline-line-0")).not.toBeNull();
    expect(screen.queryByTestId("redline-line-1")).not.toBeNull();
    expect(screen.queryByTestId("redline-line-2")).not.toBeNull();
  });

  it("falls back to heuristic when line_bboxes is empty or malformed", () => {
    render(
      <EditableOverlay
        objects={[strike({ line_bboxes: [] })]}
        selectedIds={[]}
        activeTool="select"
        page={PAGE}
        view={VIEW}
        onSelectObject={() => {}}
        onClearSelection={() => {}}
        onMoveObject={() => {}}
        onResizeObject={() => {}}
        onCreateObject={() => {}}
      />,
    );
    const el = screen.getByTestId("redline-strikethrough");
    expect(el.getAttribute("data-source")).toBe("heuristic");
  });
});
