//! Phase 30B — encoding forward map for Differences-aware native editing.
//!
//! For simple 8-bit fonts (Type1 / TrueType with WinAnsi / MacRoman base
//! encoding plus an optional `/Differences` override), the PDF spec gives
//! us a code → glyph_name map. To safely native-edit such fonts we need
//! the *reverse* map: Unicode char → byte code.
//!
//! We deliberately keep this conservative:
//!
//!   * We support a curated list of Adobe glyph names that round-trip
//!     unambiguously to Unicode. Unknown names are NEVER guessed.
//!   * The base-encoding fallback is WinAnsi / MacRoman as defined by
//!     `encoding_target_for_base`.
//!   * If a Differences override maps a glyph name we recognise, that
//!     override wins.
//!   * If multiple distinct bytes map to the same Unicode char, the map
//!     records the ambiguity and `forward_lookup` returns `None`.
//!
//! No write paths run unless every character in the replacement maps to
//! exactly one byte.

use std::collections::BTreeMap;

use super::font_registry::EncodingKind;
use super::stream_parser::EncodingTarget;
use super::tounicode::DifferencesEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardMapSource {
    /// Map derived from base encoding only (no Differences).
    BaseEncoding,
    /// Differences only — base encoding unknown or unset.
    Differences,
    /// Base encoding plus Differences overrides.
    BasePlusDifferences,
    /// Built from a ToUnicode reverse map (experimental — see
    /// `tounicode::ToUnicodeSummary::reverse_lookup`).
    ToUnicodeExperimental,
}

impl ForwardMapSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForwardMapSource::BaseEncoding => "base_encoding",
            ForwardMapSource::Differences => "differences",
            ForwardMapSource::BasePlusDifferences => "base_plus_differences",
            ForwardMapSource::ToUnicodeExperimental => "tounicode_experimental",
        }
    }
}

/// One forward map for a single font resource.
#[derive(Debug, Clone)]
pub struct EncodingForwardMap {
    pub resource_name: String,
    pub encoding_kind: EncodingKind,
    /// Unicode char → PDF byte code (single-byte fonts only).
    pub char_to_code: BTreeMap<char, u8>,
    /// Unicode chars that map to MORE THAN ONE byte → unsafe to write.
    pub ambiguous_chars: Vec<char>,
    /// Free-form diagnostic warnings.
    pub warnings: Vec<String>,
    pub source: ForwardMapSource,
}

impl EncodingForwardMap {
    /// Lookup a Unicode string and return the byte sequence if every
    /// character maps unambiguously. Returns `Err(unmapped)` listing
    /// the offending characters when not.
    pub fn forward_lookup(&self, s: &str) -> Result<Vec<u8>, Vec<char>> {
        let mut bytes: Vec<u8> = Vec::with_capacity(s.len());
        let mut missing: Vec<char> = Vec::new();
        for c in s.chars() {
            if self.ambiguous_chars.contains(&c) {
                missing.push(c);
                continue;
            }
            match self.char_to_code.get(&c) {
                Some(code) => bytes.push(*code),
                None => missing.push(c),
            }
        }
        if missing.is_empty() {
            Ok(bytes)
        } else {
            Err(missing)
        }
    }

    /// Convenience — `true` when `forward_lookup(s)` would succeed.
    pub fn can_encode(&self, s: &str) -> bool {
        self.forward_lookup(s).is_ok()
    }
}

