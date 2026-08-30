import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

vi.mock("./useDocumentReview", () => ({
  useDocumentReview: () => ({
    status: "completed",
    error: null,
    reviewDocument: vi.fn(),
    clearReview: vi.fn(),
    result: {
      review_id: "r1",
      session_id: "doc-session-test",
      summary: "Review summary",
      findings: [
        {
          finding_id: "f1",
          title: "Voltage drop",
          description: "Voltage drop exceeds the target.",
          severity: "major",
          category: "electrical",
          page_refs: [0],
          citations: [],
          recommendation: "Resize feeder.",
          confidence: 0.9,
        },
      ],
      risks: [
        {
          finding_id: "f2",
          title: "Missing breaker rating",
          description: "Breaker rating is not visible.",
          severity: "critical",
          category: "safety",
          page_refs: [1],
          citations: [],
          recommendation: "Confirm rating.",
          confidence: 0.86,
        },
      ],
      missing_information: [],
      engineering_findings: [],
      suggested_actions: [
        {
          action_id: "a1",
          action_type: "add_comment",
          page_index: 0,
          text: "Check voltage drop",
          reason: "Needs review",
          citations: [],
          confidence: 0.8,
        },
      ],
      citations: [],
      warnings: ["Model confidence varies by page."],
      elapsed_ms: 1200,
      created_at: 0,
    },
  }),
}));

import { ReviewPanel } from "./ReviewPanel";

describe("Pass 2B — ReviewPanel polish", () => {
  it("renders the primary review action, grouped findings, warning callout, and apply action", () => {
    render(<ReviewPanel sessionId="doc-session-test" onApplySuggestedActions={() => {}} />);

    expect(screen.getByTestId("review-primary-action").textContent).toMatch(/Review Document/);
    expect(screen.getByTestId("review-severity-critical").textContent).toMatch(/Missing breaker rating/);
    expect(screen.getByTestId("review-severity-major").textContent).toMatch(/Voltage drop/);
    expect(screen.getByText(/Model confidence varies/)).toBeTruthy();
    expect(screen.getByTestId("review-apply-actions").textContent).toMatch(/Apply All Suggested Actions/);
  });
});
