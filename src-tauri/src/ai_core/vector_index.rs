//! In-memory vector index using BM25-style keyword scoring.
//! Phase 9 uses term-frequency retrieval since llama-cli does not support
//! embedding mode. Dense vector embeddings can be added when llama-embedding
//! binary becomes available.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::chunking::DocumentChunk;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub chunk_id: String,
    pub page_index: usize,
    pub text: String,
    pub score: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub session_id: String,
    pub indexed_pages: usize,
    pub chunk_count: usize,
    pub embedding_model_id: String,
    pub built_at: Option<u128>,
    pub include_ocr: bool,
    pub status: String,         // "empty" | "building" | "ready" | "failed"
    pub retrieval_mode: String, // "bm25" | "dense" | "hybrid"
    pub dense_embedding_available: bool,
    pub fallback_reason: Option<String>,
}

/// In-memory document index using BM25-style term frequency scoring
/// with optional dense vector embeddings for hybrid retrieval.
pub struct DocumentIndex {
    pub session_id: String,
    chunks: Vec<DocumentChunk>,
    term_freqs: Vec<HashMap<String, f32>>,
    doc_freq: HashMap<String, usize>,
    dense_vectors: Vec<Vec<f32>>,
    retrieval_mode: String,
    dense_available: bool,
    embedding_model_id: String,
    fallback_reason: Option<String>,
    status: String,
    built_at: Option<u128>,
    include_ocr: bool,
    indexed_pages: usize,
}

