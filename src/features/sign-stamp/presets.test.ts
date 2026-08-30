import { describe, it, expect } from "vitest";
import { STAMP_PRESETS, composeStampText } from "./presets";

describe("STAMP_PRESETS", () => {
  it("includes the full Phase 25C preset list", () => {
    const ids = STAMP_PRESETS.map((p) => p.id);
    for (const expected of [
      "APPROVED", "APPROVED_AS_NOTED", "REVIEWED", "REJECTED",
      "DRAFT", "FOR_CONSTRUCTION", "AS_BUILT", "REVISE_AND_RESUBMIT",
      "CONFIDENTIAL", "VOID", "CUSTOM",
    ]) {
      expect(ids).toContain(expected);
    }
  });

  it("each preset carries a non-empty label and color", () => {
    for (const p of STAMP_PRESETS) {
      expect(p.label.length).toBeGreaterThan(0);
      expect(p.color).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });
});

describe("composeStampText", () => {
  it("falls back to baseText when customText is empty", () => {
    const t = composeStampText({ baseText: "APPROVED" });
    expect(t).toBe("APPROVED");
  });

  it("uses customText when provided (CUSTOM preset path)", () => {
    const t = composeStampText({ baseText: "CUSTOM", customText: "Reviewer initial: JD" });
    expect(t).toBe("Reviewer initial: JD");
  });

  it("appends today's date in ISO format when includeDate is true", () => {
    const t = composeStampText({ baseText: "REVIEWED", includeDate: true });
    expect(t.startsWith("REVIEWED\n")).toBe(true);
    expect(t).toMatch(/\n\d{4}-\d{2}-\d{2}$/);
  });

  it("appends author on its own line", () => {
    const t = composeStampText({ baseText: "DRAFT", author: "Alice" });
    expect(t).toBe("DRAFT\nAlice");
  });

  it("combines date + author in the right order", () => {
    const t = composeStampText({
      baseText: "APPROVED", includeDate: true, author: "QA",
    });
    const lines = t.split("\n");
    expect(lines[0]).toBe("APPROVED");
    expect(lines[1]).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(lines[2]).toBe("QA");
  });

  it("trims author whitespace", () => {
    const t = composeStampText({ baseText: "APPROVED", author: "  Bob  " });
    expect(t).toBe("APPROVED\nBob");
  });
});
