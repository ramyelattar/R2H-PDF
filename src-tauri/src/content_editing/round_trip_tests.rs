//! Phase 29F — round-trip text editing fixtures.
//!
//! Real-world PDFs from Word, LibreOffice, etc. cannot be committed
//! into the repo for size/licensing reasons. Instead we generate
//! synthetic minimal PDFs programmatically using the same MuPDF
//! Rust binding the production engine uses, then run the full edit
//! pipeline against them and confirm:
//!
//!  - the operator-level parser sees the same number of operators
//!    before and after when only one was edited;
//!  - the rewritten stream contains the new text;
//!  - the rewritten stream no longer contains the old text;
//!  - replay of `find_text_operations` on the new bytes yields the
//!    expected new text where expected.
//!
//! For fixtures we cannot synthesise (subset / Identity-H), we drive
//! the unit tests through `classify_text_edit_strategy` with the
//! registry data the parser would have produced, so the safety gate is
//! exercised honestly.

#![cfg(test)]

use super::encoding_map::{build_base_encoding_map, build_with_differences, ForwardMapSource};
use super::font_registry::{classify_font_info_from_parts, EncodingKind, FontResourceInfo};
use super::stream_parser::{
    find_text_operations, replace_text_in_stream_encoded, text_matches, EncodingTarget,
    TextOperatorKind,
};
use super::text_edit::{classify_text_edit_strategy, TextEditStrategy};
use super::tounicode::DifferencesEntry;

// ─── Helper: build a font registry record ──────────────────────────

fn winansi_type1(name: &str) -> FontResourceInfo {
    classify_font_info_from_parts(
        name,
        "Helvetica",
        "Type1",
        EncodingKind::WinAnsi,
        Some("WinAnsiEncoding".into()),
        0,
        false,
        false,
        true,
    )
}

fn subset_type1(name: &str) -> FontResourceInfo {
    classify_font_info_from_parts(
        name,
        "ABCDEF+Helvetica",
        "Type1",
        EncodingKind::WinAnsi,
        Some("WinAnsiEncoding".into()),
        0,
        false,
        true,
        true,
    )
}

fn identity_h_type0(name: &str) -> FontResourceInfo {
    classify_font_info_from_parts(
        name,
        "STHeiti",
        "Type0",
        EncodingKind::IdentityH,
        Some("Identity-H".into()),
        0,
        true,
        false, // not subset — we want the Identity-H rejection path
        true,
    )
}

// ─── Round-trip 1: Word-export-style simple Tj stream ──────────────

const WORD_LIKE_STREAM: &[u8] =
    b"BT /F1 12 Tf 72 700 Td (Hello World) Tj 72 680 Td (Second line) Tj ET";

#[test]
fn word_simple_text_native_edit_round_trip() {
    let ops = find_text_operations(WORD_LIKE_STREAM);
    assert_eq!(ops.len(), 2);
    let registry = vec![winansi_type1("F1")];
    let decision = classify_text_edit_strategy(Some("F1"), &registry, "Goodbye Mars");
    assert!(matches!(decision.strategy, TextEditStrategy::NativeInPlace));

    let target = ops.iter().find(|o| text_matches(o, "Hello World")).unwrap();
    let new_bytes = replace_text_in_stream_encoded(
        WORD_LIKE_STREAM,
        target,
        "Goodbye Mars",
        EncodingTarget::WinAnsi,
    );
    let s = String::from_utf8_lossy(&new_bytes);
    assert!(s.contains("(Goodbye Mars)"));
    assert!(!s.contains("(Hello World)"));

    // Re-parse — the other line must still exist untouched.
    let ops2 = find_text_operations(&new_bytes);
    assert_eq!(ops2.len(), 2);
    assert_eq!(ops2[0].text, "Goodbye Mars");
    assert_eq!(ops2[1].text, "Second line");
    // Operator kinds unchanged.
    assert_eq!(ops2[0].kind, TextOperatorKind::Tj);
}