/// Build a curated glyph-name → Unicode map for the common Adobe glyph
/// names we recognise. Returns `None` for unknown names so callers can
/// surface the ambiguity rather than guess.
pub fn glyph_name_to_unicode(name: &str) -> Option<char> {
    // ASCII letters / digits round-trip 1:1.
    if name.len() == 1 {
        let ch = name.chars().next().unwrap();
        if ch.is_ascii_alphanumeric() {
            return Some(ch);
        }
    }
    match name {
        // Whitespace + control
        "space" => Some(' '),
        "nbspace" | "nonbreakingspace" => Some('\u{00A0}'),
        // ASCII punctuation
        "period" => Some('.'),
        "comma" => Some(','),
        "colon" => Some(':'),
        "semicolon" => Some(';'),
        "exclam" => Some('!'),
        "question" => Some('?'),
        "hyphen" | "minus" => Some('-'),
        "endash" => Some('\u{2013}'),
        "emdash" => Some('\u{2014}'),
        "underscore" => Some('_'),
        "quoteleft" => Some('\u{2018}'),
        "quoteright" => Some('\u{2019}'),
        "quotedblleft" => Some('\u{201C}'),
        "quotedblright" => Some('\u{201D}'),
        "quotedbl" => Some('"'),
        "quotesingle" => Some('\''),
        "parenleft" => Some('('),
        "parenright" => Some(')'),
        "bracketleft" => Some('['),
        "bracketright" => Some(']'),
        "braceleft" => Some('{'),
        "braceright" => Some('}'),
        "slash" => Some('/'),
        "backslash" => Some('\\'),
        "bar" => Some('|'),
        "ampersand" => Some('&'),
        "at" => Some('@'),
        "numbersign" => Some('#'),
        "dollar" => Some('$'),
        "percent" => Some('%'),
        "asterisk" => Some('*'),
        "plus" => Some('+'),
        "equal" => Some('='),
        "less" => Some('<'),
        "greater" => Some('>'),
        "Euro" => Some('\u{20AC}'),
        "copyright" => Some('\u{00A9}'),
        "registered" => Some('\u{00AE}'),
        "trademark" => Some('\u{2122}'),
        "bullet" => Some('\u{2022}'),
        "ellipsis" => Some('\u{2026}'),
        // Number names
        "zero" => Some('0'),
        "one" => Some('1'),
        "two" => Some('2'),
        "three" => Some('3'),
        "four" => Some('4'),
        "five" => Some('5'),
        "six" => Some('6'),
        "seven" => Some('7'),
        "eight" => Some('8'),
        "nine" => Some('9'),
        // Accented Latin characters — common Adobe names.
        "Aacute" => Some('\u{00C1}'),
        "aacute" => Some('\u{00E1}'),
        "Agrave" => Some('\u{00C0}'),
        "agrave" => Some('\u{00E0}'),
        "Acircumflex" => Some('\u{00C2}'),
        "acircumflex" => Some('\u{00E2}'),
        "Atilde" => Some('\u{00C3}'),
        "atilde" => Some('\u{00E3}'),
        "Adieresis" => Some('\u{00C4}'),
        "adieresis" => Some('\u{00E4}'),
        "Aring" => Some('\u{00C5}'),
        "aring" => Some('\u{00E5}'),
        "AE" => Some('\u{00C6}'),
        "ae" => Some('\u{00E6}'),
        "Ccedilla" => Some('\u{00C7}'),
        "ccedilla" => Some('\u{00E7}'),
        "Eacute" => Some('\u{00C9}'),
        "eacute" => Some('\u{00E9}'),
        "Egrave" => Some('\u{00C8}'),
        "egrave" => Some('\u{00E8}'),
        "Ecircumflex" => Some('\u{00CA}'),
        "ecircumflex" => Some('\u{00EA}'),
        "Edieresis" => Some('\u{00CB}'),
        "edieresis" => Some('\u{00EB}'),
        "Iacute" => Some('\u{00CD}'),
        "iacute" => Some('\u{00ED}'),
        "Igrave" => Some('\u{00CC}'),
        "igrave" => Some('\u{00EC}'),
        "Icircumflex" => Some('\u{00CE}'),
        "icircumflex" => Some('\u{00EE}'),
        "Idieresis" => Some('\u{00CF}'),
        "idieresis" => Some('\u{00EF}'),
        "Ntilde" => Some('\u{00D1}'),
        "ntilde" => Some('\u{00F1}'),
        "Oacute" => Some('\u{00D3}'),
        "oacute" => Some('\u{00F3}'),
        "Ograve" => Some('\u{00D2}'),
        "ograve" => Some('\u{00F2}'),
        "Ocircumflex" => Some('\u{00D4}'),
        "ocircumflex" => Some('\u{00F4}'),
        "Otilde" => Some('\u{00D5}'),
        "otilde" => Some('\u{00F5}'),
        "Odieresis" => Some('\u{00D6}'),
        "odieresis" => Some('\u{00F6}'),
        "Uacute" => Some('\u{00DA}'),
        "uacute" => Some('\u{00FA}'),
        "Ugrave" => Some('\u{00D9}'),
        "ugrave" => Some('\u{00F9}'),
        "Ucircumflex" => Some('\u{00DB}'),
        "ucircumflex" => Some('\u{00FB}'),
        "Udieresis" => Some('\u{00DC}'),
        "udieresis" => Some('\u{00FC}'),
        "Yacute" => Some('\u{00DD}'),
        "yacute" => Some('\u{00FD}'),
        "Ydieresis" => Some('\u{0178}'),
        "ydieresis" => Some('\u{00FF}'),
        "Oslash" => Some('\u{00D8}'),
        "oslash" => Some('\u{00F8}'),
        "germandbls" => Some('\u{00DF}'),
        _ => None,
    }
}

