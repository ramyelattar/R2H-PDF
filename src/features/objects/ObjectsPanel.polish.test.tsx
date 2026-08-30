import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import type { TextBoxObject } from "../pdf-editor/types";

vi.mock("../../lib/ipc", () => ({
  docListFormFields: vi.fn(async () => ({ ok: true, data: [] })),
  pdfListContentEdits: vi.fn(async () => ({ ok: true, data: [] })),
  pdfGetPageContentObjects: vi.fn(async () => ({
    ok: true,
    data: [
      {
        id: "native-1",
        session_id: "doc-session-test",
        page_index: 0,
        object_type: "text_span",
        bbox: [0, 0, 20, 20],
        editable_level: "read_only",
        text_info: { decoded_text: "Native text", raw_text: "Native text" },
        image_info: null,
        diagnostics: ["not available for this object"],
      },
    ],
  })),
}));

import { ObjectsPanel } from "./ObjectsPanel";

const overlay: TextBoxObject = {
  id: "t1",
  sessionId: "doc-session-test",
  pageIndex: 0,
  type: "textBox",
  rect: { x: 0, y: 0, width: 100, height: 24 },
  rotation: 0,
  zIndex: 2,
  locked: false,
  hidden: false,
  createdAt: 0,
  updatedAt: 0,
  metadata: {},
  text: "Overlay note",
  fontSize: 12,
  fontFamily: "Helvetica",
  color: "#000",
  backgroundColor: "#fff",
  borderColor: "#000",
};

describe("Pass 2E — ObjectsPanel polish", () => {
  it("renders filters, row badges, z-order controls, and native delete disabled reason", async () => {
    render(
      <ObjectsPanel
        sessionId="doc-session-test"
        overlayObjects={[overlay]}
        activePageIndex={0}
        totalPages={1}
      />,
    );

    expect(screen.getByTestId("objects-search")).toBeTruthy();
    await waitFor(() => expect(screen.getByText("Overlay note")).toBeTruthy());
    expect(screen.getAllByTestId("objects-type-badge")[0].textContent).toMatch(/Text box/);
    expect(screen.getAllByTestId("objects-zorder-group").length).toBeGreaterThan(0);
    const disabledDelete = screen.getAllByTestId("objects-delete").find((button) =>
      button.getAttribute("title")?.includes("Native PDF content cannot be deleted"),
    );
    expect(disabledDelete).toBeTruthy();
    expect((disabledDelete as HTMLButtonElement).disabled).toBe(true);
  });
});
