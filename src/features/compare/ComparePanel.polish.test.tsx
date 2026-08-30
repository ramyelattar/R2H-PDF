import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

const mocks = vi.hoisted(() => ({
  openMock: vi.fn(async () => "C:/tmp/revised.pdf"),
  saveMock: vi.fn(async () => "C:/tmp/compare.html"),
  compareMock: vi.fn(async () => ({
    ok: true,
    data: {
      compare_id: "cmp-1",
      base_document: { session_id: "doc-session-test", page_count: 1 },
      revised_document: { file_path: "C:/tmp/revised.pdf", page_count: 1 },
      mode: "combined",
      summary: {
        pages_compared: 1,
        pages_with_changes: 1,
        lines_added: 1,
        lines_removed: 1,
        lines_modified: 1,
        visual_changes: 1,
        identical: false,
      },
      page_results: [],
      text_changes: [
        { page_index: 0, change_type: "added", old_text: null, new_text: "Added text", bbox: [1, 1, 10, 10], citation: "p.1" },
        { page_index: 0, change_type: "removed", old_text: "Removed text", new_text: null, bbox: [1, 12, 10, 20], citation: "p.1" },
        { page_index: 0, change_type: "modified", old_text: "Old", new_text: "New", bbox: [1, 22, 10, 30], citation: "p.1" },
      ],
      visual_changes: [
        { page_index: 0, change_type: "visual_modified", bbox: [1, 1, 20, 20], confidence: 0.8, description: "Changed region" },
      ],
      warnings: [],
      created_at: 0,
    },
  })),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: mocks.openMock,
  save: mocks.saveMock,
}));

vi.mock("../../lib/ipc", () => ({
  invokeSafe: vi.fn(async () => ({ ok: true, data: true })),
  pdfCompareDocuments: mocks.compareMock,
  reportExportCompareReview: vi.fn(async () => ({ ok: true, data: { output_path: "C:/tmp/compare.html", bytes_written: 123 } })),
  aiReviewCompareResult: vi.fn(),
}));

import { ComparePanel } from "./ComparePanel";

describe("Pass 2C — ComparePanel polish", () => {
  it("renders workflow steps, runs compare, groups results, and exposes row actions", async () => {
    render(
      <ComparePanel
        sessionId="doc-session-test"
        onAddEditorObjects={() => {}}
        onNavigateToPage={() => {}}
      />,
    );

    expect(screen.getByText(/1. Select revised PDF/)).toBeTruthy();
    expect(screen.getByText(/6. Export report\/PDF/)).toBeTruthy();
    fireEvent.click(screen.getByTestId("compare-pick"));
    await waitFor(() => expect(mocks.openMock).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("compare-run"));
    await waitFor(() => expect(mocks.compareMock).toHaveBeenCalled());

    expect(screen.getByTestId("compare-group-summary").textContent).toMatch(/Added 1/);
    expect(screen.getByTestId("compare-group-summary").textContent).toMatch(/Removed 1/);
    expect(screen.getByTestId("compare-group-summary").textContent).toMatch(/Modified 1/);
    expect(screen.getByTestId("compare-group-summary").textContent).toMatch(/Visual 1/);
    expect(screen.getAllByTestId("compare-add-comment").length).toBeGreaterThan(0);
    expect(screen.getAllByTestId("compare-add-highlight").length).toBeGreaterThan(0);
    expect(screen.getAllByTestId("compare-add-redline").length).toBeGreaterThan(0);
    expect(screen.getByTestId("compare-export-html")).toBeTruthy();
  });
});
