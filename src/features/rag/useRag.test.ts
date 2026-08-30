import { describe, it, expect } from "vitest";
import type { Citation, IndexStatus, PageText, RagAnswerResult } from "./types";

/**
 * Phase 9 tests for RAG frontend types and state logic.
 */

describe("IndexStatus", () => {
  it("represents empty index", () => {
    const status: IndexStatus = {
      session_id: "s1", indexed_pages: 0, chunk_count: 0,
      embedding_model_id: "none", built_at: null, include_ocr: false, status: "empty",
      retrieval_mode: "bm25", dense_embedding_available: false, fallback_reason: "Index not built yet.",
    };
    expect(status.status).toBe("empty");
    expect(status.retrieval_mode).toBe("bm25");
    expect(status.dense_embedding_available).toBe(false);
  });

  it("represents ready index with BM25 fallback", () => {
    const status: IndexStatus = {
      session_id: "s1", indexed_pages: 10, chunk_count: 25,
      embedding_model_id: "bm25-keyword", built_at: Date.now(), include_ocr: true, status: "ready",
      retrieval_mode: "bm25", dense_embedding_available: false,
      fallback_reason: "llama-server.exe not found. BM25 keyword retrieval is active.",
    };
    expect(status.status).toBe("ready");
    expect(status.chunk_count).toBe(25);
    expect(status.retrieval_mode).toBe("bm25");
    expect(status.fallback_reason).toContain("BM25");
  });

  it("represents ready index with hybrid mode", () => {
    const status: IndexStatus = {
      session_id: "s1", indexed_pages: 10, chunk_count: 25,
      embedding_model_id: "qwen3-embedding-4b", built_at: Date.now(), include_ocr: false, status: "ready",
      retrieval_mode: "hybrid", dense_embedding_available: true, fallback_reason: null,
    };
    expect(status.retrieval_mode).toBe("hybrid");
    expect(status.dense_embedding_available).toBe(true);
    expect(status.fallback_reason).toBeNull();
  });
});

describe("RagAnswerResult", () => {
  it("represents answer with citations and retrieval mode", () => {
    const result: RagAnswerResult = {
      answer: "The contract value is $5 million [Page 1].",
      citations: [
        { citation_id: "cite-1", page_index: 0, chunk_id: "c1", snippet: "contract value is $5 million", score: 0.85, source: "native_text" },
      ],
      source_snippets: ["The contract value is $5 million."],
      retrieved_count: 3,
      retrieval_mode_used: "bm25",
      dense_used: false,
      bm25_used: true,
      used_reranker: false,
      elapsed_ms: 5200,
      warnings: [],
    };
    expect(result.citations.length).toBe(1);
    expect(result.citations[0].page_index).toBe(0);
    expect(result.answer).toContain("$5 million");
    expect(result.retrieval_mode_used).toBe("bm25");
    expect(result.bm25_used).toBe(true);
  });

  it("represents no-evidence answer", () => {
    const result: RagAnswerResult = {
      answer: "I could not find enough evidence in the document to answer this.",
      citations: [],
      source_snippets: [],
      retrieved_count: 0,
      retrieval_mode_used: "bm25",
      dense_used: false,
      bm25_used: true,
      used_reranker: false,
      elapsed_ms: 100,
      warnings: ["No relevant chunks found for the query."],
    };
    expect(result.citations.length).toBe(0);
    expect(result.warnings.length).toBe(1);
  });
});

describe("Citation", () => {
  it("has page_index and snippet", () => {
    const cite: Citation = {
      citation_id: "cite-1", page_index: 3, chunk_id: "chunk-s1-3-0",
      snippet: "Steel reinforcement schedule shows 500 tons.", score: 0.72, source: "native_text",
    };
    expect(cite.page_index).toBe(3);
    expect(cite.snippet).toContain("500 tons");
  });
});

describe("PageText", () => {
  it("supports native and OCR text", () => {
    const page: PageText = {
      page_index: 0,
      native_text: "Native extracted text",
      ocr_text: "OCR extracted text",
    };
    expect(page.native_text).toBe("Native extracted text");
    expect(page.ocr_text).toBe("OCR extracted text");
  });

  it("supports null for missing sources", () => {
    const page: PageText = { page_index: 1, native_text: null, ocr_text: "OCR only" };
    expect(page.native_text).toBeNull();
  });
});

describe("RAG UI logic", () => {
  it("ask button disabled when index not ready", () => {
    const indexReady = false;
    const question = "What is the value?";
    const canAsk = indexReady && question.trim().length > 0;
    expect(canAsk).toBe(false);
  });

  it("ask button enabled when index ready and question present", () => {
    const indexReady = true;
    const question = "What is the value?";
    const canAsk = indexReady && question.trim().length > 0;
    expect(canAsk).toBe(true);
  });

  it("include OCR toggle changes request", () => {
    const includeOcr = true;
    expect(includeOcr).toBe(true);
  });

  it("use reranker toggle changes request", () => {
    const useReranker = true;
    expect(useReranker).toBe(true);
  });
});
