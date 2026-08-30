//! PDF content stream parser for text operator identification and replacement.
//! Parses the raw content stream bytes to locate Tj/TJ/'/" operators and
//! their string operands, including TJ arrays with kerning.

/// PDF text-showing operator kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextOperatorKind {
    /// `(...) Tj` — show a string with the current text state.
    Tj,
    /// `[(...) k (...) ...] TJ` — show an array of strings + kerning glue.
    TjArray,
    /// `(...) '` — move to next line and show string. Equivalent to `T* (...) Tj`.
    SingleQuote,
    /// `aw ac (...) "` — move to next line, set word/char spacing, show string.
    DoubleQuote,
    /// `<hex> Tj` — hex-encoded glyph IDs (not safely native-editable).
    Hex,
}

impl TextOperatorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TextOperatorKind::Tj => "tj",
            TextOperatorKind::TjArray => "tj_array",
            TextOperatorKind::SingleQuote => "single_quote",
            TextOperatorKind::DoubleQuote => "double_quote",
            TextOperatorKind::Hex => "hex",
        }
    }
}

/// A located text operation in the content stream.
#[derive(Debug, Clone)]
pub struct TextOperation {
    /// The decoded text string from the operator.
    pub text: String,
    /// Byte offset of the string operand start (including the opening paren/angle) in the stream.
    pub operand_start: usize,
    /// Byte offset of the string operand end (including the closing paren/angle) in the stream.
    pub operand_end: usize,
    /// The font name set before this text operation (from Tf operator).
    pub font_name: Option<String>,
    /// The font size set before this text operation.
    pub font_size: Option<f32>,
    /// Whether this text uses hex encoding (glyph IDs, not readable text).
    /// Hex-encoded text cannot be natively edited via simple string replacement.
    pub is_hex_encoded: bool,
    /// Phase 28B — explicit operator kind for the UI and the editor.
    pub kind: TextOperatorKind,
    /// Phase 28B — index of this operation within the stream (0-based).
    pub op_index: usize,
}

