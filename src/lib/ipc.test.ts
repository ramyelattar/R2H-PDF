import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock the Tauri invoke function
const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// Import after mock is set up
import { invokeSafe } from "./ipc";

describe("invokeSafe", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it("returns { ok: true, data } when invoke resolves", async () => {
    mockInvoke.mockResolvedValue({ session_id: "s1" });
    const result = await invokeSafe("doc_open", { path: "test.pdf" });
    expect(result).toEqual({ ok: true, data: { session_id: "s1" } });
  });

  it("returns { ok: false, error } when invoke rejects", async () => {
    mockInvoke.mockRejectedValue(new Error("session not found: s1"));
    const result = await invokeSafe("doc_close", { session_id: "s1" });
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe("SESSION_NOT_FOUND");
      expect(result.error.message).toContain("session not found");
    }
  });
});