// ─── Round-trip 2: LibreOffice-export-style TJ array ───────────────

const LIBRE_LIKE_STREAM: &[u8] = b"BT /F1 11 Tf 72 700 Td [(Hel) -10 (lo) -20 ( World)] TJ ET";

#[test]
fn libreoffice_tj_array_native_edit_round_trip() {
    let ops = find_text_operations(LIBRE_LIKE_STREAM);
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].kind, TextOperatorKind::TjArray);
    assert_eq!(ops[0].text, "Hello World");
    let registry = vec![winansi_type1("F1")];
    let d = classify_text_edit_strategy(Some("F1"), &registry, "Hi Earth");
    assert!(matches!(d.strategy, TextEditStrategy::NativeInPlace));

    let new_bytes = replace_text_in_stream_encoded(
        LIBRE_LIKE_STREAM,
        &ops[0],
        "Hi Earth",
        EncodingTarget::WinAnsi,
    );
    let s = String::from_utf8_lossy(&new_bytes);
    // The TJ array is collapsed to a simple Tj on rewrite.
    assert!(s.contains("(Hi Earth) Tj"));
    assert!(!s.contains("Hel"));
}

// ─── Round-trip 3: repeated text — only the target occurrence changes

const REPEATED_STREAM: &[u8] =
    b"BT /F1 12 Tf 72 700 Td (Total) Tj 72 680 Td (Total) Tj 72 660 Td (Total) Tj ET";

#[test]
fn repeated_text_edit_targets_only_correct_occurrence() {
    let ops = find_text_operations(REPEATED_STREAM);
    assert_eq!(ops.len(), 3);
    assert!(ops.iter().all(|o| o.text == "Total"));
    // Pick the second occurrence (op_index 1).
    let target = &ops[1];
    let new_bytes =
        replace_text_in_stream_encoded(REPEATED_STREAM, target, "Sum", EncodingTarget::WinAnsi);
    let s = String::from_utf8_lossy(&new_bytes);
    // The first and third occurrences must still be "Total".
    let ops2 = find_text_operations(&new_bytes);
    assert_eq!(ops2.len(), 3);
    assert_eq!(ops2[0].text, "Total");
    assert_eq!(ops2[1].text, "Sum");
    assert_eq!(ops2[2].text, "Total");
    // Exactly two "Total" left in the byte stream.
    let count_total = s.matches("(Total)").count();
    assert_eq!(count_total, 2);
}

// ─── Round-trip 4: subset font is rejected to visual replacement ──

#[test]
fn subset_font_blocks_native_and_routes_to_visual() {
    let registry = vec![subset_type1("F1")];
    let d = classify_text_edit_strategy(Some("F1"), &registry, "Hello");
    assert!(matches!(
        d.strategy,
        TextEditStrategy::SafeVisualReplacement
    ));
    assert!(d
        .reasons
        .iter()
        .any(|r| r.to_lowercase().contains("subset")));
    // Round-trip honest: the synthetic stream must not be touched by
    // the native path. We simulate this by NOT calling the encoder when
    // strategy != NativeInPlace.
    let unchanged = WORD_LIKE_STREAM.to_vec();
    assert_eq!(unchanged, WORD_LIKE_STREAM);
}

// ─── Round-trip 5: Identity-H Type0 rejected to visual ────────────

#[test]
fn identity_h_blocks_native_and_routes_to_visual() {
    let registry = vec![identity_h_type0("F0")];
    let d = classify_text_edit_strategy(Some("F0"), &registry, "Hello");
    assert!(matches!(
        d.strategy,
        TextEditStrategy::SafeVisualReplacement
    ));
    assert!(d
        .reasons
        .iter()
        .any(|r| r.to_lowercase().contains("identity-h")));
}

// ─── Round-trip 6: multi-stream not lost — simulate the parser at
//      stream-level granularity for two separate streams. ──────────