/// Find all text operations in a content stream.
/// Returns text operations with their byte offsets for in-place replacement.
pub fn find_text_operations(stream: &[u8]) -> Vec<TextOperation> {
    let mut ops = Vec::new();
    let mut i = 0;
    let mut current_font: Option<String> = None;
    let mut current_size: Option<f32> = None;

    while i < stream.len() {
        // Skip whitespace.
        if stream[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Look for Tf operator (font selection): /FontName size Tf
        if i + 2 < stream.len() && stream[i] == b'/' {
            // Parse font name.
            let name_start = i + 1;
            let mut j = name_start;
            while j < stream.len() && !stream[j].is_ascii_whitespace() && stream[j] != b'/' && stream[j] != b'(' && stream[j] != b'[' {
                j += 1;
            }
            let font_name = String::from_utf8_lossy(&stream[name_start..j]).to_string();

            // Look ahead for "size Tf" pattern.
            let mut k = j;
            while k < stream.len() && stream[k].is_ascii_whitespace() { k += 1; }
            // Parse number.
            let num_start = k;
            while k < stream.len() && (stream[k].is_ascii_digit() || stream[k] == b'.' || stream[k] == b'-') { k += 1; }
            if k > num_start {
                let num_str = String::from_utf8_lossy(&stream[num_start..k]).to_string();
                // Look for "Tf" after the number.
                let mut m = k;
                while m < stream.len() && stream[m].is_ascii_whitespace() { m += 1; }
                if m + 1 < stream.len() && stream[m] == b'T' && stream[m + 1] == b'f' {
                    current_font = Some(font_name);
                    current_size = num_str.parse().ok();
                    i = m + 2;
                    continue;
                }
            }
            i = j;
            continue;
        }

        // Look for string operand: (text)
        if stream[i] == b'(' {
            let str_start = i;
            if let Some((text, str_end)) = parse_pdf_string(stream, i) {
                // Look ahead for Tj, ' or " operators.
                let mut k = str_end;
                while k < stream.len() && stream[k].is_ascii_whitespace() { k += 1; }
                // ' (single quote) is a one-byte operator.
                if k < stream.len() && stream[k] == b'\'' {
                    let op_index = ops.len();
                    ops.push(TextOperation {
                        text,
                        operand_start: str_start,
                        operand_end: str_end,
                        font_name: current_font.clone(),
                        font_size: current_size,
                        is_hex_encoded: false,
                        kind: TextOperatorKind::SingleQuote,
                        op_index,
                    });
                    i = k + 1;
                    continue;
                }
                // " (double quote) is a one-byte operator with two
                // additional numeric operands before the string —
                // detecting the operator after the string is enough.
                if k < stream.len() && stream[k] == b'"' {
                    let op_index = ops.len();
                    ops.push(TextOperation {
                        text,
                        operand_start: str_start,
                        operand_end: str_end,
                        font_name: current_font.clone(),
                        font_size: current_size,
                        is_hex_encoded: false,
                        kind: TextOperatorKind::DoubleQuote,
                        op_index,
                    });
                    i = k + 1;
                    continue;
                }
                if k + 1 < stream.len() && stream[k] == b'T' && stream[k + 1] == b'j' {
                    let op_index = ops.len();
                    ops.push(TextOperation {
                        text,
                        operand_start: str_start,
                        operand_end: str_end,
                        font_name: current_font.clone(),
                        font_size: current_size,
                        is_hex_encoded: false,
                        kind: TextOperatorKind::Tj,
                        op_index,
                    });
                    i = k + 2;
                    continue;
                }
                i = str_end;
                continue;
            }
            i += 1;
            continue;
        }

        // Look for hex string operand: <hex> Tj
        if stream[i] == b'<' && (i + 1 >= stream.len() || stream[i + 1] != b'<') {
            let hex_start = i;
            let mut j = i + 1;
            while j < stream.len() && stream[j] != b'>' { j += 1; }
            if j < stream.len() {
                let hex_end = j + 1; // past the '>'
                let hex_content = &stream[i + 1..j];
                // Decode hex to get raw bytes (glyph IDs).
                let decoded = decode_hex_string(hex_content);
                // Look ahead for Tj operator.
                let mut k = hex_end;
                while k < stream.len() && stream[k].is_ascii_whitespace() { k += 1; }
                if k + 1 < stream.len() && stream[k] == b'T' && stream[k + 1] == b'j' {
                    // Hex-encoded text — store as hex representation.
                    let text = String::from_utf8(decoded.clone())
                        .unwrap_or_else(|_| decoded.iter().map(|&b| b as char).collect());
                    let op_index = ops.len();
                    ops.push(TextOperation {
                        text,
                        operand_start: hex_start,
                        operand_end: hex_end,
                        font_name: current_font.clone(),
                        font_size: current_size,
                        is_hex_encoded: true,
                        kind: TextOperatorKind::Hex,
                        op_index,
                    });
                    i = k + 2;
                    continue;
                }
                i = hex_end;
                continue;
            }
            i += 1;
            continue;
        }

        // Look for TJ array: [(string) kern (string) kern ...] TJ
        if stream[i] == b'[' {
            // Parse TJ array — extract concatenated text.
            let array_start = i;
            let mut j = i + 1;
            let mut array_text = String::new();
            let mut first_str_start: Option<usize> = None;
            let mut last_str_end = j;

            while j < stream.len() && stream[j] != b']' {
                if stream[j] == b'(' {
                    if first_str_start.is_none() {
                        first_str_start = Some(j);
                    }
                    if let Some((s, end)) = parse_pdf_string(stream, j) {
                        array_text.push_str(&s);
                        last_str_end = end;
                        j = end;
                        continue;
                    }
                }
                j += 1;
            }

            if j < stream.len() && stream[j] == b']' {
                j += 1; // skip ]
                // Look for TJ.
                let mut k = j;
                while k < stream.len() && stream[k].is_ascii_whitespace() { k += 1; }
                if k + 1 < stream.len() && stream[k] == b'T' && stream[k + 1] == b'J' {
                    if !array_text.is_empty() {
                        let op_index = ops.len();
                        ops.push(TextOperation {
                            text: array_text,
                            operand_start: array_start,
                            operand_end: k + 2, // Include the TJ operator
                            font_name: current_font.clone(),
                            font_size: current_size,
                            is_hex_encoded: false,
                            kind: TextOperatorKind::TjArray,
                            op_index,
                        });
                    }
                    i = k + 2;
                    continue;
                }
            }
            i = j.max(i + 1);
            continue;
        }

        i += 1;
    }

    ops
}

/// Decode a hex-encoded PDF string body (the bytes between `<` and `>`).
/// Whitespace is ignored. An odd trailing nibble is treated as `0`.
fn decode_hex_string(hex: &[u8]) -> Vec<u8> {
    let mut nibbles: Vec<u8> = Vec::with_capacity(hex.len());
    for &b in hex {
        if b.is_ascii_whitespace() { continue; }
        let n = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => continue,
        };
        nibbles.push(n);
    }
    if nibbles.len() % 2 == 1 { nibbles.push(0); }
    nibbles
        .chunks(2)
        .map(|pair| (pair[0] << 4) | pair[1])
        .collect()
}

