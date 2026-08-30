import { describe, it, expect } from "vitest";
import { resolveTextLabel, truncate } from "./textLabel";
import type { ContentObject } from "../../lib/ipc";

const makeObj = (decoded: string, quality?: "ok" | "partial" | "garbled"): ContentObject => ({
  id: "t1",
  session_id: "doc-session-test",
  page_index: 0,
  object_type: "text_span",
  bbox: [10, 10, 100, 30],
  z_index: 0,
  editable_level: "native_editable",
  text_info: {
    raw_text: decoded,
    decoded_text: decoded,
    glyph_count: decoded.length,
    font_name: "Helvetica",
    font_size: 12,
    fill_color: null,
    stroke_color: null,
    writing_mode: "horizontal",
    is_subset_font: false,
    encoding_safe: true,
    operator_offset: null,
    operator_length: null,
    occurrence_index: 0,
    operator_type: "tj",
    editable_strategy: "native_in_place",
    font_encoding: "WinAnsiEncoding",
    decoding_quality: quality,
  },
  image_info: null,
  style_info: null,
  diagnostics: [],
});

describe("resolveTextLabel — honest garbled-text fallback", () => {
  it("returns the decoded string when it looks clean", () => {
    const r = resolveTextLabel(makeObj("Hello world"));
    expect(r.label).toBe("Hello world");
    expect(r.garbled).toBe(false);
    expect(r.partial).toBe(false);
  });

  it("trusts the backend's explicit garbled flag and substitutes a fallback", () => {
    // The decoded glyphs might LOOK fine but the backend knew the font's
    // ToUnicode was unreliable. We must NEVER show the raw string in this
    // case — the source characters are unknown.
    const r = resolveTextLabel(makeObj("Definitely not really this", "garbled"));
    expect(r.label).toBe("Text object");
    expect(r.garbled).toBe(true);
  });

  it("substitutes a fallback when the heuristic finds mostly U+FFFD glyphs", () => {
    const r = resolveTextLabel(makeObj("��������"));
    expect(r.label).toBe("Text object");
    expect(r.garbled).toBe(true);
  });

  it("substitutes a fallback for a Private-Use-Area dump", () => {
    // Unmapped glyph indices often surface as the PUA range.
    const r = resolveTextLabel(makeObj(""));
    expect(r.label).toBe("Text object");
    expect(r.garbled).toBe(true);
  });

  it("keeps the label when only one bad glyph appears in a long string", () => {
    const r = resolveTextLabel(makeObj("Hello world from the encoder �"));
    expect(r.label.startsWith("Hello world")).toBe(true);
    expect(r.garbled).toBe(false);
    expect(r.partial).toBe(true);
  });

  it("returns a fallback for an empty decoded string", () => {
    const r = resolveTextLabel(makeObj(""));
    expect(r.label).toBe("Text object");
    expect(r.garbled).toBe(false);
  });

  it("counts surrogate-pair characters as single glyphs", () => {
    // An emoji must not be misclassified as bad even though it spans
    // two UTF-16 code units.
    const r = resolveTextLabel(makeObj("😃 hi"));
    expect(r.garbled).toBe(false);
  });
});

describe("truncate", () => {
  it("returns the string unchanged when short enough", () => {
    expect(truncate("abc", 5)).toBe("abc");
  });

  it("appends an ellipsis when truncating", () => {
    expect(truncate("abcdefghij", 5)).toBe("abcde…");
  });
});