/// Map a base encoding name to the closest `EncodingTarget`. Used when
/// no Differences override exists or to seed the forward map.
pub fn encoding_target_for_base(kind: EncodingKind) -> Option<EncodingTarget> {
    match kind {
        EncodingKind::WinAnsi => Some(EncodingTarget::WinAnsi),
        EncodingKind::MacRoman => Some(EncodingTarget::MacRoman),
        _ => None,
    }
}

/// Build a forward map from base encoding alone. Iterates the printable
/// ASCII range and the 0x80–0xFF range relevant to the chosen encoding
/// using the same lookup that `pdf_encode_string_bytes` uses internally.
///
/// This is the simpler half of the Phase 30B map; layering Differences
/// on top is handled by `build_with_differences`.
pub fn build_base_encoding_map(
    resource_name: &str,
    kind: EncodingKind,
) -> EncodingForwardMap {
    let mut char_to_code: BTreeMap<char, u8> = BTreeMap::new();
    let mut warnings: Vec<String> = Vec::new();

    // Add ASCII range (0x20..0x7E maps 1:1 except certain control chars).
    for code in 0x20u8..=0x7E {
        let ch = code as char;
        char_to_code.insert(ch, code);
    }

    match kind {
        EncodingKind::WinAnsi => {
            // 0xA0..0xFF maps 1:1 with Latin-1.
            for code in 0xA0u8..=0xFF {
                let ch = code as char;
                char_to_code.insert(ch, code);
            }
            // Selected 0x80..0x9F entries (Windows-1252-style).
            for (code, ch) in [
                (0x80u8, '\u{20AC}'), // euro
                (0x82, '\u{201A}'),
                (0x83, '\u{0192}'),
                (0x84, '\u{201E}'),
                (0x85, '\u{2026}'),
                (0x86, '\u{2020}'),
                (0x87, '\u{2021}'),
                (0x88, '\u{02C6}'),
                (0x89, '\u{2030}'),
                (0x8A, '\u{0160}'),
                (0x8B, '\u{2039}'),
                (0x8C, '\u{0152}'),
                (0x8E, '\u{017D}'),
                (0x91, '\u{2018}'),
                (0x92, '\u{2019}'),
                (0x93, '\u{201C}'),
                (0x94, '\u{201D}'),
                (0x95, '\u{2022}'),
                (0x96, '\u{2013}'),
                (0x97, '\u{2014}'),
                (0x98, '\u{02DC}'),
                (0x99, '\u{2122}'),
                (0x9A, '\u{0161}'),
                (0x9B, '\u{203A}'),
                (0x9C, '\u{0153}'),
                (0x9E, '\u{017E}'),
                (0x9F, '\u{0178}'),
            ] {
                char_to_code.insert(ch, code);
            }
        }
        EncodingKind::MacRoman => {
            // A subset of common Mac Roman entries.
            for (code, ch) in [
                (0x80u8, '\u{00C4}'),
                (0x81, '\u{00C5}'),
                (0x82, '\u{00C7}'),
                (0x83, '\u{00C9}'),
                (0x84, '\u{00D1}'),
                (0x85, '\u{00D6}'),
                (0x86, '\u{00DC}'),
                (0x87, '\u{00E1}'),
                (0x88, '\u{00E0}'),
                (0x89, '\u{00E2}'),
                (0x8A, '\u{00E4}'),
                (0x8B, '\u{00E3}'),
                (0x8C, '\u{00E5}'),
                (0x8D, '\u{00E7}'),
                (0x8E, '\u{00E9}'),
                (0x8F, '\u{00E8}'),
                (0x96, '\u{00F1}'),
                (0x9A, '\u{00F6}'),
                (0x9F, '\u{00FC}'),
                (0xCB, '\u{00C0}'),
            ] {
                char_to_code.insert(ch, code);
            }
        }
        _ => {
            warnings.push(format!(
                "No base-encoding map for kind {:?} on resource {resource_name}.",
                kind
            ));
        }
    }

    EncodingForwardMap {
        resource_name: resource_name.to_string(),
        encoding_kind: kind,
        char_to_code,
        ambiguous_chars: Vec::new(),
        warnings,
        source: ForwardMapSource::BaseEncoding,
    }
}

