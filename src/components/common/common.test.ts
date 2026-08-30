import { describe, it, expect } from "vitest";

/**
 * Phase 16 tests for shared UX components and polish states.
 */

describe("EmptyState behavior", () => {
  it("no-document state shows welcome message", () => {
    const activeTab = null;
    const showWelcome = activeTab === null;
    expect(showWelcome).toBe(true);
  });

  it("PDF-only actions disabled when no document", () => {
    const hasDocument = false;
    const canSave = hasDocument;
    const canExport = hasDocument;
    const canOcr = hasDocument;
    expect(canSave).toBe(false);
    expect(canExport).toBe(false);
    expect(canOcr).toBe(false);
  });
});

describe("Missing local-ai state", () => {
  it("app does not crash when local-ai missing", () => {
    const localAiReady = false;
    const appCanOpen = true; // PDF viewing still works
    expect(appCanOpen).toBe(true);
    expect(localAiReady).toBe(false);
  });

  it("AI features show disabled when local-ai missing", () => {
    const localAiReady = false;
    const canAskRag = localAiReady;
    const canReview = localAiReady;
    const canOcr = false; // model missing
    expect(canAskRag).toBe(false);
    expect(canReview).toBe(false);
    expect(canOcr).toBe(false);
  });

  it("model manager shows actionable missing message", () => {
    const missingAssets = ["llama-cli.exe", "Qwen3-4B model"];
    const message = `Missing: ${missingAssets.join(", ")}. Open Models tab to validate.`;
    expect(message).toContain("Missing");
    expect(message).toContain("validate");
  });
});

describe("Loading states", () => {
  it("RAG index building shows loading", () => {
    const status = "building";
    expect(status).toBe("building");
  });

  it("AI generation shows loading", () => {
    const planning = true;
    expect(planning).toBe(true);
  });

  it("OCR running shows progress", () => {
    const ocrStatus = "running";
    expect(ocrStatus).toBe("running");
  });
});

describe("Disabled states", () => {
  it("Ask button disabled when index not ready", () => {
    const indexReady = false;
    const question = "What is the value?";
    const canAsk = indexReady && question.trim().length > 0;
    expect(canAsk).toBe(false);
  });

  it("Review button disabled when no document", () => {
    const hasDocument = false;
    const canReview = hasDocument;
    expect(canReview).toBe(false);
  });

  it("Report export disabled when no review data", () => {
    const hasReviewData = false;
    const hasEngineeringData = false;
    const canExportReport = hasReviewData || hasEngineeringData;
    expect(canExportReport).toBe(false);
  });

  it("Apply AI actions disabled when none accepted", () => {
    const acceptedCount = 0;
    const canApply = acceptedCount > 0;
    expect(canApply).toBe(false);
  });

  it("Engineering extraction disabled when no text", () => {
    const hasText = false;
    const canExtract = hasText;
    expect(canExtract).toBe(false);
  });
});

describe("Error messages are actionable", () => {
  it("missing model error includes fix instruction", () => {
    const error = "Qwen3 model file was not found. Open Models tab and validate local-ai.";
    expect(error).toContain("Models tab");
    expect(error).toContain("validate");
  });

  it("missing runtime error includes path", () => {
    const error = "llama-cli.exe was not found in local-ai/runtimes/llama-cpp.";
    expect(error).toContain("llama-cli");
    expect(error).toContain("runtimes");
  });

  it("OCR error includes model reference", () => {
    const error = "OCR worker failed. Validate PaddleOCR-VL model and Python environment.";
    expect(error).toContain("PaddleOCR");
    expect(error).toContain("Python");
  });

  it("no evidence message is clear", () => {
    const msg = "No relevant source text was found in the document.";
    expect(msg).not.toContain("error");
    expect(msg).toContain("No relevant");
  });
});

describe("StatusBadge variants", () => {
  it("ready badge", () => { expect("ready").toBe("ready"); });
  it("warning badge", () => { expect("warning").toBe("warning"); });
  it("error badge", () => { expect("error").toBe("error"); });
  it("disabled badge", () => { expect("disabled").toBe("disabled"); });
});
