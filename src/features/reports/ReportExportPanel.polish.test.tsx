import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("./useReportExport", () => ({
  useReportExport: () => ({
    status: "completed",
    error: null,
    exportReport: vi.fn(),
    lastResult: {
      report_id: "rep-1",
      output_path: "C:/tmp/review.html",
      format: "html",
      sections_included: ["review", "engineering"],
      findings_count: 3,
      engineering_findings_count: 1,
      calculation_traces_count: 1,
      ai_actions_count: 2,
      citations_count: 4,
      warnings: ["Some citations were omitted."],
      created_at: 0,
    },
  }),
}));

import { ReportExportPanel } from "./ReportExportPanel";

describe("Pass 2F — ReportExportPanel polish", () => {
  it("renders HTML export card, honest PDF disabled copy, sections, action, and last result", () => {
    render(<ReportExportPanel sessionId="doc-session-test" />);

    expect(screen.getByTestId("report-html-card").textContent).toMatch(/HTML report export/);
    expect(screen.getByText(/PDF report export is disabled/)).toBeTruthy();
    expect(screen.getByTestId("report-sections-card").textContent).toMatch(/Engineering findings/);
    expect(screen.getByTestId("report-export-action").textContent).toMatch(/Export Report/);
    expect(screen.getByTestId("report-last-result").textContent).toMatch(/review.html/);
    expect(screen.getByText(/Some citations were omitted/)).toBeTruthy();
  });
});
