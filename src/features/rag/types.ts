export interface Citation {
  citation_id: string;
  page_index: number;
  chunk_id: string;
  snippet: string;
  score: number;
  source: string;
}

export interface RagAnswerResult {
  answer: string;
  citations: Citation[];
  source_snippets: string[];
  retrieved_count: number;
  retrieval_mode_used: string;
  dense_used: boolean;
  bm25_used: boolean;
  used_reranker: boolean;
  elapsed_ms: number;
  warnings: string[];
}

export interface IndexStatus {
  session_id: string;
  indexed_pages: number;
  chunk_count: number;
  embedding_model_id: string;
  built_at: number | null;
  include_ocr: boolean;
  status: "empty" | "building" | "ready" | "failed";
  retrieval_mode: string;
  dense_embedding_available: boolean;
  fallback_reason: string | null;
}

export interface PageText {
  page_index: number;
  native_text: string | null;
  ocr_text: string | null;
}

export type RagStatus = "idle" | "building" | "asking" | "ready" | "error";
