/**
 * Tests for useSearchNavigation hook
 *
 * Validates: Requirements 3.1, 3.2 (Property 5)
 *
 * Covers:
 *  - next() advances index
 *  - next() wraps from last to first (end boundary wraparound)
 *  - previous() decrements index
 *  - previous() wraps from first to last (start boundary wraparound)
 *  - next() and previous() are no-ops when matches is empty
 *  - activeMatch returns the correct match at activeIndex
 *  - activeIndex resets to 0 when matches array changes
 */

import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import type { SearchMatch } from "../lib/ipc";
import { useSearchNavigation } from "./useSearchNavigation";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const makeBBox = () => ({ x0: 0, y0: 0, x1: 10, y1: 10 });

const makeMatch = (page_index: number, match_index: number): SearchMatch => ({
  page_index,
  match_index,
  snippet: `match-${page_index}-${match_index}`,
  bbox: makeBBox(),
});

const THREE_MATCHES: SearchMatch[] = [
  makeMatch(0, 0),
  makeMatch(0, 1),
  makeMatch(1, 0),
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("useSearchNavigation", () => {
  // -------------------------------------------------------------------------
  // next()
  // -------------------------------------------------------------------------

  it("next() advances activeIndex from 0 to 1 with multiple matches", () => {
    const { result } = renderHook(() => useSearchNavigation(THREE_MATCHES));

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.next();
    });

    expect(result.current.activeIndex).toBe(1);
  });

  it("next() wraps from last index back to 0 (end boundary wraparound)", () => {
    const { result } = renderHook(() => useSearchNavigation(THREE_MATCHES));

    // Advance to the last match (index 2)
    act(() => {
      result.current.next(); // 0 → 1
    });
    act(() => {
      result.current.next(); // 1 → 2
    });

    expect(result.current.activeIndex).toBe(2);

    // One more next() should wrap to 0
    act(() => {
      result.current.next(); // 2 → 0 (wraparound)
    });

    expect(result.current.activeIndex).toBe(0);
  });

  // -------------------------------------------------------------------------
  // previous()
  // -------------------------------------------------------------------------

  it("previous() decrements activeIndex from 1 to 0", () => {
    const { result } = renderHook(() => useSearchNavigation(THREE_MATCHES));

    // Move to index 1 first
    act(() => {
      result.current.next();
    });
    expect(result.current.activeIndex).toBe(1);

    act(() => {
      result.current.previous();
    });

    expect(result.current.activeIndex).toBe(0);
  });

  it("previous() wraps from index 0 to last index (start boundary wraparound)", () => {
    const { result } = renderHook(() => useSearchNavigation(THREE_MATCHES));

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.previous(); // 0 → 2 (wraparound)
    });

    expect(result.current.activeIndex).toBe(THREE_MATCHES.length - 1);
  });

  // -------------------------------------------------------------------------
  // Empty matches — no-ops
  // -------------------------------------------------------------------------

  it("next() is a no-op when matches is empty", () => {
    const { result } = renderHook(() => useSearchNavigation([]));

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.next();
    });

    expect(result.current.activeIndex).toBe(0);
  });

  it("previous() is a no-op when matches is empty", () => {
    const { result } = renderHook(() => useSearchNavigation([]));

    expect(result.current.activeIndex).toBe(0);

    act(() => {
      result.current.previous();
    });

    expect(result.current.activeIndex).toBe(0);
  });

  // -------------------------------------------------------------------------
  // activeMatch
  // -------------------------------------------------------------------------

  it("activeMatch returns the match at activeIndex", () => {
    const { result } = renderHook(() => useSearchNavigation(THREE_MATCHES));

    expect(result.current.activeMatch).toBe(THREE_MATCHES[0]);

    act(() => {
      result.current.next();
    });

    expect(result.current.activeMatch).toBe(THREE_MATCHES[1]);
  });

  it("activeMatch returns null when matches is empty", () => {
    const { result } = renderHook(() => useSearchNavigation([]));

    expect(result.current.activeMatch).toBeNull();
  });

  // -------------------------------------------------------------------------
  // activeIndex reset on matches change
  // -------------------------------------------------------------------------

  it("activeIndex resets to 0 when the matches array reference changes", () => {
    let matches = THREE_MATCHES;
    const { result, rerender } = renderHook(() =>
      useSearchNavigation(matches)
    );

    // Advance to index 2
    act(() => {
      result.current.next();
      result.current.next();
    });
    expect(result.current.activeIndex).toBe(2);

    // Swap in a new matches array reference
    const NEW_MATCHES: SearchMatch[] = [makeMatch(2, 0), makeMatch(2, 1)];
    matches = NEW_MATCHES;
    rerender();

    expect(result.current.activeIndex).toBe(0);
    expect(result.current.activeMatch).toBe(NEW_MATCHES[0]);
  });
});
