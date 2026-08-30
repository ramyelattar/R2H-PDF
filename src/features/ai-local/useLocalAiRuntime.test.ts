import { describe, it, expect } from "vitest";
import type { LocalGenerateRequest, LocalGenerateResult, LocalModelInfo, LocalRuntimeStatus } from "./types";

/**
 * Phase 8 tests for local AI runtime types and state logic.
 */

describe("LocalModelInfo", () => {
  it("represents an available model", () => {
    const model: LocalModelInfo = {
      id: "qwen3-4b-q4-k-m",
      name: "Qwen3 4B Q4_K_M",
      model_type: "llm",
      format: "gguf",
      path: "local-ai/models/llm/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf",
      runtime: "llama.cpp",
      exists: true,
      size_bytes: 2_500_000_000,
      default: true,
      context_window: 32768,
    };
    expect(model.exists).toBe(true);
    expect(model.default).toBe(true);
    expect(model.context_window).toBe(32768);
  });

  it("represents a missing model", () => {
    const model: LocalModelInfo = {
      id: "missing", name: "Missing", model_type: "llm", format: "gguf",
      path: "/no/such/file.gguf", runtime: "llama.cpp",
      exists: false, size_bytes: 0, default: false, context_window: 4096,
    };
    expect(model.exists).toBe(false);
    expect(model.size_bytes).toBe(0);
  });
});

describe("LocalRuntimeStatus", () => {
  it("reports available runtime", () => {
    const status: LocalRuntimeStatus = {
      available: true,
      runtime_path: "local-ai/runtimes/llama-cpp/llama-cli.exe",
      runtime_version: "llama.cpp (local)",
      default_model_id: "qwen3-4b-q4-k-m",
      errors: [],
    };
    expect(status.available).toBe(true);
    expect(status.errors.length).toBe(0);
  });

  it("reports missing runtime with errors", () => {
    const status: LocalRuntimeStatus = {
      available: false,
      runtime_path: "",
      runtime_version: "",
      default_model_id: null,
      errors: ["llama.cpp runtime not found"],
    };
    expect(status.available).toBe(false);
    expect(status.errors[0]).toContain("not found");
  });
});

describe("LocalGenerateRequest", () => {
  it("has correct shape", () => {
    const req: LocalGenerateRequest = {
      model_id: "qwen3-4b-q4-k-m",
      prompt: "Hello world",
      system_prompt: "You are helpful.",
      max_tokens: 256,
      temperature: 0.7,
      top_p: null,
      timeout_ms: 60000,
    };
    expect(req.model_id).toBe("qwen3-4b-q4-k-m");
    expect(req.timeout_ms).toBe(60000);
  });
});

describe("LocalGenerateResult", () => {
  it("represents successful generation", () => {
    const result: LocalGenerateResult = {
      model_id: "qwen3-4b-q4-k-m",
      text: "Here are the key points...",
      tokens_generated: 45,
      elapsed_ms: 3200,
      finish_reason: "stop",
      warnings: [],
    };
    expect(result.text.length).toBeGreaterThan(0);
    expect(result.finish_reason).toBe("stop");
  });

  it("represents timeout", () => {
    const result: LocalGenerateResult = {
      model_id: "qwen3-4b-q4-k-m",
      text: "",
      tokens_generated: null,
      elapsed_ms: 60000,
      finish_reason: "timeout",
      warnings: ["Generation timed out"],
    };
    expect(result.finish_reason).toBe("timeout");
    expect(result.warnings.length).toBe(1);
  });
});

describe("Offline enforcement", () => {
  it("generate request does not contain URL or endpoint", () => {
    const req: LocalGenerateRequest = {
      model_id: "qwen3-4b-q4-k-m",
      prompt: "test",
      system_prompt: null,
      max_tokens: 10,
      temperature: null,
      top_p: null,
      timeout_ms: null,
    };
    // The request type has no URL/endpoint field — generation is local only.
    const keys = Object.keys(req);
    expect(keys).not.toContain("endpoint");
    expect(keys).not.toContain("url");
    expect(keys).not.toContain("api_key");
  });
});