/// Parse a PDF string literal starting at position `start` (which should be '(').
/// Returns the decoded string and the byte position after the closing ')'.
fn parse_pdf_string(stream: &[u8], start: usize) -> Option<(String, usize)> {
    if start >= stream.len() || stream[start] != b'(' {
        return None;
    }

    let mut result = Vec::new();
    let mut i = start + 1;
    let mut depth = 1;

    while i < stream.len() && depth > 0 {
        match stream[i] {
            b'(' => { depth += 1; result.push(b'('); }
            b')' => {
                depth -= 1;
                if depth > 0 { result.push(b')'); }
            }
            b'\\' => {
                i += 1;
                if i >= stream.len() { break; }
                match stream[i] {
                    b'n' => result.push(b'\n'),
                    b'r' => result.push(b'\r'),
                    b't' => result.push(b'\t'),
                    b'(' => result.push(b'('),
                    b')' => result.push(b')'),
                    b'\\' => result.push(b'\\'),
                    b'0'..=b'7' => {
                        // Octal escape.
                        let mut octal = (stream[i] - b'0') as u32;
                        if i + 1 < stream.len() && stream[i + 1] >= b'0' && stream[i + 1] <= b'7' {
                            i += 1;
                            octal = octal * 8 + (stream[i] - b'0') as u32;
                            if i + 1 < stream.len() && stream[i + 1] >= b'0' && stream[i + 1] <= b'7' {
                                i += 1;
                                octal = octal * 8 + (stream[i] - b'0') as u32;
                            }
                        }
                        result.push(octal as u8);
                    }
                    other => result.push(other),
                }
            }
            other => result.push(other),
        }
        i += 1;
    }

    if depth != 0 {
        return None; // Unbalanced parens.
    }

    // Try to decode as UTF-8, fall back to Latin-1.
    let text = String::from_utf8(result.clone())
        .unwrap_or_else(|_| result.iter().map(|&b| b as char).collect());

    Some((text, i))
}

/// Replace a text operation's string operand in the content stream.
/// Returns the modified stream bytes.
pub fn replace_text_in_stream(
    stream: &[u8],
    op: &TextOperation,
    replacement: &str,
) -> Vec<u8> {
    // Default: assume WinAnsi-compatible 8-bit encoding, which is the
    // safest single-byte target for ASCII + Latin-1 text. Non-encodable
    // characters fall back to ASCII-only encoding (every other char is
    // stripped) — callers that need a stricter check should use
    // `replace_text_in_stream_encoded` directly.
    replace_text_in_stream_encoded(stream, op, replacement, EncodingTarget::WinAnsi)
}

/// Encoding target for the new string operand. Phase 29D: choose this
/// based on the source font's encoding so non-ASCII Latin-1 characters
/// round-trip as one byte, not as UTF-8 multibyte sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingTarget {
    /// Pure 7-bit ASCII. Any non-ASCII char is replaced with `?`.
    Ascii,
    /// WinAnsiEncoding — codepoints 0x20–0xFF mapped 1:1 (close enough
    /// for the common cases we native-edit).
    WinAnsi,
    /// MacRomanEncoding — codepoints 0x20–0xFF mapped 1:1 via Mac table.
    MacRoman,
}

/// Phase 29D — encoding-aware version that takes an explicit target.
/// Phase 31A — replace a text operation using **explicit byte mapping**.
/// The caller supplies the byte sequence that should appear inside the
/// PDF string literal; we only escape PDF-special characters and emit
/// any non-printable bytes (0x00–0x1F, 0x7F–0xFF) as octal escapes so
/// the new operand is round-trip-safe through any PDF parser.
///
/// This is what `EncodingForwardMap::forward_lookup`'s output should
/// flow through when a `/Differences` table dictates an exact byte
/// sequence — the byte map is authoritative; we do not re-translate.
pub fn replace_text_in_stream_with_bytes(
    stream: &[u8],
    op: &TextOperation,
    bytes: &[u8],
) -> Vec<u8> {
    let encoded = escape_bytes_for_pdf_literal(bytes);
    let mut result = Vec::with_capacity(stream.len() + encoded.len());
    let is_tj_array = stream[op.operand_start] == b'[';
    if is_tj_array {
        result.extend_from_slice(&stream[..op.operand_start]);
        result.push(b'(');
        result.extend_from_slice(&encoded);
        result.extend_from_slice(b") Tj");
        result.extend_from_slice(&stream[op.operand_end..]);
    } else {
        result.extend_from_slice(&stream[..op.operand_start]);
        result.push(b'(');
        result.extend_from_slice(&encoded);
        result.push(b')');
        result.extend_from_slice(&stream[op.operand_end..]);
    }
    result
}

