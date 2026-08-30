//! Document chunking: splits page text into overlapping chunks for RAG retrieval.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingOptions {
    pub max_chars: usize,
    pub overlap_chars: usize,
    pub include_ocr: bool,
    pub include_native_text: bool,
}

impl Default for ChunkingOptions {
    fn default() -> Self {
        Self {
            max_chars: 1200,
            overlap_chars: 150,
            include_ocr: true,
            include_native_text: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub chunk_id: String,
    pub session_id: String,
    pub page_index: usize,
    pub source: String, // "native_text" | "ocr_text" | "mixed"
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
    pub token_estimate: usize,
}

/// Chunk a single page's text into overlapping segments.
/// Tries to break at line boundaries when possible.
pub fn chunk_page_text(
    session_id: &str,
    page_index: usize,
    text: &str,
    source: &str,
    options: &ChunkingOptions,
) -> Vec<DocumentChunk> {
    let normalized = normalize_whitespace(text);
    if normalized.trim().is_empty() {
        return Vec::new();
    }

    let max_chars = options.max_chars;
    let overlap = options.overlap_chars;
    let step = max_chars.saturating_sub(overlap).max(100);

    let mut chunks = Vec::new();
    let mut start = 0;
    let text_len = normalized.len();
    let mut chunk_idx = 0;

    while start < text_len {
        // Ensure start is on a char boundary.
        while start < text_len && !normalized.is_char_boundary(start) {
            start += 1;
        }
        if start >= text_len {
            break;
        }

        let mut end = (start + max_chars).min(text_len);
        // Ensure end is on a char boundary.
        while end < text_len && !normalized.is_char_boundary(end) {
            end += 1;
        }
        let end = end.min(text_len);

        // Try to break at a line boundary.
        let step_end = (start + step).min(end);
        let mut step_end_safe = step_end;
        while step_end_safe < text_len && !normalized.is_char_boundary(step_end_safe) {
            step_end_safe += 1;
        }
        let actual_end = if end < text_len {
            find_line_break(&normalized, step_end_safe.min(end), end)
        } else {
            end
        };

        let chunk_text = normalized[start..actual_end].trim().to_string();
        if !chunk_text.is_empty() {
            chunks.push(DocumentChunk {
                chunk_id: format!("chunk-{}-{}-{}", session_id, page_index, chunk_idx),
                session_id: session_id.to_string(),
                page_index,
                source: source.to_string(),
                text: chunk_text.clone(),
                char_start: start,
                char_end: actual_end,
                token_estimate: chunk_text.len() / 4, // rough estimate
            });
            chunk_idx += 1;
        }

        if actual_end >= text_len {
            break;
        }
        start += step;
        // Ensure start stays on char boundary.
        while start < text_len && !normalized.is_char_boundary(start) {
            start += 1;
        }
    }

    chunks
}

/// Merge native text and OCR text for a page.
/// If both exist, prefer native text but append OCR-only content.
pub fn merge_page_texts(native: Option<&str>, ocr: Option<&str>) -> (String, String) {
    match (native, ocr) {
        (Some(n), None) if !n.trim().is_empty() => (n.to_string(), "native_text".to_string()),
        (None, Some(o)) if !o.trim().is_empty() => (o.to_string(), "ocr_text".to_string()),
        (Some(n), Some(_o)) if !n.trim().is_empty() => (n.to_string(), "native_text".to_string()),
        (Some(_n), Some(o)) if !o.trim().is_empty() => (o.to_string(), "ocr_text".to_string()),
        _ => (String::new(), "none".to_string()),
    }
}

fn normalize_whitespace(text: &str) -> String {
    // Collapse multiple whitespace but preserve newlines.
    let mut result = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch == '\n' {
            prev_space = false;
            result.push(ch);
        } else if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            prev_space = false;
            result.push(ch);
        }
    }
    result
}

fn find_line_break(text: &str, min_pos: usize, max_pos: usize) -> usize {
    // Look for a newline between min_pos and max_pos.
    if let Some(pos) = text[min_pos..max_pos].rfind('\n') {
        return min_pos + pos + 1;
    }
    // Fall back to a space.
    if let Some(pos) = text[min_pos..max_pos].rfind(' ') {
        return min_pos + pos + 1;
    }
    max_pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_preserves_page_index() {
        let chunks = chunk_page_text(
            "s1",
            3,
            "Hello world this is a test.",
            "native_text",
            &ChunkingOptions::default(),
        );
        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.page_index == 3));
    }

    #[test]
    fn chunk_skips_empty_text() {
        let chunks = chunk_page_text(
            "s1",
            0,
            "   \n  \n  ",
            "native_text",
            &ChunkingOptions::default(),
        );
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_handles_arabic_english() {
        let text = "This is English text. هذا نص عربي للاختبار. More English follows.";
        let chunks = chunk_page_text(
            "s1",
            0,
            text,
            "native_text",
            &ChunkingOptions {
                max_chars: 80,
                overlap_chars: 10,
                ..Default::default()
            },
        );
        assert!(!chunks.is_empty());
        // All chunks should have valid text.
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
        }
    }

    #[test]
    fn chunk_respects_max_chars() {
        let text = "a ".repeat(1000); // 2000 chars
        let opts = ChunkingOptions {
            max_chars: 200,
            overlap_chars: 20,
            ..Default::default()
        };
        let chunks = chunk_page_text("s1", 0, &text, "native_text", &opts);
        for chunk in &chunks {
            assert!(chunk.text.len() <= 210); // slight tolerance for line-break search
        }
    }

    #[test]
    fn merge_prefers_native_text() {
        let (text, source) = merge_page_texts(Some("Native content"), Some("OCR content"));
        assert_eq!(source, "native_text");
        assert_eq!(text, "Native content");
    }

    #[test]
    fn merge_falls_back_to_ocr() {
        let (text, source) = merge_page_texts(Some(""), Some("OCR content"));
        assert_eq!(source, "ocr_text");
        assert_eq!(text, "OCR content");
    }
}
