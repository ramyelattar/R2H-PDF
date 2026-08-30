//! Phase 29C — ToUnicode / Differences parsing foundation.
//!
//! This module provides honest analysis primitives for PDF ToUnicode
//! CMaps and simple-encoding Differences arrays. The goal is **not** to
//! enable native editing of arbitrary CID/Type0 fonts — that requires a
//! reverse glyph lookup which we deliberately do not implement here —
//! but to surface accurate diagnostics so the editor's safety decisions
//! are informed.
//!
//! Three things this module *does* do:
//!   1. Parse `bfchar` entries from a ToUnicode CMap stream
//!      (one-glyph-id ↔ one-Unicode-codepoint mappings).
//!   2. Parse `bfrange` entries (range of glyph IDs ↔ contiguous Unicode).
//!   3. Parse a `Differences` array into a (code → glyph_name) map.
//!
//! Three things this module deliberately does *not* do:
//!   1. Build a *reverse* mapping (Unicode → glyph ID) suitable for
//!      native editing of subset / Type0 fonts. The honest reason is
//!      that without the font program we cannot guarantee the writer
//!      will hit the right glyph.
//!   2. Replace `text_edit`'s encoding gate.
//!   3. Mutate any PDF.

use std::collections::BTreeMap;

/// One forward mapping entry from a ToUnicode CMap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToUnicodeEntry {
    /// CID / character code (8- or 16-bit numeric value from the CMap stream).
    pub code: u32,
    /// Unicode string this code maps to (typically one codepoint, but
    /// surrogate pairs and ligatures may yield more).
    pub unicode: String,
}

/// Summary of a parsed ToUnicode CMap.
#[derive(Debug, Clone, Default)]
pub struct ToUnicodeSummary {
    pub entries: Vec<ToUnicodeEntry>,
    pub bfchar_count: usize,
    pub bfrange_count: usize,
    pub parse_warnings: Vec<String>,
}

impl ToUnicodeSummary {
    /// Phase 29C — reverse-map a Unicode string back to a CMap code, if
    /// every character maps unambiguously. Used by the decision engine
    /// to decide whether a replacement could *in principle* be encoded.
    /// Returns `None` when the map is ambiguous or missing entries.
    pub fn reverse_lookup(&self, unicode: &str) -> Option<Vec<u32>> {
        let mut by_unicode: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for e in &self.entries {
            by_unicode.entry(e.unicode.clone()).or_default().push(e.code);
        }
        let mut out: Vec<u32> = Vec::new();
        for ch in unicode.chars() {
            let key = ch.to_string();
            let codes = by_unicode.get(&key)?;
            if codes.len() != 1 {
                return None; // ambiguous reverse mapping
            }
            out.push(codes[0]);
        }
        Some(out)
    }
}

/// Parse a ToUnicode CMap from its raw stream bytes. The parser is
/// deliberately permissive — malformed entries are skipped and recorded
/// in `parse_warnings` rather than failing the whole CMap.
pub fn parse_to_unicode_cmap(stream: &[u8]) -> ToUnicodeSummary {
    let text = String::from_utf8_lossy(stream).into_owned();
    let mut summary = ToUnicodeSummary::default();

    // Walk the text top-to-bottom, splitting into the bfchar / bfrange
    // sections. PDF CMaps have the rough shape:
    //   N beginbfchar
    //     <CODE> <UNICODE>
    //     ...
    //   endbfchar
    //   M beginbfrange
    //     <CODE_LOW> <CODE_HIGH> <UNICODE_START>
    //     <CODE_LOW> <CODE_HIGH> [<UC0> <UC1> ...]
    //     ...
    //   endbfrange
    //
    // We scan for the `begin*` ... `end*` blocks and parse them.
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if let Some(start) = find_keyword(bytes, cursor, b"beginbfchar") {
            let body_start = start + b"beginbfchar".len();
            let body_end = find_keyword(bytes, body_start, b"endbfchar").unwrap_or(bytes.len());
            let body = &text[body_start..body_end];
            parse_bfchar_body(body, &mut summary);
            summary.bfchar_count += 1;
            cursor = body_end + b"endbfchar".len();
            continue;
        }
        if let Some(start) = find_keyword(bytes, cursor, b"beginbfrange") {
            let body_start = start + b"beginbfrange".len();
            let body_end = find_keyword(bytes, body_start, b"endbfrange").unwrap_or(bytes.len());
            let body = &text[body_start..body_end];
            parse_bfrange_body(body, &mut summary);
            summary.bfrange_count += 1;
            cursor = body_end + b"endbfrange".len();
            continue;
        }
        break;
    }

    summary
}

fn find_keyword(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= bytes.len() || needle.is_empty() {
        return None;
    }
    bytes[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|i| i + from)
}