impl DocumentIndex {
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            chunks: Vec::new(),
            term_freqs: Vec::new(),
            doc_freq: HashMap::new(),
            dense_vectors: Vec::new(),
            retrieval_mode: "bm25".to_string(),
            dense_available: false,
            embedding_model_id: "bm25-keyword".to_string(),
            fallback_reason: None,
            status: "empty".to_string(),
            built_at: None,
            include_ocr: false,
            indexed_pages: 0,
        }
    }

    /// Build the BM25 index from chunks.
    pub fn build(&mut self, chunks: Vec<DocumentChunk>, include_ocr: bool) {
        self.status = "building".to_string();
        self.include_ocr = include_ocr;

        self.term_freqs.clear();
        self.doc_freq.clear();
        self.dense_vectors.clear();
        self.chunks = chunks;

        for chunk in &self.chunks {
            let tf = compute_term_freq(&chunk.text);
            for term in tf.keys() {
                *self.doc_freq.entry(term.clone()).or_insert(0) += 1;
            }
            self.term_freqs.push(tf);
        }

        let mut pages = std::collections::HashSet::new();
        for chunk in &self.chunks {
            pages.insert(chunk.page_index);
        }
        self.indexed_pages = pages.len();

        self.built_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or_default(),
        );
        self.status = "ready".to_string();
    }

    /// Add dense vectors after BM25 build. Must have same length as chunks.
    pub fn set_dense_vectors(&mut self, vectors: Vec<Vec<f32>>, model_id: &str) {
        if vectors.len() == self.chunks.len() {
            self.dense_vectors = vectors;
            self.dense_available = true;
            self.retrieval_mode = "hybrid".to_string();
            self.embedding_model_id = model_id.to_string();
            self.fallback_reason = None;
        }
    }

    /// Mark dense as unavailable with a reason.
    pub fn set_dense_fallback(&mut self, reason: &str) {
        self.dense_available = false;
        self.retrieval_mode = "bm25".to_string();
        self.fallback_reason = Some(reason.to_string());
    }

    /// Search using the best available method (hybrid or BM25-only).
    pub fn search(&self, query: &str, top_k: usize, min_score: f32) -> Vec<VectorSearchResult> {
        self.search_bm25(query, top_k, min_score)
    }

    /// Hybrid search: combine dense cosine + BM25 results.
    pub fn search_hybrid(
        &self,
        query: &str,
        query_embedding: Option<&[f32]>,
        top_k: usize,
        min_score: f32,
    ) -> Vec<VectorSearchResult> {
        let bm25_results = self.search_bm25(query, top_k * 2, min_score);

        let dense_results = if self.dense_available {
            if let Some(qe) = query_embedding {
                self.search_dense(qe, top_k * 2)
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if dense_results.is_empty() {
            // Pure BM25.
            let mut results = bm25_results;
            results.truncate(top_k);
            return results;
        }

        // Merge: normalize scores and combine.
        let bm25_max = bm25_results
            .first()
            .map(|r| r.score)
            .unwrap_or(1.0)
            .max(0.001);
        let dense_max = dense_results
            .first()
            .map(|r| r.score)
            .unwrap_or(1.0)
            .max(0.001);

        let mut combined: HashMap<String, VectorSearchResult> = HashMap::new();

        for r in &bm25_results {
            let norm_score = r.score / bm25_max * 0.4; // BM25 weight: 40%
            combined
                .entry(r.chunk_id.clone())
                .or_insert_with(|| VectorSearchResult {
                    chunk_id: r.chunk_id.clone(),
                    page_index: r.page_index,
                    text: r.text.clone(),
                    score: 0.0,
                    source: r.source.clone(),
                })
                .score += norm_score;
        }

        for r in &dense_results {
            let norm_score = r.score / dense_max * 0.6; // Dense weight: 60%
            combined
                .entry(r.chunk_id.clone())
                .or_insert_with(|| VectorSearchResult {
                    chunk_id: r.chunk_id.clone(),
                    page_index: r.page_index,
                    text: r.text.clone(),
                    score: 0.0,
                    source: r.source.clone(),
                })
                .score += norm_score;
        }

        let mut results: Vec<VectorSearchResult> = combined.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        results
    }

    /// BM25-only search.
    fn search_bm25(&self, query: &str, top_k: usize, min_score: f32) -> Vec<VectorSearchResult> {
        if self.chunks.is_empty() || self.status != "ready" {
            return Vec::new();
        }

        let query_terms = tokenize(query);
        let n = self.chunks.len() as f32;

        let mut scores: Vec<(usize, f32)> = Vec::new();

        for (idx, tf) in self.term_freqs.iter().enumerate() {
            let mut score = 0.0f32;
            for term in &query_terms {
                let term_tf = tf.get(term).copied().unwrap_or(0.0);
                let df = self.doc_freq.get(term).copied().unwrap_or(0) as f32;
                if term_tf > 0.0 && df > 0.0 {
                    let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
                    score += idf * term_tf;
                }
            }
            if score >= min_score {
                scores.push((idx, score));
            }
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);

        scores
            .into_iter()
            .map(|(idx, score)| {
                let chunk = &self.chunks[idx];
                VectorSearchResult {
                    chunk_id: chunk.chunk_id.clone(),
                    page_index: chunk.page_index,
                    text: chunk.text.clone(),
                    score,
                    source: chunk.source.clone(),
                }
            })
            .collect()
    }

    /// Dense cosine similarity search.
    fn search_dense(&self, query_embedding: &[f32], top_k: usize) -> Vec<VectorSearchResult> {
        if self.dense_vectors.is_empty() {
            return Vec::new();
        }

        let mut scores: Vec<(usize, f32)> = self
            .dense_vectors
            .iter()
            .enumerate()
            .map(|(idx, vec)| {
                (
                    idx,
                    crate::ai_core::embedding::cosine_similarity(query_embedding, vec),
                )
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);

        scores
            .into_iter()
            .map(|(idx, score)| {
                let chunk = &self.chunks[idx];
                VectorSearchResult {
                    chunk_id: chunk.chunk_id.clone(),
                    page_index: chunk.page_index,
                    text: chunk.text.clone(),
                    score,
                    source: chunk.source.clone(),
                }
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.term_freqs.clear();
        self.doc_freq.clear();
        self.dense_vectors.clear();
        self.retrieval_mode = "bm25".to_string();
        self.dense_available = false;
        self.fallback_reason = None;
        self.status = "empty".to_string();
        self.built_at = None;
        self.indexed_pages = 0;
    }

    pub fn status(&self) -> IndexStatus {
        IndexStatus {
            session_id: self.session_id.clone(),
            indexed_pages: self.indexed_pages,
            chunk_count: self.chunks.len(),
            embedding_model_id: self.embedding_model_id.clone(),
            built_at: self.built_at,
            include_ocr: self.include_ocr,
            status: self.status.clone(),
            retrieval_mode: self.retrieval_mode.clone(),
            dense_embedding_available: self.dense_available,
            fallback_reason: self.fallback_reason.clone(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status == "ready"
    }

    pub fn is_hybrid(&self) -> bool {
        self.retrieval_mode == "hybrid" && self.dense_available
    }

    pub fn dense_vector_count(&self) -> usize {
        self.dense_vectors.len()
    }
}

/// Tokenize text into lowercase terms.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

/// Compute normalized term frequency for a chunk.
fn compute_term_freq(text: &str) -> HashMap<String, f32> {
    let tokens = tokenize(text);
    let total = tokens.len() as f32;
    if total == 0.0 {
        return HashMap::new();
    }
    let mut freq: HashMap<String, f32> = HashMap::new();
    for token in tokens {
        *freq.entry(token).or_insert(0.0) += 1.0;
    }
    // Normalize by document length.
    for val in freq.values_mut() {
        *val /= total;
    }
    freq
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(id: &str, page: usize, text: &str) -> DocumentChunk {
        DocumentChunk {
            chunk_id: id.to_string(),
            session_id: "s1".to_string(),
            page_index: page,
            source: "native_text".to_string(),
            text: text.to_string(),
            char_start: 0,
            char_end: text.len(),
            token_estimate: text.len() / 4,
        }
    }

    #[test]
    fn index_search_ranks_relevant_higher() {
        let mut index = DocumentIndex::new("s1");
        let chunks = vec![
            make_chunk(
                "c1",
                0,
                "The electrical load schedule shows 500kW total connected load.",
            ),
            make_chunk(
                "c2",
                1,
                "The architectural drawings show floor plans and elevations.",
            ),
            make_chunk(
                "c3",
                2,
                "Cable sizing for the main distribution board requires 500A breaker.",
            ),
        ];
        index.build(chunks, false);

        let results = index.search("electrical load", 3, 0.0);
        assert!(!results.is_empty());
        assert_eq!(results[0].chunk_id, "c1"); // Most relevant
    }

    #[test]
    fn index_add_search_clear() {
        let mut index = DocumentIndex::new("s1");
        assert_eq!(index.status().status, "empty");

        index.build(
            vec![make_chunk("c1", 0, "Hello world test document")],
            false,
        );
        assert_eq!(index.status().status, "ready");
        assert_eq!(index.status().chunk_count, 1);

        let results = index.search("hello", 5, 0.0);
        assert_eq!(results.len(), 1);

        index.clear();
        assert_eq!(index.status().status, "empty");
        assert_eq!(index.status().chunk_count, 0);
    }

    #[test]
    fn index_status_transitions() {
        let mut index = DocumentIndex::new("s1");
        assert_eq!(index.status().status, "empty");
        index.build(vec![make_chunk("c1", 0, "test")], true);
        assert_eq!(index.status().status, "ready");
        assert!(index.status().include_ocr);
        assert!(index.status().built_at.is_some());
    }
}
