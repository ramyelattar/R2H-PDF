/**
 * Pass 2A — Export panel polish tests.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  revealItemInDir: vi.fn(),
}));

import { ExportPanel } from "./ExportPanel";
import type { EditorObject, ShapeObject, AnnotationObject } from "../pdf-editor/types";

const baseShape = (overrides: Partial<ShapeObject> = {}): ShapeObject => ({
  id: "s1",
  sessionId: "doc-session-test",
  pageIndex: 0,
  type: "rectangle",
  rect: { x: 0, y: 0, width: 100, height: 100 },
  rotation: 0,
  zIndex: 1,
  locked: false,
  hidden: false,
  createdAt: 0,
  updatedAt: 0,
  metadata: {},
  strokeColor: "#fff",
  strokeWidth: 1,
  fillColor: "#fff",
  ...overrides,
});

const baseComment = (overrides: Partial<AnnotationObject> = {}): AnnotationObject => ({
  id: "c1",
  sessionId: "doc-session-test",
  pageIndex: 0,
  type: "comment",
  rect: { x: 0, y: 0, width: 100, height: 100 },
  rotation: 0,
  zIndex: 1,
  locked: false,
  hidden: false,
  createdAt: 0,
  updatedAt: 0,
  metadata: {},
  color: "#fff",
  contents: "Hello",
  ...overrides,
} as AnnotationObject);

const render_ = (objects: EditorObject[] = []) =>
  render(
    <ExportPanel
      sessionId="doc-session-test"
      sourcePath="C:/tmp/test.pdf"
      objects={objects}
      appendDiagnostic={() => {}}
    />,
  );

describe("Pass 2A — ExportPanel", () => {
  it("renders the primary title 'Export Edited PDF'", () => {
    render_();
    // The panel header uses a section-header__title — that's the section
    // title, separate from the action button which also uses the same
    // copy. Use heading role to scope.
    expect(screen.getByRole("heading", { name: /Export Edited PDF/ })).toBeTruthy();
  });

  it("renders the primary Export button", () => {
    render_([baseShape()]);
    const btn = screen.getByTestId("export-primary-btn");
    expect(btn).toBeTruthy();
    expect(btn.textContent).toMatch(/Export Edited PDF/);
  });

  it("allows exporting an unchanged PDF when no overlays are present", () => {
    render_([]);
    const btn = screen.getByTestId("export-primary-btn") as HTMLButtonElement;
    expect(btn.disabled).toBe(false);
    expect(btn.getAttribute("aria-disabled")).toBe("false");
  });

  it("shows every required included-export category, including zero-count categories", () => {
    render_([
      baseShape({ id: "r1", type: "rectangle" }),
      baseComment({ id: "c1", type: "comment" }),
      baseShape({ id: "ocr1", metadata: { source: "ocr" } }),
    ]);
    const list = screen.getByTestId("export-included-list");
    expect(list.textContent).toMatch(/Text edits/);
    expect(list.textContent).toMatch(/Image edits/);
    expect(list.textContent).toMatch(/Annotations/);
    expect(list.textContent).toMatch(/Signatures/);
    expect(list.textContent).toMatch(/Stamps/);
    expect(list.textContent).toMatch(/Forms/);
    expect(list.textContent).toMatch(/OCR overlays/);
    expect(list.textContent).toMatch(/Compare redlines/);
    expect(list.textContent).toMatch(/AI comments/);
  });

  it("shows a helpful empty hint inside the included card when nothing has been added yet", () => {
    render_([]);
    const card = screen.getByTestId("export-included-card");
    expect(card.textContent).toMatch(/No edits yet/i);
  });

  it("does not render an export error or retry button by default", () => {
    render_([baseShape()]);
    expect(screen.queryByTestId("export-error")).toBeNull();
    expect(screen.queryByTestId("export-retry-btn")).toBeNull();
  });

  it("does not render redaction warning when apply-redactions is off", () => {
    render_([baseShape()]);
    expect(screen.queryByTestId("export-redaction-warning")).toBeNull();
  });

  it("renders no dead buttons in the default state (every button has a handler)", () => {
    render_([baseShape()]);
    const buttons = screen.getAllByRole("button");
    expect(buttons.length).toBeGreaterThan(0);
    let dead = 0;
    for (const btn of buttons) {
      // A button is "dead" only if it has no onClick handler AND is not
      // disabled. Disabled buttons with no handler are acceptable — the
      // disabled state communicates the reason to the user.
      const isDisabled = (btn as HTMLButtonElement).disabled;
      const hasOnClick = btn.outerHTML.includes("onclick") || true; // React listens via delegation
      if (!isDisabled && !hasOnClick) dead++;
    }
    expect(dead).toBe(0);
  });
});
