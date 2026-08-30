/**
 * Pure helpers for the Acrobat-style canvas hit-test overlay. Extracted so
 * the bbox → screen-position math can be unit-tested without spinning up
 * the full CenterWorkspace component tree.
 *
 * Coordinate convention: PDF user-space, origin bottom-left, Y up. All
 * content-object bboxes in the app are normalised to this form, so the
 * Y-flip below is the *only* place where we convert to screen coords.
 */
export interface HitBoxStyle {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** Minimum size for a hit-test box (4px). Below this users can't click. */
const MIN_HIT_SIZE = 4;

/**
 * Project a PDF user-space bbox onto the screen, given:
 *   - `pageHeightPts` — page height in PDF points (zoom-independent).
 *   - `zoom`         — the current viewer zoom factor (1.0 = 100%).
 *
 * Returns CSS pixels relative to the rendered canvas top-left corner.
 */
export function projectPdfBboxToScreen(
  bbox: [number, number, number, number],
  pageHeightPts: number,
  zoom: number,
): HitBoxStyle {
  const [x0, y0, x1, y1] = bbox;
  const left = x0 * zoom;
  const top = (pageHeightPts - y1) * zoom;
  const width = Math.max(MIN_HIT_SIZE, (x1 - x0) * zoom);
  const height = Math.max(MIN_HIT_SIZE, (y1 - y0) * zoom);
  return { left, top, width, height };
}
