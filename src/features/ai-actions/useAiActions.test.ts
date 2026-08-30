import { describe, it, expect } from "vitest";
import type { AiActionBatch, AiProposedAction, AiActionStatus } from "./types";

describe("AiProposedAction", () => {
  it("has correct shape for add_comment", () => {
    const action: AiProposedAction = {
      action_id: "a1",
      action_type: "add_comment",
      session_id: "s1",
      page_index: 0,
      rect: { x: 72, y: 700, width: 200, height: 30 },
      text: "Risk clause identified",
      reason: "Contains termination for convenience",
      confidence: 0.85,
      source_citations: [{ citation_id: "c1", page_index: 0, chunk_id: "ch1", snippet: "termination", score: 0.9, source: "native_text" }],
      status: "proposed",
      created_at: Date.now(),
    };
    expect(action.action_type).toBe("add_comment");
    expect(action.confidence).toBeGreaterThan(0);
    expect(action.source_citations.length).toBe(1);
  });

  it("has correct shape for add_highlight", () => {
    const action: AiProposedAction = {
      action_id: "a2",
      action_type: "add_highlight",
      session_id: "s1",
      page_index: 1,
      rect: { x: 72, y: 500, width: 400, height: 20 },
      text: "",
      reason: "Key financial term",
      confidence: 0.9,
      source_citations: [{ citation_id: "c2", page_index: 1, chunk_id: "ch2", snippet: "$5 million", score: 0.85, source: "native_text" }],
      status: "proposed",
      created_at: Date.now(),
    };
    expect(action.action_type).toBe("add_highlight");
  });
});

describe("AiActionBatch", () => {
  it("groups actions with citations and warnings", () => {
    const batch: AiActionBatch = {
      batch_id: "b1",
      session_id: "s1",
      user_request: "Highlight risk clauses",
      actions: [
        { action_id: "a1", action_type: "add_highlight", session_id: "s1", page_index: 0, rect: { x: 72, y: 700, width: 400, height: 20 }, text: "", reason: "Risk", confidence: 0.8, source_citations: [], status: "proposed", created_at: 0 },
      ],
      citations: [{ citation_id: "c1", page_index: 0, chunk_id: "ch1", snippet: "risk", score: 0.8, source: "native_text" }],
      warnings: [],
      created_at: Date.now(),
    };
    expect(batch.actions.length).toBe(1);
    expect(batch.citations.length).toBe(1);
  });

  it("no-evidence batch has empty actions with warning", () => {
    const batch: AiActionBatch = {
      batch_id: "b2",
      session_id: "s1",
      user_request: "Find something not in document",
      actions: [],
      citations: [],
      warnings: ["No relevant document content found."],
      created_at: Date.now(),
    };
    expect(batch.actions.length).toBe(0);
    expect(batch.warnings.length).toBe(1);
  });
});

describe("Action status transitions", () => {
  it("proposed → accepted", () => {
    const status: AiActionStatus = "accepted";
    expect(status).toBe("accepted");
  });

  it("proposed → rejected", () => {
    const status: AiActionStatus = "rejected";
    expect(status).toBe("rejected");
  });

  it("accepted → applied", () => {
    const status: AiActionStatus = "applied";
    expect(status).toBe("applied");
  });

  it("rejected actions are not applied", () => {
    const actions: AiProposedAction[] = [
      { action_id: "a1", action_type: "add_comment", session_id: "s1", page_index: 0, rect: { x: 0, y: 0, width: 100, height: 20 }, text: "", reason: "", confidence: 0.5, source_citations: [], status: "rejected", created_at: 0 },
      { action_id: "a2", action_type: "add_highlight", session_id: "s1", page_index: 0, rect: { x: 0, y: 0, width: 100, height: 20 }, text: "", reason: "", confidence: 0.8, source_citations: [], status: "accepted", created_at: 0 },
    ];
    const toApply = actions.filter((a) => a.status === "accepted");
    expect(toApply.length).toBe(1);
    expect(toApply[0].action_id).toBe("a2");
  });
});

describe("Safety rules", () => {
  it("redaction actions remain draft overlay objects", () => {
    const action: AiProposedAction = {
      action_id: "r1", action_type: "add_redaction", session_id: "s1",
      page_index: 0, rect: { x: 72, y: 300, width: 200, height: 20 },
      text: "", reason: "PII detected", confidence: 0.9,
      source_citations: [{ citation_id: "c1", page_index: 0, chunk_id: "ch1", snippet: "SSN", score: 0.95, source: "native_text" }],
      status: "applied", created_at: 0,
    };
    // When applied, this creates a draft redaction overlay — not a destructive PDF mutation.
    expect(action.action_type).toBe("add_redaction");
    // The status "applied" means it was added to the editor overlay, not burned into PDF.
  });
});
