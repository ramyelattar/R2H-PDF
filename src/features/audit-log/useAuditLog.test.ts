import { describe, it, expect, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useAuditLog } from "./useAuditLog";

describe("useAuditLog", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("starts empty when no stored data", () => {
    const { result } = renderHook(() => useAuditLog());
    expect(result.current.entries).toEqual([]);
  });

  it("records a create action", () => {
    const { result } = renderHook(() => useAuditLog());
    act(() => { result.current.record("object_created", "obj-1", "textBox", 0, "Created textBox"); });
    expect(result.current.entries.length).toBe(1);
    expect(result.current.entries[0].action).toBe("object_created");
    expect(result.current.entries[0].objectId).toBe("obj-1");
  });

  it("records move and delete actions", () => {
    const { result } = renderHook(() => useAuditLog());
    act(() => { result.current.record("object_moved", "obj-1", "textBox", 0, "Moved"); });
    act(() => { result.current.record("object_deleted", "obj-1", "textBox", 0, "Deleted"); });
    expect(result.current.entries.length).toBe(2);
    expect(result.current.entries[0].action).toBe("object_moved");
    expect(result.current.entries[1].action).toBe("object_deleted");
  });

  it("persists to localStorage", () => {
    const { result } = renderHook(() => useAuditLog());
    act(() => { result.current.record("object_created", "obj-1", "textBox", 0, "Created"); });

    // Re-mount the hook to simulate page reload.
    const { result: result2 } = renderHook(() => useAuditLog());
    expect(result2.current.entries.length).toBe(1);
    expect(result2.current.entries[0].objectId).toBe("obj-1");
  });

  it("clears all entries", () => {
    const { result } = renderHook(() => useAuditLog());
    act(() => { result.current.record("object_created", "obj-1", "textBox", 0, "Created"); });
    act(() => { result.current.clear(); });
    expect(result.current.entries).toEqual([]);
  });
});
