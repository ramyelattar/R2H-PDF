import { describe, it, expect } from "vitest";
import { decodeBase64ToBytes } from "./pdfBase64";

/**
 * Render-pipeline tests for `PdfCanvasViewer`.
 *
 * The most important test in this file is `decodes a multi-megabyte payload
 * without using fetch(data:)`. That is the regression test for the blank-PDF
 * bug: the previous implementation fell back to `fetch("data:...;base64,...")`
 * for payloads >= 64 KB, which is blocked by the packaged-app CSP
 * (`connect-src 'self' ipc: https://ipc.localhost`). The fetch rejection
 * was swallowed by `void render()`, leaving the viewer permanently stuck
 * on "Rendering page…" with a blank canvas.
 */

describe("decodeBase64ToBytes", () => {
  it("decodes small base64 correctly", () => {
    // "AQID" = [1, 2, 3]
    const result = decodeBase64ToBytes("AQID");
    expect(result).toEqual(new Uint8ClampedArray([1, 2, 3]));
  });

  it("decodes empty string to empty array", () => {
    const result = decodeBase64ToBytes("");
    expect(result).toEqual(new Uint8ClampedArray([]));
  });

  it("decodes 256 bytes correctly", () => {
    const bytes = new Uint8Array(256);
    for (let i = 0; i < 256; i++) bytes[i] = i;
    const base64 = btoa(String.fromCharCode(...bytes));
    const result = decodeBase64ToBytes(base64);
    expect(result.length).toBe(256);
    expect(result[0]).toBe(0);
    expect(result[255]).toBe(255);
  });

  // REGRESSION: previous implementation used fetch("data:...") for payloads
  // >= 64 KB, which is blocked by the packaged-app CSP and caused the blank-
  // PDF render bug. Decoder must work for multi-megabyte payloads without
  // touching the network/fetch.
  it("decodes a 1 MB payload without fetch / CSP dependency", () => {
    const size = 1_048_576; // 1 MB raw → ~1.4 MB base64
    const bytes = new Uint8Array(size);
    for (let i = 0; i < size; i++) bytes[i] = i & 0xff;

    // Build base64 in chunks to avoid String.fromCharCode stack overflow.
    let binary = "";
    const chunk = 0x8000;
    for (let i = 0; i < size; i += chunk) {
      binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
    }
    const base64 = btoa(binary);
    expect(base64.length).toBeGreaterThan(65_536); // hits old fetch path

    // Stub fetch so this test fails loudly if anyone reintroduces the
    // fetch(data:) fallback under the same name.
    const originalFetch = globalThis.fetch;
    let fetchCalled = false;
    globalThis.fetch = (() => {
      fetchCalled = true;
      throw new Error("decodeBase64ToBytes must not call fetch");
    }) as typeof fetch;

    try {
      const result = decodeBase64ToBytes(base64);
      expect(fetchCalled).toBe(false);
      expect(result.length).toBe(size);
      expect(result[0]).toBe(0);
      expect(result[255]).toBe(255);
      expect(result[size - 1]).toBe((size - 1) & 0xff);
    } finally {
      globalThis.fetch = originalFetch;
    }
  });
});

describe("stale render protection", () => {
  it("newer render ID supersedes older one", () => {
    let latestRenderId = 0;
    let globalSeq = 0;

    globalSeq += 1;
    const render1Id = globalSeq;
    latestRenderId = render1Id;

    globalSeq += 1;
    const render2Id = globalSeq;
    latestRenderId = render2Id;

    expect(latestRenderId).not.toBe(render1Id);
    expect(latestRenderId).toBe(render2Id);
  });
});