/// Phase 31A — escape a raw byte sequence into a PDF string-literal-safe
/// byte sequence. Printable ASCII passes through (with the usual
/// parens/backslash escapes); everything else becomes octal `\NNN`.
pub fn escape_bytes_for_pdf_literal(bytes: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    for &b in bytes {
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            0x20..=0x7E => out.push(b),
            other => {
                out.extend_from_slice(format!("\\{:03o}", other).as_bytes());
            }
        }
    }
    out
}

pub fn replace_text_in_stream_encoded(
    stream: &[u8],
    op: &TextOperation,
    replacement: &str,
    target: EncodingTarget,
) -> Vec<u8> {
    let encoded = pdf_encode_string_bytes(replacement, target);
    let mut result = Vec::with_capacity(stream.len() + encoded.len());

    // Check if this is a simple Tj or a TJ array.
    let is_tj_array = stream[op.operand_start] == b'[';

    if is_tj_array {
        // For TJ arrays, replace the entire array+operator with a simple (text) Tj.
        result.extend_from_slice(&stream[..op.operand_start]);
        result.push(b'(');
        result.extend_from_slice(&encoded);
        result.extend_from_slice(b") Tj");
        result.extend_from_slice(&stream[op.operand_end..]);
    } else {
        // For simple Tj, replace just the string operand.
        result.extend_from_slice(&stream[..op.operand_start]);
        result.push(b'(');
        result.extend_from_slice(&encoded);
        result.push(b')');
        result.extend_from_slice(&stream[op.operand_end..]);
    }

    result
}

/// Phase 29D — encode a Rust string into a PDF string-literal byte
/// sequence for the requested encoding target. Special characters are
/// escaped per the PDF spec. Returns the raw bytes (not a `String`),
/// because non-ASCII characters round-trip as their single-byte 8-bit
/// code, NOT as UTF-8 — that's the whole point.
pub fn pdf_encode_string_bytes(s: &str, target: EncodingTarget) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.extend_from_slice(b"\\\\"),
            '(' => out.extend_from_slice(b"\\("),
            ')' => out.extend_from_slice(b"\\)"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\r' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                // Control char — emit as octal escape.
                out.extend_from_slice(format!("\\{:03o}", c as u32).as_bytes());
            }
            c if (c as u32) < 0x80 => {
                // ASCII — emit as-is.
                out.push(c as u8);
            }
            c => {
                // Non-ASCII path: choose by target encoding.
                if let Some(byte) = encode_non_ascii_char(c, target) {
                    // Use octal escape (\NNN) so the byte is unambiguous
                    // inside a `(...)` PDF string literal. This avoids
                    // accidentally interacting with any 8-bit char that
                    // PDF parsers might treat specially.
                    out.extend_from_slice(format!("\\{:03o}", byte).as_bytes());
                } else {
                    // No mapping for this target: emit '?' so we don't
                    // silently corrupt the stream.
                    out.push(b'?');
                }
            }
        }
    }
    out
}

fn encode_non_ascii_char(ch: char, target: EncodingTarget) -> Option<u8> {
    match target {
        EncodingTarget::Ascii => None,
        EncodingTarget::WinAnsi => {
            // For U+00A0..U+00FF the WinAnsi table coincides with Latin-1.
            // For U+0080..U+009F there are well-known WinAnsi mappings
            // (€, ‚, ƒ, …, etc.) but they are rarely used in editing;
            // we list the common ones explicitly.
            match ch as u32 {
                0x00A0..=0x00FF => Some(ch as u32 as u8),
                0x2022 => Some(0x95), // bullet
                0x2013 => Some(0x96), // en dash
                0x2014 => Some(0x97), // em dash
                0x2018 => Some(0x91), // left single quote
                0x2019 => Some(0x92), // right single quote
                0x201C => Some(0x93), // left double quote
                0x201D => Some(0x94), // right double quote
                0x2026 => Some(0x85), // horizontal ellipsis
                0x20AC => Some(0x80), // euro
                0x2122 => Some(0x99), // trademark
                _ => None,
            }
        }
        EncodingTarget::MacRoman => {
            // Mac Roman is a different 8-bit table than Latin-1; we cover
            // the common accented Latin chars and fall back otherwise.
            match ch as u32 {
                0x00E9 => Some(0x8E), // é
                0x00E8 => Some(0x8F), // è
                0x00E0 => Some(0x88), // à
                0x00F1 => Some(0x96), // ñ
                0x00FC => Some(0x9F), // ü
                0x00F6 => Some(0x9A), // ö
                0x00E7 => Some(0x8D), // ç
                0x00C9 => Some(0x83), // É
                0x00C8 => Some(0x82), // È
                _ => None,
            }
        }
    }
}

