import { describe, it, expect } from "vitest";
import { applyAiActionsToEditorObjects } from "./actionToEditorObject";
import type { AiProposedAction } from "./types";
import type { CommentObject, HighlightObject, RedactionObject, StampObject, TextBoxObject } from "../pdf-editor/types";

function makeAction(overrides: Partial<AiProposedAction> = {}): AiProposedAction {
  return {
    action_id: "a1",
    action_type: "add_comment",
    session_id: "s1",
    page_index: 0,
    rect: { x: 72, y: 700, width: 200, height: 30 },
    text: "Risk identified",
    reason: "Termination clause",
    confidence: 0.85,
    source_citations: [{ citation_id: "c1", page_index: 0, chunk_id: "ch1", snippet: "termination", score: 0.9, source: "native_text" }],
    status: "accepted",
    created_at: Date.now(),
    ...overrides,
  };
}

describe("applyAiActionsToEditorObjects", () => {
  it("maps add_highlight to HighlightObject", () => {
    const action = makeAction({ action_type: "add_highlight", action_id: "h1" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(1);
    const obj = applied[0] as HighlightObject;
    expect(obj.type).toBe("highlight");
    expect(obj.opacity).toBe(0.35);
    expect(obj.pageIndex).toBe(0);
    expect(obj.metadata.ai).toBe(true);
  });

  it("maps add_comment to CommentObject", () => {
    const action = makeAction({ action_type: "add_comment", action_id: "c1" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(1);
    const obj = applied[0] as CommentObject;
    expect(obj.type).toBe("comment");
    expect(obj.contents).toBe("Risk identified");
    expect(obj.author).toBe("R2H Local AI");
    expect(obj.status).toBe("open");
  });

  it("maps add_text_box to TextBoxObject", () => {
    const action = makeAction({ action_type: "add_text_box", action_id: "t1", text: "Important note" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    const obj = applied[0] as TextBoxObject;
    expect(obj.type).toBe("textBox");
    expect(obj.text).toBe("Important note");
    expect(obj.fontSize).toBe(11);
  });

  it("maps add_redaction as draft (never burns in)", () => {
    const action = makeAction({ action_type: "add_redaction", action_id: "r1", reason: "PII" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    const obj = applied[0] as RedactionObject;
    expect(obj.type).toBe("redaction");
    expect(obj.status).toBe("draft");
    expect(obj.reason).toBe("PII");
  });

  it("maps apply_stamp to StampObject", () => {
    const action = makeAction({ action_type: "apply_stamp", action_id: "st1", text: "APPROVED" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    const obj = applied[0] as StampObject;
    expect(obj.type).toBe("stamp");
    expect(obj.stampText).toBe("APPROVED");
    expect(obj.stampType).toBe("APPROVED");
  });

  it("skips rejected actions", () => {
    const action = makeAction({ status: "rejected" });
    const { applied, skipped } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(0);
    expect(skipped.length).toBe(1);
    expect(skipped[0].reason).toContain("rejected");
  });

  it("skips actions from wrong session", () => {
    const action = makeAction({ session_id: "other-session" });
    const { applied, skipped } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(0);
    expect(skipped[0].reason).toContain("does not match");
  });

  it("skips actions with invalid rect", () => {
    const action = makeAction({ rect: { x: 72, y: 700, width: -10, height: 30 } });
    const { applied, skipped } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(0);
    expect(skipped[0].reason).toContain("Invalid rect");
  });

  it("skips actions with out-of-bounds page", () => {
    const action = makeAction({ page_index: 99 });
    const { applied, skipped } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied.length).toBe(0);
    expect(skipped[0].reason).toContain("out of bounds");
  });

  it("preserves citations in metadata", () => {
    const action = makeAction({ action_type: "add_highlight" });
    const { applied } = applyAiActionsToEditorObjects([action], "s1", 10);
    expect(applied[0].metadata.citations).toBeDefined();
    expect((applied[0].metadata.citations as unknown[]).length).toBe(1);
  });

  it("handles multiple actions in batch", () => {
    const actions = [
      makeAction({ action_id: "a1", action_type: "add_highlight", page_index: 0 }),
      makeAction({ action_id: "a2", action_type: "add_comment", page_index: 1 }),
      makeAction({ action_id: "a3", action_type: "add_redaction", page_index: 2 }),
    ];
    const { applied } = applyAiActionsToEditorObjects(actions, "s1", 10);
    expect(applied.length).toBe(3);
    expect(applied[0].type).toBe("highlight");
    expect(applied[1].type).toBe("comment");
    expect(applied[2].type).toBe("redaction");
  });
});
