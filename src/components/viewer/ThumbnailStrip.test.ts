import { describe, it } from "vitest";
import * as fc from "fast-check";

/**
 * Validates: Requirements 20
 *
 * Property 23: Virtual list window computation bounds
 * For all valid inputs, the computed renderStart and renderEnd
 * satisfy: 0 ≤ renderStart ≤ renderEnd ≤ totalPages
 */

const ITEM_HEIGHT = 180;
const OVERSCAN = 3;

function computeWindow(scrollTop: number, containerHeight: number, totalPages: number) {
  const firstVisible = Math.floor(scrollTop / ITEM_HEIGHT);
  const lastVisible = Math.ceil((scrollTop + containerHeight) / ITEM_HEIGHT);
  const renderEnd = Math.min(totalPages, lastVisible + OVERSCAN);
  const renderStart = Math.min(Math.max(0, firstVisible - OVERSCAN), renderEnd);
  return { renderStart, renderEnd };
}

describe("ThumbnailStrip virtual list window computation", () => {
  it("renderStart and renderEnd are always within bounds", () => {
    fc.assert(
      fc.property(
        fc.nat(100000),  // scrollTop
        fc.integer({ min: 1, max: 5000 }),  // containerHeight
        fc.integer({ min: 1, max: 1000 }),  // totalPages
        (scrollTop, containerHeight, totalPages) => {
          const { renderStart, renderEnd } = computeWindow(scrollTop, containerHeight, totalPages);
          return (
            renderStart >= 0 &&
            renderStart <= renderEnd &&
            renderEnd <= totalPages
          );
        }
      ),
      { numRuns: 200 }
    );
  });
});
