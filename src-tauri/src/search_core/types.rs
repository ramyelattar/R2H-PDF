use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SearchScope {
    CurrentPage,
    AllPages,
    Selection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub session_id: String,
    pub query: String,
    pub scope: SearchScope,
    pub case_sensitive: bool,
    pub whole_words: bool,
    pub use_regex: bool,
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub char_start: usize,
    pub char_end: usize,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTextWithSpans {
    pub text: String,
    pub spans: Vec<SpanRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub page_index: usize,
    pub match_index: usize,
    pub snippet: String,
    pub bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub session_id: String,
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub session_id: String,
    pub indexed_pages: usize,
    pub total_pages: usize,
    pub is_ready: bool,
    pub last_updated: Option<String>,
}