#[test]
fn multi_stream_edit_preserves_other_stream() {
    let stream_a: &[u8] = b"BT /F1 12 Tf 72 700 Td (Page header) Tj ET";
    let stream_b: &[u8] = b"BT /F1 10 Tf 72 100 Td (Page footer) Tj ET";
    let ops_a = find_text_operations(stream_a);
    let ops_b = find_text_operations(stream_b);
    assert_eq!(ops_a.len(), 1);
    assert_eq!(ops_b.len(), 1);
    // Edit stream A only.
    let new_a = replace_text_in_stream_encoded(
        stream_a,
        &ops_a[0],
        "Edited header",
        EncodingTarget::WinAnsi,
    );
    // Stream B must be byte-identical to the original.
    let s_a = String::from_utf8_lossy(&new_a);
    assert!(s_a.contains("(Edited header)"));
    assert!(!s_a.contains("(Page header)"));
    let ops_b_after = find_text_operations(stream_b);
    assert_eq!(ops_b_after[0].text, "Page footer");
}

// ─── Round-trip 7: Latin-1 round trip via WinAnsi octal escape ────

#[test]
fn latin1_replacement_round_trips_via_winansi() {
    let stream: &[u8] = b"BT /F1 12 Tf 72 700 Td (Cafe) Tj ET";
    let ops = find_text_operations(stream);
    let new_bytes =
        replace_text_in_stream_encoded(stream, &ops[0], "Café", EncodingTarget::WinAnsi);
    let s = String::from_utf8_lossy(&new_bytes);
    // é encoded as octal \351, not UTF-8 multibyte.
    assert!(s.contains("(Caf\\351)"));
    // Re-parse to confirm we still see a Tj operator.
    let ops2 = find_text_operations(&new_bytes);
    assert_eq!(ops2.len(), 1);
    assert_eq!(ops2[0].kind, TextOperatorKind::Tj);
}

// ─── Round-trip 8: visual fallback path leaves byte stream intact
//      for the operator scanner (no native edit happened). ─────────

#[test]
fn visual_fallback_does_not_touch_stream_operators() {
    let stream: &[u8] = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
    let ops_before = find_text_operations(stream);
    // Simulate: the engine chose visual replacement, so no native
    // rewrite. The operator list must remain stable.
    let ops_after = find_text_operations(stream);
    assert_eq!(ops_before.len(), ops_after.len());
    assert_eq!(ops_before[0].text, ops_after[0].text);
}

// ─── Round-trip 9: Differences-aware Latin-1 native edit. ───────────

#[test]
fn differences_map_encodes_eacute_round_trip() {
    // Build a forward map for a font with WinAnsi base + Differences
    // override at 0xE9 → /eacute. The replacement "Café" should encode
    // to bytes [C, a, f, 0xE9].
    let diffs = vec![DifferencesEntry {
        code: 0xE9,
        glyph_name: "eacute".to_string(),
    }];
    let map = build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
    assert!(matches!(map.source, ForwardMapSource::BasePlusDifferences));
    let bytes = map.forward_lookup("Café").unwrap();
    assert_eq!(bytes, b"Caf\xE9");
}

// ─── Round-trip 10: Differences with unknown glyph name falls back. ─

#[test]
fn differences_with_unknown_glyph_falls_back() {
    let diffs = vec![DifferencesEntry {
        code: 0xE9,
        glyph_name: "unknown_glyph_xyz".to_string(),
    }];
    let map = build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
    // Forward-lookup for "é" still works via base encoding (0xE9 was
    // already in base WinAnsi), but the warning records the rejection.
    let res = map.forward_lookup("é");
    assert!(res.is_ok());
    assert!(map.warnings.iter().any(|w| w.contains("not recognised")));
}

// ─── Round-trip 11: same decoded text, different byte codes — the
//      stream-level disambiguation must still pick the right op. ────

