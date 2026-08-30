import type { ContentObject } from "../../lib/ipc";

/**
 * Honest label resolution for a text content object.
 *
 * Background: when MuPDF can't decode a font's encoding (no usable
 * ToUnicode table, no standard Encoding mapping, custom /Differences
 * with no name remap), its text-page extraction silently returns
 * U+FFFD or Private-Use-Area characters. Showing those in the side
 * panel as "the text you're editing" is hostile UX — the user sees
 * a row of replacement characters and assumes the whole app is broken.
 * The backend already marks these objects with
 * `decoding_quality === "garbled"`; this helper substitutes a
 * neutral, honest fallback.
 *
 * Returns the original decoded text when usable, otherwise a short
 * "Text object" placeholder. A `garbled` flag is also returned so the
 * caller can render a warning chip / change the row's tone.
 */
export interface TextLabel {
  /** Text to render as the primary label. Never garbage. */
  label: string;
  /** True when the decoded glyph stream is too damaged to trust. */
  garbled: boolean;
  /** True when only some glyphs failed but the overall string is usable. */
  partial: boolean;
}

/** Threshold above which a string is treated as garbage instead of text. */
const GARBLED_RATIO = 0.2;

function isBadGlyph(cp: number): boolean {
  // U+FFFD REPLACEMENT CHARACTER is the smoking gun for a broken
  // ToUnicode / encoding decode. The C1 control range and Private
  // Use Area also count: those are what shows up when a font uses
  // unmapped glyph indices.
  if (cp === 0xfffd) return true;
  if (cp >= 0x0080 && cp <= 0x009f) return true;
  if (cp >= 0xe000 && cp <= 0xf8ff) return true;
  return false;
}

/**
 * Count bad glyphs in a string, working over Unicode code points rather
 * than UTF-16 code units so surrogate pairs (e.g. emoji) count once.
 */
function countBadGlyphs(s: string): { total: number; bad: number } {
  let total = 0;
  let bad = 0;
  for (const c of s) {
    total += 1;
    const cp = c.codePointAt(0) ?? 0;
    if (isBadGlyph(cp)) bad += 1;
  }
  return { total, bad };
}

export function resolveTextLabel(obj: ContentObject): TextLabel {
  const ti = obj.text_info;
  if (!ti) return { label: "Text object", garbled: false, partial: false };

  // Prefer the backend's explicit assessment when present.
  const quality = ti.decoding_quality;
  if (quality === "garbled") {
    return { label: "Text object", garbled: true, partial: false };
  }

  // Fallback heuristic: even for older backends that don't ship the
  // `decoding_quality` field, refuse to render a label that is mostly
  // replacement characters. This is also the safety net for any
  // future regression in the backend assessment.
  const raw = ti.decoded_text ?? "";
  const { total, bad } = countBadGlyphs(raw);
  if (total === 0) {
    return { label: "Text object", garbled: false, partial: false };
  }
  if (bad / total >= GARBLED_RATIO) {
    return { label: "Text object", garbled: true, partial: false };
  }

  return {
    label: raw,
    garbled: false,
    partial: quality === "partial" || bad > 0,
  };
}

/** Truncate-with-ellipsis helper for list rows. */
export function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "…";
}
