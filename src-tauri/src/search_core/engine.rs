use std::collections::HashMap;

use regex::{Regex, RegexBuilder};

use crate::search_core::{
    errors::SearchCoreError,
    types::{
        BBox, IndexStatus, PageTextWithSpans, SearchMatch, SearchQuery, SearchResponse,
        SearchScope, SpanRecord,
    },
};

/// Per-session index: maps page_index → raw text and spans
struct SessionIndex {
    pages: HashMap<usize, String>,
    spans: HashMap<usize, Vec<SpanRecord>>,
    total_pages: usize,
    last_updated: Option<String>,
}

pub struct SearchEngine {
    indexes: HashMap<String, SessionIndex>,
    active_page_by_session: HashMap<String, usize>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            indexes: HashMap::new(),
            active_page_by_session: HashMap::new(),
        }
    }

    /// Update the active page for a session (called from navigation handlers).
    pub fn set_active_page(&mut self, session_id: String, page_index: usize) {
        self.active_page_by_session.insert(session_id, page_index);
    }

    /// Index a document using plain text only (no span data — bboxes will be zero).
    pub fn index_document(
        &mut self,
        session_id: String,
        pages: Vec<String>,
    ) -> IndexStatus {
        let total = pages.len();
        let mut page_map = HashMap::new();
        let mut span_map: HashMap<usize, Vec<SpanRecord>> = HashMap::new();
        for (i, text) in pages.into_iter().enumerate() {
            span_map.insert(i, Vec::new());
            page_map.insert(i, text);
        }
        self.indexes.insert(
            session_id.clone(),
            SessionIndex {
                pages: page_map,
                spans: span_map,
                total_pages: total,
                last_updated: Some("indexed".to_string()),
            },
        );
        IndexStatus {
            session_id,
            indexed_pages: total,
            total_pages: total,
            is_ready: true,
            last_updated: Some("indexed".to_string()),
        }
    }

    /// Index a document with span data for accurate bbox population.
    pub fn index_document_with_spans(
        &mut self,
        session_id: String,
        pages: Vec<PageTextWithSpans>,
    ) -> IndexStatus {
        let total = pages.len();
        let mut page_map = HashMap::new();
        let mut span_map = HashMap::new();
        for (i, page) in pages.into_iter().enumerate() {
            span_map.insert(i, page.spans);
            page_map.insert(i, page.text);
        }
        self.indexes.insert(
            session_id.clone(),
            SessionIndex {
                pages: page_map,
                spans: span_map,
                total_pages: total,
                last_updated: Some("indexed".to_string()),
            },
        );
        IndexStatus {
            session_id,
            indexed_pages: total,
            total_pages: total,
            is_ready: true,
            last_updated: Some("indexed".to_string()),
        }
    }

    pub fn get_index_status(&self, session_id: &str) -> IndexStatus {
        match self.indexes.get(session_id) {
            Some(idx) => IndexStatus {
                session_id: session_id.to_string(),
                indexed_pages: idx.pages.len(),
                total_pages: idx.total_pages,
                is_ready: true,
                last_updated: idx.last_updated.clone(),
            },
            None => IndexStatus {
                session_id: session_id.to_string(),
                indexed_pages: 0,
                total_pages: 0,
                is_ready: false,
                last_updated: None,
            },
        }
    }

    pub fn query(&self, q: SearchQuery) -> Result<SearchResponse, SearchCoreError> {
        let max = q.max_results.unwrap_or(200);

        // Build the regex from the query parameters.
        let re = build_regex(&q)?;

        let idx = match self.indexes.get(&q.session_id) {
            Some(idx) => idx,
            None => {
                // No index yet — return empty results rather than an error so the
                // caller can distinguish "no results" from "session unknown".
                return Ok(SearchResponse {
                    session_id: q.session_id,
                    query: q.query,
                    matches: Vec::new(),
                    total: 0,
                    truncated: false,
                });
            }
        };

        // Determine which pages to search based on scope.
        let page_filter: Option<usize> = match &q.scope {
            SearchScope::CurrentPage => {
                let active = self
                    .active_page_by_session
                    .get(&q.session_id)
                    .copied()
                    .unwrap_or(0);
                Some(active)
            }
            SearchScope::AllPages | SearchScope::Selection => None,
        };

        // Collect and sort pages.
        let mut pages: Vec<(usize, &String)> =
            idx.pages.iter().map(|(k, v)| (*k, v)).collect();
        pages.sort_by_key(|(k, _)| *k);

        let mut matches: Vec<SearchMatch> = Vec::new();
        let mut truncated = false;

        'outer: for (page_idx, text) in &pages {
            // Apply scope filter.
            if let Some(only_page) = page_filter {
                if *page_idx != only_page {
                    continue;
                }
            }

            let spans = idx.spans.get(page_idx).map(|s| s.as_slice()).unwrap_or(&[]);

            for m in re.find_iter(text) {
                if matches.len() >= max {
                    truncated = true;
                    break 'outer;
                }

                let match_start = m.start();
                let match_end = m.end();

                let snippet_start = match_start.saturating_sub(30);
                let snippet_end = (match_end + 30).min(text.len());
                // Ensure snippet boundaries are on valid char boundaries.
                let snippet_start = floor_char_boundary(text, snippet_start);
                let snippet_end = ceil_char_boundary(text, snippet_end);
                let snippet = text[snippet_start..snippet_end].to_string();

                let bbox = spans_to_bbox(spans, match_start, match_end);
                let match_index = matches.len();

                matches.push(SearchMatch {
                    page_index: *page_idx,
                    match_index,
                    snippet,
                    bbox,
                });
            }
        }

        let total = matches.len();

        Ok(SearchResponse {
            session_id: q.session_id,
            query: q.query,
            matches,
            total,
            truncated,
        })
    }

    pub fn clear_index(&mut self, session_id: &str) {
        self.indexes.remove(session_id);
        self.active_page_by_session.remove(session_id);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `Regex` from a `SearchQuery`, applying escaping and word-boundary
/// wrapping as requested.
fn build_regex(q: &SearchQuery) -> Result<Regex, SearchCoreError> {
    // Step 1: escape literal queries; leave regex queries as-is.
    let raw = if q.use_regex {
        q.query.clone()
    } else {
        regex::escape(&q.query)
    };

    // Step 2: wrap with word boundaries when requested.
    let pattern = if q.whole_words {
        format!(r"\b{}\b", raw)
    } else {
        raw
    };

    RegexBuilder::new(&pattern)
        .case_insensitive(!q.case_sensitive)
        .size_limit(10_000)
        .build()
        .map_err(|e| match e {
            regex::Error::CompiledTooBig(_) => SearchCoreError::RegexTooComplex,
            other => SearchCoreError::InvalidRegex(other.to_string()),
        })
}

/// Compute the union bounding box of all spans that overlap the byte range
/// `[match_start, match_end)` in the page text.
///
/// If no spans overlap (e.g. the index was built without span data), returns a
/// zero bbox so callers can still display the match without crashing.
fn spans_to_bbox(spans: &[SpanRecord], match_start: usize, match_end: usize) -> BBox {
    let mut x0 = f32::MAX;
    let mut y0 = f32::MAX;
    let mut x1 = f32::MIN;
    let mut y1 = f32::MIN;
    let mut found = false;

    for span in spans {
        // Overlap condition: span starts before match ends AND span ends after match starts.
        if span.char_start < match_end && span.char_end > match_start {
            x0 = x0.min(span.bbox.x0);
            y0 = y0.min(span.bbox.y0);
            x1 = x1.max(span.bbox.x1);
            y1 = y1.max(span.bbox.y1);
            found = true;
        }
    }

    if found {
        BBox { x0, y0, x1, y1 }
    } else {
        BBox { x0: 0.0, y0: 0.0, x1: 0.0, y1: 0.0 }
    }
}

/// Round `pos` down to the nearest valid UTF-8 character boundary in `s`.
fn floor_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p > 0 && !s.is_char_boundary(p) {
        p -= 1;
    }
    p
}

