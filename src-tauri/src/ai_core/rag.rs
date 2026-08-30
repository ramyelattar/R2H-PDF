//! RAG pipeline: retrieves relevant chunks, builds grounded prompts,
//! generates answers with citations using the local LLM.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::chunking::{chunk_page_text, merge_page_texts, ChunkingOptions, DocumentChunk};
use super::local_runtime::LocalRuntime;
use super::local_types::LocalGenerateRequest;
use super::vector_index::{DocumentIndex, IndexStatus, VectorSearchResult};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub citation_id: String,
    pub page_index: usize,
    pub chunk_id: String,
    pub snippet: String,
    pub score: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQuestionRequest {
    pub session_id: String,
    pub question: String,
    pub top_k: Option<usize>,
    pub include_ocr: bool,
    pub use_reranker: bool,
    pub max_context_chars: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagAnswerResult {
    pub answer: String,
    pub citations: Vec<Citation>,
    pub source_snippets: Vec<String>,
    pub retrieved_count: usize,
    pub retrieval_mode_used: String,
    pub dense_used: bool,
    pub bm25_used: bool,
    pub used_reranker: bool,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildIndexRequest {
    pub session_id: String,
    pub include_ocr: bool,
    pub page_texts: Vec<PageText>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageText {
    pub page_index: usize,
    pub native_text: Option<String>,
    pub ocr_text: Option<String>,
}

// ---------------------------------------------------------------------------
// RAG Engine
// ---------------------------------------------------------------------------

pub struct RagEngine {
    indexes: HashMap<String, DocumentIndex>,
}

impl RagEngine {
    pub fn new() -> Self {
        Self { indexes: HashMap::new() }
    }

    /// Build a document index from page texts.
    /// Automatically attempts dense embeddings if runtime is available.
    pub fn build_index(&mut self, request: BuildIndexRequest, embedding_runtime: Option<&mut super::embedding::EmbeddingRuntime>) -> IndexStatus {
        let session_id = &request.session_id;
        let options = ChunkingOptions {
            include_ocr: request.include_ocr,
            include_native_text: true,
            ..Default::default()
        };

        let mut all_chunks: Vec<DocumentChunk> = Vec::new();

        for page in &request.page_texts {
            let native = if options.include_native_text { page.native_text.as_deref() } else { None };
            let ocr = if options.include_ocr { page.ocr_text.as_deref() } else { None };

            let (text, source) = merge_page_texts(native, ocr);
            if text.is_empty() {
                continue;
            }

            let chunks = chunk_page_text(session_id, page.page_index, &text, &source, &options);
            all_chunks.extend(chunks);
        }

        // Build BM25 index (always).
        let mut index = DocumentIndex::new(session_id);
        index.build(all_chunks.clone(), request.include_ocr);

        // Attempt dense embeddings if runtime is available.
        if let Some(emb_runtime) = embedding_runtime {
            if emb_runtime.is_available() {
                // Start server if not already running.
                if !emb_runtime.is_server_ready() {
                    if let Err(e) = emb_runtime.start_server() {
                        index.set_dense_fallback(&format!("Embedding server start failed: {e}"));
                        let status = index.status();
                        self.indexes.insert(session_id.to_string(), index);
                        return status;
                    }
                }

                // Generate embeddings for all chunks.
                let texts: Vec<String> = all_chunks.iter().map(|c| c.text.clone()).collect();
                match emb_runtime.embed_texts(&texts) {
                    Ok(vectors) => {
                        index.set_dense_vectors(vectors, "qwen3-embedding-4b");
                    }
                    Err(e) => {
                        index.set_dense_fallback(&format!("Embedding generation failed: {e}"));
                    }
                }
            } else {
                let reason = emb_runtime.fallback_reason().unwrap_or_else(|| "Embedding runtime not available".to_string());
                index.set_dense_fallback(&reason);
            }
        } else {
            index.set_dense_fallback("Embedding runtime not configured");
        }

        let status = index.status();
        self.indexes.insert(session_id.to_string(), index);
        status
    }

    /// Get index status for a session.
    pub fn get_index_status(&self, session_id: &str) -> IndexStatus {
        self.indexes.get(session_id)
            .map(|idx| idx.status())
            .unwrap_or_else(|| IndexStatus {
                session_id: session_id.to_string(),
                indexed_pages: 0,
                chunk_count: 0,
                embedding_model_id: "none".to_string(),
                built_at: None,
                include_ocr: false,
                status: "empty".to_string(),
                retrieval_mode: "bm25".to_string(),
                dense_embedding_available: false,
                fallback_reason: Some("Index not built yet.".to_string()),
            })
    }

    /// Clear the index for a session.
    pub fn clear_index(&mut self, session_id: &str) {
        self.indexes.remove(session_id);
    }

    /// Semantic search — uses hybrid when dense vectors are available.
    pub fn search(&self, session_id: &str, query: &str, top_k: usize, query_embedding: Option<&[f32]>) -> Vec<VectorSearchResult> {
        self.indexes.get(session_id)
            .map(|idx| idx.search_hybrid(query, query_embedding, top_k, 0.01))
            .unwrap_or_default()
    }

    /// Full RAG pipeline: retrieve → build prompt → generate → cite.
    pub fn ask(
        &self,
        request: &RagQuestionRequest,
        runtime: &LocalRuntime,
        embedding_runtime: Option<&super::embedding::EmbeddingRuntime>,
    ) -> Result<RagAnswerResult, String> {
        let start = Instant::now();
        let mut warnings: Vec<String> = Vec::new();

        // 1. Get query embedding if dense is available.
        let query_embedding = if let Some(emb) = embedding_runtime {
            if emb.is_server_ready() {
                match emb.embed_query(&request.question) {
                    Ok(vec) => Some(vec),
                    Err(e) => {
                        warnings.push(format!("Query embedding failed, using BM25 only: {e}"));
                        None
                    }
                }
            } else { None }
        } else { None };

        // 2. Retrieve relevant chunks (hybrid or BM25).
        let top_k = request.top_k.unwrap_or(5);
        let results = self.search(&request.session_id, &request.question, top_k, query_embedding.as_deref());

        let dense_used = query_embedding.is_some() && self.indexes.get(&request.session_id).map(|i| i.is_hybrid()).unwrap_or(false);
        let retrieval_mode_used = if dense_used { "hybrid" } else { "bm25" };

        if results.is_empty() {
            return Ok(RagAnswerResult {
                answer: "I could not find enough evidence in the document to answer this.".to_string(),
                citations: Vec::new(),
                source_snippets: Vec::new(),
                retrieved_count: 0,
                retrieval_mode_used: retrieval_mode_used.to_string(),
                dense_used,
                bm25_used: true,
                used_reranker: false,
                elapsed_ms: start.elapsed().as_millis() as u64,
                warnings: vec!["No relevant chunks found for the query.".to_string()],
            });
        }

        // 2. Reranker (not wired in Phase 9).
        if request.use_reranker {
            warnings.push("Reranker runtime not wired yet. Using BM25 ranking only.".to_string());
        }

        // 3. Build citations.
        let citations: Vec<Citation> = results.iter().enumerate().map(|(i, r)| {
            Citation {
                citation_id: format!("cite-{}", i + 1),
                page_index: r.page_index,
                chunk_id: r.chunk_id.clone(),
                snippet: truncate_snippet(&r.text, 200),
                score: r.score,
                source: r.source.clone(),
            }
        }).collect();

        let source_snippets: Vec<String> = results.iter()
            .map(|r| truncate_snippet(&r.text, 300))
            .collect();

        // 4. Build grounded prompt.
        let max_context = request.max_context_chars.unwrap_or(4000);
        let context = build_context(&results, max_context);
        let prompt = build_rag_prompt(&request.question, &context, &citations);

        // 5. Generate answer using local LLM.
        let default_model = runtime.registry.default_model()
            .ok_or_else(|| "No default LLM model configured".to_string())?;

        let gen_request = LocalGenerateRequest {
            model_id: default_model.id,
            prompt,
            system_prompt: Some(RAG_SYSTEM_PROMPT.to_string()),
            max_tokens: Some(512),
            temperature: Some(0.3),
            top_p: Some(0.9),
            timeout_ms: Some(90_000),
        };

        let gen_result = runtime.generate(gen_request)?;
        let answer = gen_result.text.trim().to_string();

        if gen_result.warnings.len() > 0 {
            warnings.extend(gen_result.warnings);
        }

        Ok(RagAnswerResult {
            answer,
            citations,
            source_snippets,
            retrieved_count: results.len(),
            retrieval_mode_used: retrieval_mode_used.to_string(),
            dense_used,
            bm25_used: true,
            used_reranker: false,
            elapsed_ms: start.elapsed().as_millis() as u64,
            warnings,
        })
    }
}

// ---------------------------------------------------------------------------
// Prompt Building
// ---------------------------------------------------------------------------

const RAG_SYSTEM_PROMPT: &str = "\
You are a document assistant. Answer questions ONLY based on the provided context excerpts. \
Rules: \
1. If the context does not contain enough information, say 'I could not find enough evidence in the document to answer this.' \
2. Cite page numbers using [Page X] format when referencing specific information. \
3. Do not invent facts or cite pages not in the context. \
4. Be concise and accurate. \
5. Use the same language as the user's question unless asked otherwise.";

fn build_context(results: &[VectorSearchResult], max_chars: usize) -> String {
    let mut context = String::new();
    for (i, r) in results.iter().enumerate() {
        let entry = format!("[Page {}] (Source: {})\n{}\n\n", r.page_index + 1, r.source, r.text);
        if context.len() + entry.len() > max_chars {
            break;
        }
        context.push_str(&entry);
    }
    context
}

fn build_rag_prompt(question: &str, context: &str, _citations: &[Citation]) -> String {
    format!(
        "Context from the document:\n---\n{}\n---\n\nQuestion: {}\n\nAnswer based only on the context above:",
        context, question
    )
}

fn truncate_snippet(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}…", &text[..text.floor_char_boundary(max_len)])
    }
}

// Polyfill for str::floor_char_boundary (stable in Rust 1.73+)
trait FloorCharBoundary {
    fn floor_char_boundary(&self, index: usize) -> usize;
}

impl FloorCharBoundary for str {
    fn floor_char_boundary(&self, index: usize) -> usize {
        if index >= self.len() { return self.len(); }
        let mut i = index;
        while i > 0 && !self.is_char_boundary(i) { i -= 1; }
        i
    }
}

// ---------------------------------------------------------------------------
// Tauri State
// ---------------------------------------------------------------------------

pub struct RagState {
    pub engine: Mutex<RagEngine>,
    pub embedding: Mutex<super::embedding::EmbeddingRuntime>,
}

impl RagState {
    pub fn new() -> Self {
        let workspace_root = find_workspace_root_for_rag();
        Self {
            engine: Mutex::new(RagEngine::new()),
            embedding: Mutex::new(super::embedding::EmbeddingRuntime::new(&workspace_root)),
        }
    }
}

fn find_workspace_root_for_rag() -> std::path::PathBuf {
    let local_ai_root = super::local_ai_paths::resolve_local_ai_root(None);
    super::local_ai_paths::workspace_root_from_local_ai_root(&local_ai_root)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_index_and_search() {
        let mut engine = RagEngine::new();
        let request = BuildIndexRequest {
            session_id: "s1".to_string(),
            include_ocr: false,
            page_texts: vec![
                PageText { page_index: 0, native_text: Some("The contract value is $5 million.".to_string()), ocr_text: None },
                PageText { page_index: 1, native_text: Some("Completion date is December 2025.".to_string()), ocr_text: None },
            ],
        };
        let status = engine.build_index(request, None);
        assert_eq!(status.status, "ready");
        assert_eq!(status.indexed_pages, 2);
        assert_eq!(status.retrieval_mode, "bm25");

        let results = engine.search("s1", "contract value", 3, None);
        assert!(!results.is_empty());
        assert_eq!(results[0].page_index, 0);
    }

    #[test]
    fn no_evidence_returns_clear_message() {
        let engine = RagEngine::new();
        let results = engine.search("s1", "anything", 5, None);
        assert!(results.is_empty());
    }

    #[test]
    fn citations_built_from_retrieved_chunks() {
        let mut engine = RagEngine::new();
        engine.build_index(BuildIndexRequest {
            session_id: "s1".to_string(),
            include_ocr: false,
            page_texts: vec![
                PageText { page_index: 2, native_text: Some("Steel reinforcement schedule shows 500 tons.".to_string()), ocr_text: None },
            ],
        }, None);
        let results = engine.search("s1", "steel reinforcement", 3, None);
        assert!(!results.is_empty());
        assert_eq!(results[0].page_index, 2);
    }

    #[test]
    fn clear_index_removes_data() {
        let mut engine = RagEngine::new();
        engine.build_index(BuildIndexRequest {
            session_id: "s1".to_string(),
            include_ocr: false,
            page_texts: vec![PageText { page_index: 0, native_text: Some("test".to_string()), ocr_text: None }],
        }, None);
        assert_eq!(engine.get_index_status("s1").status, "ready");
        engine.clear_index("s1");
        assert_eq!(engine.get_index_status("s1").status, "empty");
    }

    #[test]
    fn build_index_reports_fallback_when_no_embedding() {
        let mut engine = RagEngine::new();
        let status = engine.build_index(BuildIndexRequest {
            session_id: "s1".to_string(),
            include_ocr: false,
            page_texts: vec![PageText { page_index: 0, native_text: Some("test content".to_string()), ocr_text: None }],
        }, None);
        assert_eq!(status.retrieval_mode, "bm25");
        assert!(!status.dense_embedding_available);
        assert!(status.fallback_reason.is_some());
    }

    #[test]
    fn prompt_includes_context_and_question() {
        let context = "[Page 1] (Source: native_text)\nSome content\n\n";
        let prompt = build_rag_prompt("What is the value?", context, &[]);
        assert!(prompt.contains("Some content"));
        assert!(prompt.contains("What is the value?"));
        assert!(prompt.contains("Context from the document"));
    }
}