fn parse_bfchar_body(body: &str, summary: &mut ToUnicodeSummary) {
    // Each entry: <hex-code> <hex-unicode>
    let mut tokens = body
        .split(|c: char| c.is_whitespace())
        .filter(|s| !s.is_empty());
    while let (Some(code_tok), Some(uni_tok)) = (tokens.next(), tokens.next()) {
        let Some(code) = parse_pdf_hex_int(code_tok) else {
            summary.parse_warnings.push(format!("bfchar: bad code token {code_tok}"));
            continue;
        };
        let Some(unicode) = parse_pdf_hex_string(uni_tok) else {
            summary.parse_warnings.push(format!("bfchar: bad unicode token {uni_tok}"));
            continue;
        };
        summary.entries.push(ToUnicodeEntry { code, unicode });
    }
}

fn parse_bfrange_body(body: &str, summary: &mut ToUnicodeSummary) {
    // Two shapes per entry:
    //  <lo> <hi> <unicode_start>   — contiguous mapping
    //  <lo> <hi> [<u0> <u1> ...]   — explicit per-code list
    // We tokenize and decide based on what we see next.
    let mut iter = body
        .split(|c: char| c.is_whitespace())
        .filter(|s| !s.is_empty())
        .peekable();
    while let (Some(lo_tok), Some(hi_tok)) = (iter.next(), iter.next()) {
        let Some(lo) = parse_pdf_hex_int(lo_tok) else {
            summary.parse_warnings.push(format!("bfrange: bad lo {lo_tok}"));
            continue;
        };
        let Some(hi) = parse_pdf_hex_int(hi_tok) else {
            summary.parse_warnings.push(format!("bfrange: bad hi {hi_tok}"));
            continue;
        };
        if hi < lo {
            summary.parse_warnings.push("bfrange: hi < lo".to_string());
            continue;
        }
        let Some(next) = iter.next() else { break; };
        if next.starts_with('[') {
            // Read explicit list until ']' (may include the bracket inline).
            let mut list_tokens: Vec<String> = Vec::new();
            let mut first = next.trim_start_matches('[').to_string();
            if !first.is_empty() {
                list_tokens.push(first.clone());
                // If first contains ']' close inline.
            }
            // collect until ']' encountered
            if !next.ends_with(']') {
                while let Some(tok) = iter.next() {
                    if tok.ends_with(']') {
                        first = tok.trim_end_matches(']').to_string();
                        if !first.is_empty() {
                            list_tokens.push(first);
                        }
                        break;
                    }
                    list_tokens.push(tok.to_string());
                }
            }
            for (i, t) in list_tokens.iter().enumerate() {
                let code = lo + i as u32;
                if code > hi { break; }
                if let Some(unicode) = parse_pdf_hex_string(t) {
                    summary.entries.push(ToUnicodeEntry { code, unicode });
                } else {
                    summary.parse_warnings.push(format!("bfrange list: bad token {t}"));
                }
            }
        } else {
            // Contiguous range: starting unicode codepoint.
            let Some(start) = parse_pdf_hex_string_as_codepoint(next) else {
                summary.parse_warnings.push(format!("bfrange: bad start {next}"));
                continue;
            };
            for code in lo..=hi {
                let offset = (code - lo) as u32;
                let cp = start + offset;
                if let Some(ch) = char::from_u32(cp) {
                    summary.entries.push(ToUnicodeEntry {
                        code,
                        unicode: ch.to_string(),
                    });
                } else {
                    summary.parse_warnings.push(format!("bfrange: invalid codepoint {cp:#X}"));
                }
            }
        }
    }
}

/// Parse `<HHHH>` (PDF hex int) into u32. Accepts arbitrary hex length.
fn parse_pdf_hex_int(tok: &str) -> Option<u32> {
    let trimmed = tok.trim_matches(|c: char| c == '<' || c == '>');
    if trimmed.is_empty() { return None; }
    u32::from_str_radix(trimmed, 16).ok()
}

/// Parse `<HHHHHHHH>` (PDF hex string) into a UTF-16BE string.
fn parse_pdf_hex_string(tok: &str) -> Option<String> {
    let trimmed = tok.trim_matches(|c: char| c == '<' || c == '>');
    if trimmed.is_empty() { return None; }
    // Each pair of hex digits is one byte. UTF-16BE.
    let bytes: Vec<u8> = (0..trimmed.len())
        .step_by(2)
        .filter_map(|i| {
            let end = (i + 2).min(trimmed.len());
            u8::from_str_radix(&trimmed[i..end], 16).ok()
        })
        .collect();
    if bytes.len() % 2 != 0 || bytes.is_empty() {
        // 8-bit single-byte mapping — interpret as Latin-1.
        let chars: String = bytes.iter().map(|&b| b as char).collect();
        if chars.is_empty() { return None; }
        return Some(chars);
    }
    let u16s: Vec<u16> = bytes
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&u16s).ok()
}

fn parse_pdf_hex_string_as_codepoint(tok: &str) -> Option<u32> {
    let trimmed = tok.trim_matches(|c: char| c == '<' || c == '>');
    if trimmed.is_empty() { return None; }
    u32::from_str_radix(trimmed, 16).ok()
}

// ─── Differences array parsing ──────────────────────────────────────

/// One Differences entry — character code → glyph name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DifferencesEntry {
    pub code: u32,
    pub glyph_name: String,
}