/// Attempt to match a text operation to a target text string.
/// Returns true if the operation's text matches (case-sensitive, trimmed).
pub fn text_matches(op: &TextOperation, target: &str) -> bool {
    let op_trimmed = op.text.trim();
    let target_trimmed = target.trim();
    op_trimmed == target_trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tj() {
        let stream = b"BT /F1 12 Tf 72 700 Td (Hello World) Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].text, "Hello World");
        assert_eq!(ops[0].font_name, Some("F1".to_string()));
        assert_eq!(ops[0].font_size, Some(12.0));
    }

    #[test]
    fn parse_escaped_string() {
        let stream = b"BT (Hello \\(World\\)) Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].text, "Hello (World)");
    }

    #[test]
    fn parse_tj_array() {
        let stream = b"BT [(He) -10 (llo)] TJ ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].text, "Hello");
    }

    #[test]
    fn replace_simple_tj() {
        let stream = b"BT /F1 12 Tf 72 700 Td (Hello World) Tj ET";
        let ops = find_text_operations(stream);
        let result = replace_text_in_stream(stream, &ops[0], "Goodbye World");
        let result_str = String::from_utf8_lossy(&result);
        assert!(result_str.contains("(Goodbye World)"));
        assert!(result_str.contains("Tj"));
        assert!(!result_str.contains("Hello World"));
    }

    #[test]
    fn replace_tj_array() {
        let stream = b"BT [(He) -10 (llo)] TJ ET";
        let ops = find_text_operations(stream);
        let result = replace_text_in_stream(stream, &ops[0], "Hi");
        let result_str = String::from_utf8_lossy(&result);
        assert!(result_str.contains("(Hi) Tj"));
    }

    #[test]
    fn multiple_text_ops() {
        let stream = b"BT /F1 12 Tf 72 700 Td (Line 1) Tj 72 680 Td (Line 2) Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].text, "Line 1");
        assert_eq!(ops[1].text, "Line 2");
    }

    #[test]
    fn text_matches_trimmed() {
        let op = TextOperation {
            text: "  Hello World  ".to_string(),
            operand_start: 0,
            operand_end: 0,
            font_name: None,
            font_size: None,
            is_hex_encoded: false,
            kind: TextOperatorKind::Tj,
            op_index: 0,
        };
        assert!(text_matches(&op, "Hello World"));
        assert!(!text_matches(&op, "hello world"));
    }

    // ─── Phase 28B: extended operator support ──────────────────────

    #[test]
    fn parse_single_quote_operator() {
        let stream = b"BT /F1 12 Tf 72 700 Td (Next line) ' ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1, "expected 1 op, got {:?}", ops);
        assert_eq!(ops[0].text, "Next line");
        assert_eq!(ops[0].kind, TextOperatorKind::SingleQuote);
    }

    #[test]
    fn parse_double_quote_operator() {
        let stream = b"BT /F1 12 Tf 72 700 Td 0 0 (Spaced line) \" ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].text, "Spaced line");
        assert_eq!(ops[0].kind, TextOperatorKind::DoubleQuote);
    }

    #[test]
    fn replace_single_quote_keeps_operator() {
        let stream = b"BT (Old) ' ET";
        let ops = find_text_operations(stream);
        let result = replace_text_in_stream(stream, &ops[0], "New");
        let s = String::from_utf8_lossy(&result);
        assert!(s.contains("(New) '"), "got: {s}");
        assert!(!s.contains("Old"));
    }

    #[test]
    fn replace_double_quote_keeps_operator() {
        let stream = b"BT 0 0 (Old) \" ET";
        let ops = find_text_operations(stream);
        let result = replace_text_in_stream(stream, &ops[0], "New");
        let s = String::from_utf8_lossy(&result);
        assert!(s.contains("(New) \""), "got: {s}");
    }

    #[test]
    fn op_index_increments_in_order() {
        let stream = b"BT /F1 12 Tf 72 700 Td (One) Tj 72 680 Td (Two) Tj 72 660 Td (Three) Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 3);
        assert_eq!(ops[0].op_index, 0);
        assert_eq!(ops[1].op_index, 1);
        assert_eq!(ops[2].op_index, 2);
    }

    #[test]
    fn repeated_identical_text_keeps_separate_op_indexes() {
        // Two identical "Total" strings — must keep different op_index
        // so the editor can disambiguate which one to replace.
        let stream = b"BT (Total) Tj 72 680 Td (Total) Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 2);
        assert_eq!(ops[0].text, "Total");
        assert_eq!(ops[1].text, "Total");
        assert_eq!(ops[0].op_index, 0);
        assert_eq!(ops[1].op_index, 1);
        assert_ne!(ops[0].operand_start, ops[1].operand_start);
    }

    #[test]
    fn hex_operator_is_classified_as_hex() {
        let stream = b"BT /F1 12 Tf <48656C6C6F> Tj ET";
        let ops = find_text_operations(stream);
        assert_eq!(ops.len(), 1);
        assert!(ops[0].is_hex_encoded);
        assert_eq!(ops[0].kind, TextOperatorKind::Hex);
    }

    #[test]
    fn replace_by_exact_operand_start() {
        // Verify that we can replace exactly one occurrence by passing
        // its op_index, not by string matching.
        let stream = b"BT (Total) Tj (Total) Tj ET";
        let ops = find_text_operations(stream);
        let result = replace_text_in_stream(stream, &ops[1], "Sum");
        let s = String::from_utf8_lossy(&result);
        // First occurrence unchanged, second swapped.
        assert!(s.starts_with("BT (Total) Tj (Sum)"), "got: {s}");
    }

    #[test]
    fn pdf_encode_escapes_special() {
        let out = pdf_encode_string_bytes("a(b)c\\d", EncodingTarget::WinAnsi);
        assert_eq!(out, b"a\\(b\\)c\\\\d");
    }

    // ─── Phase 29D: encoding-aware string emission ────────────────

    #[test]
    fn winansi_encodes_latin1_as_octal_escape() {
        // é (U+00E9) → byte 0xE9 → \351 octal
        let out = pdf_encode_string_bytes("café", EncodingTarget::WinAnsi);
        assert_eq!(out, b"caf\\351");
    }

    #[test]
    fn winansi_encodes_smart_punctuation() {
        // U+2014 em dash → 0x97 → \227
        let out = pdf_encode_string_bytes("a—b", EncodingTarget::WinAnsi);
        assert_eq!(out, b"a\\227b");
    }

    #[test]
    fn ascii_target_replaces_non_ascii_with_question_mark() {
        let out = pdf_encode_string_bytes("café", EncodingTarget::Ascii);
        assert_eq!(out, b"caf?");
    }

    #[test]
    fn macroman_encodes_e_acute_correctly() {
        // é (U+00E9) → 0x8E in MacRoman → \216
        let out = pdf_encode_string_bytes("é", EncodingTarget::MacRoman);
        assert_eq!(out, b"\\216");
    }

    #[test]
    fn winansi_unsupported_char_falls_back_to_question_mark() {
        // U+4E2D (CJK) is not in WinAnsi — should be '?'.
        let out = pdf_encode_string_bytes("中", EncodingTarget::WinAnsi);
        assert_eq!(out, b"?");
    }

    #[test]
    fn winansi_preserves_ascii_unchanged() {
        let out = pdf_encode_string_bytes("Hello World!", EncodingTarget::WinAnsi);
        assert_eq!(out, b"Hello World!");
    }

    #[test]
    fn control_chars_are_octal_escaped() {
        let out = pdf_encode_string_bytes("\x01", EncodingTarget::WinAnsi);
        assert_eq!(out, b"\\001");
    }

    #[test]
    fn replace_text_in_stream_encoded_uses_target_encoding() {
        // Build a stream with a Tj and replace with Latin-1 content.
        let stream = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
        let ops = find_text_operations(stream);
        let out = replace_text_in_stream_encoded(stream, &ops[0], "café", EncodingTarget::WinAnsi);
        let s = String::from_utf8_lossy(&out);
        // The new operand uses octal escape for é, not UTF-8 multibyte.
        assert!(s.contains("(caf\\351)"), "got: {s}");
        // Old content gone.
        assert!(!s.contains("(Hello)"));
    }
}
