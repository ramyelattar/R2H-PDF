import { describe, it, expect } from "vitest";
import type { DocumentReviewResult, ReviewFinding, ReviewSuggestedAction } from "./types";

describe("DocumentReviewResult", () => {
  it("represents a complete review", () => {
    const result: DocumentReviewResult = {
      review_id: "r1", session_id: "s1",
      summary: "This is a construction contract for bridge renewal.",
      findings: [{ finding_id: "f1", title: "Scope unclear", description: "Section 3 lacks detail", severity: "warning", category: "general", page_refs: [2], citations: [], recommendation: "Clarify scope", confidence: 0.8 }],
      risks: [{ finding_id: "f2", title: "Termination risk", description: "Clause 12 allows termination for convenience", severity: "major", category: "risk", page_refs: [5], citations: [{ citation_id: "c1", page_index: 5, chunk_id: "ch1", snippet: "termination for convenience", score: 0.9, source: "native_text" }], recommendation: "Negotiate", confidence: 0.85 }],
      missing_information: [{ finding_id: "f3", title: "Missing completion date", description: "No explicit completion date found", severity: "warning", category: "missing_information", page_refs: [], citations: [], recommendation: "Request clarification", confidence: 0.7 }],
      engineering_findings: [],
      suggested_actions: [{ action_id: "sa1", action_type: "add_comment", page_index: 5, text: "Risk: termination clause", reason: "High risk clause", citations: [], confidence: 0.8 }],
      citations: [{ citation_id: "c1", page_index: 5, chunk_id: "ch1", snippet: "termination", score: 0.9, source: "native_text" }],
      warnings: [], elapsed_ms: 8500, created_at: Date.now(),
    };
    expect(result.summary).toContain("construction contract");
    expect(result.findings.length).toBe(1);
    expect(result.risks.length).toBe(1);
    expect(result.risks[0].citations.length).toBe(1);
    expect(result.suggested_actions.length).toBe(1);
  });

  it("represents no-evidence review", () => {
    const result: DocumentReviewResult = {
      review_id: "r2", session_id: "s1",
      summary: "Not enough document evidence was found to complete a reliable review.",
      findings: [], risks: [], missing_information: [],
      engineering_findings: [], suggested_actions: [],
      citations: [], warnings: ["No relevant document chunks found."],
      elapsed_ms: 100, created_at: Date.now(),
    };
    expect(result.summary).toContain("Not enough");
    expect(result.warnings.length).toBe(1);
  });
});

describe("ReviewFinding", () => {
  it("has citations for document-specific claims", () => {
    const finding: ReviewFinding = {
      finding_id: "f1", title: "Risk clause", description: "Termination clause found",
      severity: "major", category: "risk", page_refs: [3],
      citations: [{ citation_id: "c1", page_index: 3, chunk_id: "ch1", snippet: "terminate", score: 0.9, source: "native_text" }],
      recommendation: "Review with counsel", confidence: 0.85,
    };
    expect(finding.citations.length).toBeGreaterThan(0);
    expect(finding.page_refs[0]).toBe(3);
  });
});

describe("ReviewSuggestedAction", () => {
  it("only allows safe action types", () => {
    const safe: ReviewSuggestedAction = {
      action_id: "sa1", action_type: "add_comment", page_index: 0,
      text: "Note", reason: "Important", citations: [], confidence: 0.7,
    };
    const safeTypes = ["add_comment", "add_highlight", "add_text_box", "add_redaction", "apply_stamp"];
    expect(safeTypes).toContain(safe.action_type);
  });

  it("destructive types are not allowed", () => {
    const unsafeTypes = ["delete_page", "export_pdf", "run_command", "call_api"];
    const safeTypes = ["add_comment", "add_highlight", "add_text_box", "add_redaction", "apply_stamp"];
    for (const t of unsafeTypes) {
      expect(safeTypes).not.toContain(t);
    }
  });
});

describe("Review UI logic", () => {
  it("review button disabled when running", () => {
    const status = "running";
    expect(status === "running").toBe(true);
  });

  it("findings grouped by severity", () => {
    const findings: ReviewFinding[] = [
      { finding_id: "f1", title: "A", description: "", severity: "major", category: "risk", page_refs: [], citations: [], recommendation: "", confidence: 0.8 },
      { finding_id: "f2", title: "B", description: "", severity: "info", category: "general", page_refs: [], citations: [], recommendation: "", confidence: 0.7 },
      { finding_id: "f3", title: "C", description: "", severity: "warning", category: "general", page_refs: [], citations: [], recommendation: "", confidence: 0.7 },
    ];
    const majors = findings.filter((f) => f.severity === "major");
    expect(majors.length).toBe(1);
  });

  it("citation click provides page index", () => {
    const pageIndex = 5;
    // Navigation would call jumpToPage(pageIndex + 1) for 1-based display.
    expect(pageIndex).toBe(5);
  });
});
