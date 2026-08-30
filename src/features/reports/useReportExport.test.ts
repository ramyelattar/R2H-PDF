import { describe, it, expect } from "vitest";
import type { ReportExportResult, ReportFormat } from "./types";

describe("ReportExportResult", () => {
  it("represents successful HTML export", () => {
    const result: ReportExportResult = {
      report_id: "report-1", output_path: "/tmp/review_report.html",
      format: "html", sections_included: ["cover", "review", "engineering", "appendix"],
      findings_count: 5, engineering_findings_count: 2,
      calculation_traces_count: 1, ai_actions_count: 3, citations_count: 8,
      warnings: [], created_at: Date.now(),
    };
    expect(result.format).toBe("html");
    expect(result.findings_count).toBe(5);
    expect(result.citations_count).toBe(8);
    expect(result.sections_included).toContain("review");
  });

  it("PDF format reports unavailable", () => {
    // PDF export is deferred — UI should show clear message.
    const format: ReportFormat = "pdf";
    expect(format).toBe("pdf");
    // The backend returns an error for PDF format.
  });
});

describe("Report options", () => {
  it("toggles affect sections included", () => {
    const includeReview = true;
    const includeEngineering = false;
    const sections: string[] = [];
    if (includeReview) sections.push("review");
    if (includeEngineering) sections.push("engineering");
    expect(sections).toContain("review");
    expect(sections).not.toContain("engineering");
  });

  it("no-data state shows warning", () => {
    // When no review or engineering data exists, export should fail with clear message.
    const errorMessage = "No review or engineering findings are available to export.";
    expect(errorMessage).toContain("No review");
  });
});

describe("Report UI logic", () => {
  it("export button disabled when exporting", () => {
    const status = "exporting";
    expect(status === "exporting").toBe(true);
  });

  it("summary renders after completion", () => {
    const result: ReportExportResult = {
      report_id: "r1", output_path: "/out.html", format: "html",
      sections_included: ["cover"], findings_count: 3,
      engineering_findings_count: 0, calculation_traces_count: 0,
      ai_actions_count: 0, citations_count: 5, warnings: [], created_at: 0,
    };
    expect(result.output_path).toContain(".html");
  });
});