/// Round `pos` up to the nearest valid UTF-8 character boundary in `s`.
fn ceil_char_boundary(s: &str, pos: usize) -> usize {
    if pos >= s.len() {
        return s.len();
    }
    let mut p = pos;
    while p < s.len() && !s.is_char_boundary(p) {
        p += 1;
    }
    p
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search_core::types::{BBox, SearchScope, SpanRecord};

    fn make_query(
        session_id: &str,
        query: &str,
        case_sensitive: bool,
        whole_words: bool,
        use_regex: bool,
        scope: SearchScope,
    ) -> SearchQuery {
        SearchQuery {
            session_id: session_id.to_string(),
            query: query.to_string(),
            scope,
            case_sensitive,
            whole_words,
            use_regex,
            max_results: Some(100),
        }
    }

    fn engine_with_pages(pages: Vec<&str>) -> SearchEngine {
        let mut engine = SearchEngine::new();
        engine.index_document(
            "s1".to_string(),
            pages.into_iter().map(|s| s.to_string()).collect(),
        );
        engine
    }

    // ------------------------------------------------------------------
    // Plain match
    // ------------------------------------------------------------------
    #[test]
    fn plain_match_finds_text() {
        let engine = engine_with_pages(vec!["Hello world"]);
        let q = make_query("s1", "world", false, false, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 1);
        assert!(resp.matches[0].snippet.contains("world"));
    }

    // ------------------------------------------------------------------
    // Case-insensitive match
    // ------------------------------------------------------------------
    #[test]
    fn case_insensitive_match() {
        let engine = engine_with_pages(vec!["Hello World"]);
        let q = make_query("s1", "hello", false, false, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 1);
    }

    #[test]
    fn case_sensitive_no_match() {
        let engine = engine_with_pages(vec!["Hello World"]);
        let q = make_query("s1", "hello", true, false, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 0);
    }

    // ------------------------------------------------------------------
    // Regex match
    // ------------------------------------------------------------------
    #[test]
    fn regex_match() {
        let engine = engine_with_pages(vec!["foo123bar"]);
        let q = make_query("s1", r"\d+", false, false, true, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 1);
        assert!(resp.matches[0].snippet.contains("123"));
    }

    #[test]
    fn invalid_regex_returns_error() {
        let engine = engine_with_pages(vec!["text"]);
        let q = make_query("s1", r"[invalid", false, false, true, SearchScope::AllPages);
        let result = engine.query(q);
        assert!(matches!(result, Err(SearchCoreError::InvalidRegex(_))));
    }

    // ------------------------------------------------------------------
    // Whole-word match
    // ------------------------------------------------------------------
    #[test]
    fn whole_word_matches_standalone() {
        let engine = engine_with_pages(vec!["cat concatenate cat"]);
        let q = make_query("s1", "cat", false, true, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        // Should match "cat" twice but NOT "cat" inside "concatenate"
        assert_eq!(resp.total, 2);
    }

    #[test]
    fn whole_word_does_not_match_substring() {
        let engine = engine_with_pages(vec!["concatenate"]);
        let q = make_query("s1", "cat", false, true, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 0);
    }

    // ------------------------------------------------------------------
    // Scope: CurrentPage
    // ------------------------------------------------------------------
    #[test]
    fn scope_current_page_limits_results() {
        let mut engine = SearchEngine::new();
        engine.index_document(
            "s1".to_string(),
            vec!["needle on page 0".to_string(), "needle on page 1".to_string()],
        );
        engine.set_active_page("s1".to_string(), 0);

        let q = make_query("s1", "needle", false, false, false, SearchScope::CurrentPage);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 1);
        assert_eq!(resp.matches[0].page_index, 0);
    }

    // ------------------------------------------------------------------
    // BBox population via spans
    // ------------------------------------------------------------------
    #[test]
    fn bbox_populated_from_spans() {
        let mut engine = SearchEngine::new();
        let spans = vec![SpanRecord {
            char_start: 0,
            char_end: 5,
            bbox: BBox { x0: 10.0, y0: 20.0, x1: 50.0, y1: 30.0 },
        }];
        engine.index_document_with_spans(
            "s1".to_string(),
            vec![PageTextWithSpans {
                text: "hello world".to_string(),
                spans,
            }],
        );

        let q = make_query("s1", "hello", false, false, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        assert_eq!(resp.total, 1);
        let bbox = &resp.matches[0].bbox;
        assert!((bbox.x0 - 10.0).abs() < f32::EPSILON);
        assert!((bbox.y0 - 20.0).abs() < f32::EPSILON);
        assert!((bbox.x1 - 50.0).abs() < f32::EPSILON);
        assert!((bbox.y1 - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn bbox_zero_when_no_spans() {
        let engine = engine_with_pages(vec!["hello world"]);
        let q = make_query("s1", "hello", false, false, false, SearchScope::AllPages);
        let resp = engine.query(q).unwrap();
        let bbox = &resp.matches[0].bbox;
        assert_eq!(bbox.x0, 0.0);
        assert_eq!(bbox.y0, 0.0);
        assert_eq!(bbox.x1, 0.0);
        assert_eq!(bbox.y1, 0.0);
    }

    // ------------------------------------------------------------------
    // Property-based tests
    // ------------------------------------------------------------------

    /// **Validates: Requirements 1.1**
    ///
    /// Property 1: for any valid ASCII text and a simple literal pattern,
    /// every returned `SearchMatch.snippet` must be matched by the compiled regex.
    #[cfg(test)]
    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(32))]
            #[test]
            fn prop_regex_matches_satisfy_pattern(
                text in "[a-zA-Z0-9 ]{0,100}",
                pattern in "[a-z]{1,5}",
            ) {
                let mut engine = SearchEngine::new();
                engine.index_document("s1".to_string(), vec![text.clone()]);
                let q = SearchQuery {
                    session_id: "s1".to_string(),
                    query: pattern.clone(),
                    scope: SearchScope::AllPages,
                    case_sensitive: false,
                    whole_words: false,
                    use_regex: true,
                    max_results: Some(100),
                };
                if let Ok(resp) = engine.query(q) {
                    // Build the verification regex with the same flags used by the engine
                    // (case_insensitive = true because case_sensitive = false above).
                    let re = regex::RegexBuilder::new(&pattern)
                        .case_insensitive(true)
                        .build()
                        .unwrap();
                    for m in &resp.matches {
                        prop_assert!(
                            re.is_match(&m.snippet),
                            "snippet {:?} did not match pattern {:?}",
                            m.snippet,
                            pattern
                        );
                    }
                }
            }
        }

        // **Validates: Requirements 1.3**
        //
        // Property 3: when `whole_words: true`, every returned match's snippet
        // must contain the query term bounded by non-word characters or string
        // edges.  We generate arbitrary text and word queries, run a whole-word
        // search, and verify the property holds for every returned match by
        // checking that the matched occurrence in the original page text is
        // bounded by `\b` (word boundary).
        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]
            #[test]
            fn prop_whole_word_matches_respect_boundaries(
                word in "[a-z]{2,6}",
                text in "[a-zA-Z0-9 .,;:!?()-]{0,200}",
            ) {
                let mut engine = SearchEngine::new();
                engine.index_document("s1".to_string(), vec![text.clone()]);
                let q = SearchQuery {
                    session_id: "s1".to_string(),
                    query: word.clone(),
                    scope: SearchScope::AllPages,
                    case_sensitive: false,
                    whole_words: true,
                    use_regex: false,
                    max_results: Some(200),
                };
                if let Ok(resp) = engine.query(q) {
                    // Build a verification regex that matches the word at word boundaries.
                    let verify_re = regex::RegexBuilder::new(
                        &format!(r"\b{}\b", regex::escape(&word))
                    )
                        .case_insensitive(true)
                        .build()
                        .unwrap();

                    for m in &resp.matches {
                        // The snippet must contain the query term at a word boundary.
                        prop_assert!(
                            verify_re.is_match(&m.snippet),
                            "word {:?} not found at word boundary in snippet {:?} (page text: {:?})",
                            word,
                            m.snippet,
                            text
                        );
                    }

                    // Additionally verify against the original page text: every match
                    // position in the full text must be at a word boundary.
                    let boundary_re = regex::RegexBuilder::new(
                        &format!(r"\b{}\b", regex::escape(&word))
                    )
                        .case_insensitive(true)
                        .build()
                        .unwrap();
                    let expected_count = boundary_re.find_iter(&text).count();
                    prop_assert_eq!(
                        resp.matches.len(),
                        expected_count,
                        "match count mismatch: engine returned {} but boundary regex found {} in {:?}",
                        resp.matches.len(),
                        expected_count,
                        text
                    );
                }
            }
        }
    }
}