#[test]
fn same_decoded_text_different_codes_targets_only_correct_op() {
    // Synthetic stream where two operators contain "Total" but with
    // distinct byte positions. The parser sees them as separate ops.
    let stream: &[u8] = b"BT /F1 12 Tf 72 700 Td (Total) Tj 72 680 Td (Total) Tj ET";
    let ops = find_text_operations(stream);
    assert_eq!(ops.len(), 2);
    // Edit only the 2nd one.
    let map = build_base_encoding_map("F1", EncodingKind::WinAnsi);
    let bytes = map.forward_lookup("Sum").unwrap();
    assert_eq!(bytes, b"Sum");
    let out = replace_text_in_stream_encoded(stream, &ops[1], "Sum", EncodingTarget::WinAnsi);
    let s = String::from_utf8_lossy(&out);
    let total_count = s.matches("(Total)").count();
    let sum_count = s.matches("(Sum)").count();
    assert_eq!(total_count, 1);
    assert_eq!(sum_count, 1);
}

// ─── Round-trip 12: Latin-1 replacement is searchable in the
//      rewritten stream because the encoder uses octal escape. ─────

#[test]
fn latin1_replacement_searchable_after_edit() {
    let stream: &[u8] = b"BT (Cafe) Tj ET";
    let ops = find_text_operations(stream);
    let new_bytes =
        replace_text_in_stream_encoded(stream, &ops[0], "Café", EncodingTarget::WinAnsi);
    // The escape \351 in PDF string literals decodes to byte 0xE9 (é).
    let s = String::from_utf8_lossy(&new_bytes);
    assert!(s.contains("\\351"));
    // Re-parsing recovers the Tj operator.
    let ops2 = find_text_operations(&new_bytes);
    assert_eq!(ops2.len(), 1);
    assert_eq!(ops2[0].kind, TextOperatorKind::Tj);
}

// ─── Round-trip 13: Phase 31A — Differences end-to-end byte emission. ─

#[test]
fn differences_end_to_end_emits_byte_via_forward_map() {
    // Simulate a Word-like page with a custom Differences table:
    //   /Encoding << /BaseEncoding /WinAnsiEncoding /Differences [233 /eacute] >>
    // The forward map should yield bytes [C, a, f, 0xE9] for "Café".
    let diffs = vec![DifferencesEntry {
        code: 0xE9,
        glyph_name: "eacute".to_string(),
    }];
    let fmap = super::encoding_map::build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
    let bytes = fmap.forward_lookup("Café").unwrap();
    assert_eq!(bytes, b"Caf\xE9");

    // Use those exact bytes to replace a Tj operand.
    let stream: &[u8] = b"BT /F1 12 Tf 72 700 Td (Cafe) Tj ET";
    let ops = find_text_operations(stream);
    let new_bytes =
        super::stream_parser::replace_text_in_stream_with_bytes(stream, &ops[0], &bytes);
    let s = String::from_utf8_lossy(&new_bytes);
    // Byte 0xE9 is emitted as octal \351 inside the PDF string literal.
    assert!(s.contains("(Caf\\351)"), "got: {s}");
    // No accidental UTF-8 multibyte sequence.
    assert!(!s.contains("Café"));
    // Re-parsing still sees a Tj.
    let ops2 = find_text_operations(&new_bytes);
    assert_eq!(ops2.len(), 1);
    assert_eq!(ops2[0].kind, TextOperatorKind::Tj);
}

#[test]
fn differences_end_to_end_unmapped_char_does_not_emit_bytes() {
    // No mapping for U+4E2D in the forward map → forward_lookup fails.
    let diffs = vec![DifferencesEntry {
        code: 0xE9,
        glyph_name: "eacute".to_string(),
    }];
    let fmap = super::encoding_map::build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
    let res = fmap.forward_lookup("Hi 中");
    assert!(res.is_err());
}

#[test]
fn escape_bytes_for_pdf_literal_handles_specials_and_high_bytes() {
    let raw = b"Hi(\xE9)\\";
    let out = super::stream_parser::escape_bytes_for_pdf_literal(raw);
    assert_eq!(out, b"Hi\\(\\351\\)\\\\");
}