/// Parse a PDF `Differences` array token list into structured entries.
/// The array shape is:
///   [ 32 /space /exclam ... 161 /aacute /agrave ... ]
/// — leading number resets the current code, subsequent names assign
/// to the running code which increments per name.
pub fn parse_differences_tokens(tokens: &[String]) -> Vec<DifferencesEntry> {
    let mut out = Vec::new();
    let mut code: u32 = 0;
    for t in tokens {
        if let Ok(n) = t.parse::<u32>() {
            code = n;
            continue;
        }
        let name = t.trim_start_matches('/').to_string();
        if name.is_empty() { continue; }
        out.push(DifferencesEntry { code, glyph_name: name });
        code = code.saturating_add(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bfchar_simple() {
        let stream = b"\
1 beginbfchar
<0041> <0041>
<0042> <0042>
<0043> <0043>
endbfchar
";
        let s = parse_to_unicode_cmap(stream);
        assert_eq!(s.bfchar_count, 1);
        assert_eq!(s.entries.len(), 3);
        assert_eq!(s.entries[0], ToUnicodeEntry { code: 0x41, unicode: "A".to_string() });
        assert!(s.parse_warnings.is_empty());
    }

    #[test]
    fn parse_bfrange_contiguous() {
        let stream = b"\
1 beginbfrange
<0030> <0039> <0030>
endbfrange
";
        let s = parse_to_unicode_cmap(stream);
        assert_eq!(s.bfrange_count, 1);
        // 0-9 — 10 entries.
        assert_eq!(s.entries.len(), 10);
        assert_eq!(s.entries[0].unicode, "0");
        assert_eq!(s.entries[9].unicode, "9");
    }

    #[test]
    fn parse_bfrange_explicit_list() {
        let stream = b"\
1 beginbfrange
<0061> <0063> [<0041> <0042> <0043>]
endbfrange
";
        let s = parse_to_unicode_cmap(stream);
        assert_eq!(s.entries.len(), 3);
        assert_eq!(s.entries[0], ToUnicodeEntry { code: 0x61, unicode: "A".to_string() });
        assert_eq!(s.entries[2], ToUnicodeEntry { code: 0x63, unicode: "C".to_string() });
    }

    #[test]
    fn malformed_tokens_do_not_panic() {
        let stream = b"\
1 beginbfchar
NOT_HEX <0041>
<0042> NOT_HEX
endbfchar
";
        let s = parse_to_unicode_cmap(stream);
        assert!(s.parse_warnings.len() >= 2);
        // No valid entries.
        assert!(s.entries.is_empty());
    }

    #[test]
    fn reverse_lookup_unambiguous_unicode_returns_codes() {
        let stream = b"\
1 beginbfchar
<0041> <0041>
<0042> <0042>
endbfchar
";
        let s = parse_to_unicode_cmap(stream);
        let codes = s.reverse_lookup("AB").unwrap();
        assert_eq!(codes, vec![0x41, 0x42]);
    }

    #[test]
    fn reverse_lookup_missing_char_returns_none() {
        let stream = b"\
1 beginbfchar
<0041> <0041>
endbfchar
";
        let s = parse_to_unicode_cmap(stream);
        assert!(s.reverse_lookup("AX").is_none());
    }

    #[test]
    fn reverse_lookup_ambiguous_returns_none() {
        let stream = b"\
1 beginbfchar
<0041> <0041>
<00C1> <0041>
endbfchar
";
        let s = parse_to_unicode_cmap(stream);
        // Two different codes both map to "A" — reverse is ambiguous.
        assert!(s.reverse_lookup("A").is_none());
    }

    #[test]
    fn parse_differences_array_simple() {
        // Equivalent to PDF: [ 32 /space 65 /A /B /C ]
        let toks = vec![
            "32".to_string(),
            "/space".to_string(),
            "65".to_string(),
            "/A".to_string(),
            "/B".to_string(),
            "/C".to_string(),
        ];
        let out = parse_differences_tokens(&toks);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], DifferencesEntry { code: 32, glyph_name: "space".to_string() });
        assert_eq!(out[1], DifferencesEntry { code: 65, glyph_name: "A".to_string() });
        assert_eq!(out[2], DifferencesEntry { code: 66, glyph_name: "B".to_string() });
        assert_eq!(out[3], DifferencesEntry { code: 67, glyph_name: "C".to_string() });
    }

    #[test]
    fn empty_cmap_returns_zero_entries() {
        let s = parse_to_unicode_cmap(b"");
        assert_eq!(s.entries.len(), 0);
        assert_eq!(s.bfchar_count, 0);
        assert_eq!(s.bfrange_count, 0);
    }

    #[test]
    fn parse_pdf_hex_int_handles_brackets() {
        assert_eq!(parse_pdf_hex_int("<00FF>"), Some(255));
        assert_eq!(parse_pdf_hex_int("0041"), Some(0x41));
    }

    #[test]
    fn parse_pdf_hex_string_returns_utf16be() {
        // <00C9> = U+00C9 = É
        let s = parse_pdf_hex_string("<00C9>").unwrap();
        assert_eq!(s, "É");
    }
}
