import { describe, it, expect } from "vitest";

/**
 * Validates: Requirements 16.2
 * Property 26: URL validation regex accepts localhost-only http/https URLs
 * and rejects external, empty, non-HTTP, and malformed URLs.
 */

const URL_REGEX = /^https?:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/[^\s]*)?$/;

describe("AI Settings URL validation regex", () => {
  it.each([
    "http://localhost:11434",
    "https://localhost:11434/api/tags",
    "http://127.0.0.1:18080/v1/embeddings",
  ])("accepts valid URL: %s", (url) => {
    expect(URL_REGEX.test(url)).toBe(true);
  });

  it.each([
    "",
    "ftp://example.com",
    "https://example.com",
    "http://192.168.1.1:8080/api",
    "https://my-server.local:3000/v1",
    "not a url",
    "://missing-scheme",
    "   ",
  ])("rejects invalid URL: %s", (url) => {
    expect(URL_REGEX.test(url)).toBe(false);
  });
});
