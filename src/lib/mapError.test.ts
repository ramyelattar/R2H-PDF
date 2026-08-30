import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { invokeSafe } from "./ipc";

describe("mapError error code mapping", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it.each([
    ["session not found: s1", "SESSION_NOT_FOUND"],
    ["page out of range: requested 5, total 3", "PAGE_OUT_OF_RANGE"],
    ["invalid query: empty", "INVALID_INPUT"],
    ["INVALID_REGEX: unclosed group", "INVALID_REGEX"],
    ["REGEX_TOO_COMPLEX", "REGEX_TOO_COMPLEX"],
    ["something completely unknown", "IPC_ERROR"],
  ])("maps '%s' to code '%s'", async (message, expectedCode) => {
    mockInvoke.mockRejectedValue(new Error(message));
    const result = await invokeSafe("any_command");
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.error.code).toBe(expectedCode);
    }
  });
});