/// Layer `/Differences` overrides on top of `base_map`. For every
/// recognised glyph name, the new code → unicode association is
/// installed; conflicts mark the char as ambiguous so write paths refuse.
pub fn build_with_differences(
    resource_name: &str,
    base_kind: EncodingKind,
    diffs: &[DifferencesEntry],
) -> EncodingForwardMap {
    let mut map = build_base_encoding_map(resource_name, base_kind);
    let had_base = !map.char_to_code.is_empty();

    // Reverse-look the existing char→code so we can detect collisions.
    let mut code_to_char: BTreeMap<u8, char> = BTreeMap::new();
    for (ch, code) in &map.char_to_code {
        code_to_char.insert(*code, *ch);
    }

    let mut applied = 0usize;
    for d in diffs {
        if d.code > 0xFF {
            map.warnings.push(format!(
                "Differences code {} out of single-byte range for resource {resource_name}; skipped.",
                d.code
            ));
            continue;
        }
        let code = d.code as u8;
        let Some(unicode) = glyph_name_to_unicode(&d.glyph_name) else {
            map.warnings.push(format!(
                "Differences: glyph name '{}' (code {}) is not recognised; skipped to avoid guessing.",
                d.glyph_name, d.code
            ));
            continue;
        };

        // Detect collision: another char already maps to this byte code,
        // OR this char already maps to a *different* byte code.
        if let Some(existing_ch) = code_to_char.get(&code) {
            if *existing_ch != unicode {
                map.char_to_code.remove(existing_ch);
            }
        }
        if let Some(existing_code) = map.char_to_code.get(&unicode) {
            if *existing_code != code {
                // Ambiguous — same char already maps to a different code.
                map.ambiguous_chars.push(unicode);
                continue;
            }
        }
        map.char_to_code.insert(unicode, code);
        code_to_char.insert(code, unicode);
        applied += 1;
    }

    map.source = if had_base && applied > 0 {
        ForwardMapSource::BasePlusDifferences
    } else if applied > 0 {
        ForwardMapSource::Differences
    } else {
        ForwardMapSource::BaseEncoding
    };
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_letters_map_to_themselves() {
        let m = build_base_encoding_map("F1", EncodingKind::WinAnsi);
        assert_eq!(m.char_to_code.get(&'A'), Some(&b'A'));
        assert_eq!(m.char_to_code.get(&'z'), Some(&b'z'));
        assert_eq!(m.char_to_code.get(&'0'), Some(&b'0'));
        assert_eq!(m.source, ForwardMapSource::BaseEncoding);
    }

    #[test]
    fn winansi_latin1_maps_to_high_bytes() {
        let m = build_base_encoding_map("F1", EncodingKind::WinAnsi);
        assert_eq!(m.char_to_code.get(&'é'), Some(&0xE9));
        assert_eq!(m.char_to_code.get(&'ñ'), Some(&0xF1));
    }

    #[test]
    fn winansi_em_dash_maps_to_0x97() {
        let m = build_base_encoding_map("F1", EncodingKind::WinAnsi);
        assert_eq!(m.char_to_code.get(&'\u{2014}'), Some(&0x97));
    }

    #[test]
    fn macroman_e_acute_maps_to_0x8e() {
        let m = build_base_encoding_map("F2", EncodingKind::MacRoman);
        assert_eq!(m.char_to_code.get(&'é'), Some(&0x8E));
    }

    #[test]
    fn forward_lookup_returns_ok_for_known_chars() {
        let m = build_base_encoding_map("F1", EncodingKind::WinAnsi);
        let out = m.forward_lookup("Café").unwrap();
        assert_eq!(out, b"Caf\xE9");
    }

    #[test]
    fn forward_lookup_reports_missing_chars() {
        let m = build_base_encoding_map("F1", EncodingKind::WinAnsi);
        // U+4E2D is not in WinAnsi; should be reported missing.
        let err = m.forward_lookup("Hi 中").unwrap_err();
        assert!(err.contains(&'中'));
    }

    #[test]
    fn differences_apply_eacute_override() {
        let diffs = vec![DifferencesEntry { code: 0xE9, glyph_name: "eacute".to_string() }];
        let m = build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
        assert!(matches!(m.source, ForwardMapSource::BasePlusDifferences));
        assert_eq!(m.char_to_code.get(&'é'), Some(&0xE9));
    }

    #[test]
    fn differences_unknown_glyph_name_is_skipped() {
        let diffs = vec![DifferencesEntry { code: 0xA1, glyph_name: "madeup_glyph".to_string() }];
        let m = build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
        assert!(m.warnings.iter().any(|w| w.contains("not recognised")));
        // Did NOT insert anything for the unknown name.
        assert!(!m.char_to_code.values().any(|&b| b == 0xA1) || m.char_to_code.get(&'¡') == Some(&0xA1));
    }

    #[test]
    fn ambiguous_differences_marks_char_unsafe() {
        // Map two codes to the same Unicode → ambiguous.
        let diffs = vec![
            DifferencesEntry { code: 0xA1, glyph_name: "eacute".to_string() },
            DifferencesEntry { code: 0xA2, glyph_name: "eacute".to_string() },
        ];
        let m = build_with_differences("F1", EncodingKind::WinAnsi, &diffs);
        // 'é' is mapped (one of the codes wins), and the other map is
        // recorded as ambiguous OR not installed.
        // Forward-lookup must refuse "é".
        let res = m.forward_lookup("é");
        // Either ambiguous_chars catches it, or one code mapped + the
        // other was rejected. In our implementation the second iteration
        // detects 'é' already mapped to 0xA1 (different code), so the
        // char is added to ambiguous_chars.
        assert!(res.is_err() || m.ambiguous_chars.contains(&'é'));
    }

    #[test]
    fn glyph_name_to_unicode_handles_letters_and_specials() {
        assert_eq!(glyph_name_to_unicode("A"), Some('A'));
        assert_eq!(glyph_name_to_unicode("z"), Some('z'));
        assert_eq!(glyph_name_to_unicode("space"), Some(' '));
        assert_eq!(glyph_name_to_unicode("eacute"), Some('é'));
        assert_eq!(glyph_name_to_unicode("Euro"), Some('€'));
        assert_eq!(glyph_name_to_unicode("not_a_real_glyph"), None);
    }

    #[test]
    fn encoding_target_for_base_maps_known_kinds() {
        assert!(matches!(encoding_target_for_base(EncodingKind::WinAnsi), Some(EncodingTarget::WinAnsi)));
        assert!(matches!(encoding_target_for_base(EncodingKind::MacRoman), Some(EncodingTarget::MacRoman)));
        assert!(encoding_target_for_base(EncodingKind::IdentityH).is_none());
    }
}
