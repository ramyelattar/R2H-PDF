import { describe, it, expect } from "vitest";

/**
 * Phase 1 tests for loading state transitions.
 *
 * Validates that:
 * - New PDF tabs start with loadState = "loading"
 * - First successful render flips to "ready"
 * - Render failure flips to "error"
 */

type LoadState = "empty" | "loading" | "ready" | "error";

interface TabState {
  loadState: LoadState;
  errorMessage?: string;
}

function createTab(): TabState {
  return { loadState: "loading" };
}

function onRenderSuccess(tab: TabState): TabState {
  if (tab.loadState === "loading") {
    return { ...tab, loadState: "ready", errorMessage: undefined };
  }
  return tab;
}

function onRenderError(tab: TabState, message: string): TabState {
  if (tab.loadState === "loading") {
    return { ...tab, loadState: "error", errorMessage: message };
  }
  return tab;
}

describe("loading state transitions", () => {
  it("new tab starts in loading state", () => {
    const tab = createTab();
    expect(tab.loadState).toBe("loading");
  });

  it("first render success transitions to ready", () => {
    const tab = createTab();
    const updated = onRenderSuccess(tab);
    expect(updated.loadState).toBe("ready");
  });

  it("render error transitions to error with message", () => {
    const tab = createTab();
    const updated = onRenderError(tab, "Page out of range");
    expect(updated.loadState).toBe("error");
    expect(updated.errorMessage).toBe("Page out of range");
  });

  it("subsequent render success does not change ready state", () => {
    let tab = createTab();
    tab = onRenderSuccess(tab);
    expect(tab.loadState).toBe("ready");
    // Second render success should not change anything
    tab = onRenderSuccess(tab);
    expect(tab.loadState).toBe("ready");
  });

  it("render error after ready does not regress state", () => {
    let tab = createTab();
    tab = onRenderSuccess(tab);
    // Error after ready should not change state (only loading → error)
    tab = onRenderError(tab, "Some error");
    expect(tab.loadState).toBe("ready");
  });
});
