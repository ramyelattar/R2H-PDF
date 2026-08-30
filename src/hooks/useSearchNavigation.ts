import { useState, useEffect } from "react";
import type { SearchMatch } from "../lib/ipc";

/**
 * Manages navigation state for a set of search matches.
 *
 * - `next()` advances to the next match with wraparound: `(i + 1) % n`
 * - `previous()` goes back with wraparound: `(i - 1 + n) % n`
 * - `activeMatch` is the currently focused `SearchMatch`, or `null` when
 *   the matches array is empty.
 * - `activeIndex` resets to 0 whenever the `matches` array reference changes.
 *
 * Req 3.1, 3.2
 */
export const useSearchNavigation = (matches: SearchMatch[]) => {
  const [activeIndex, setActiveIndex] = useState(0);

  // Reset to the first match whenever the result set changes.
  useEffect(() => {
    setActiveIndex(0);
  }, [matches]);

  const next = () => {
    if (matches.length === 0) return;
    setActiveIndex((i) => (i + 1) % matches.length);
  };

  const previous = () => {
    if (matches.length === 0) return;
    setActiveIndex((i) => (i - 1 + matches.length) % matches.length);
  };

  return {
    activeIndex,
    next,
    previous,
    activeMatch: matches[activeIndex] ?? null,
  };
};
