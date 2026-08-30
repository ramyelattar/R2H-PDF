/**
 * Coordinate conversion utilities between PDF page space and screen space.
 *
 * PDF space: origin bottom-left, y-up, units in points (1pt = 1/72 in).
 * Screen space: origin top-left, y-down, units in CSS pixels.
 */

import type { EditorRect } from "./types";

export interface PageDimensions {
  /** Page width in PDF points. */
  widthPts: number;
  /** Page height in PDF points. */
  heightPts: number;
}

export interface ViewTransform {
  /** Current zoom factor (1.0 = 100%). */
  zoom: number;
  /** Device pixel ratio (for HiDPI). Only needed for canvas; DOM uses CSS px. */
  dpr?: number;
}

/**
 * Convert a PDF-space rect to a screen-space rect (CSS pixels).
 * Flips Y axis: PDF y-up → screen y-down.
 */
export function pdfRectToScreen(
  rect: EditorRect,
  page: PageDimensions,
  view: ViewTransform,
): EditorRect {
  const zoom = view.zoom;
  return {
    x: rect.x * zoom,
    // PDF y is from bottom; screen y is from top.
    y: (page.heightPts - rect.y - rect.height) * zoom,
    width: rect.width * zoom,
    height: rect.height * zoom,
  };
}

/**
 * Convert a screen-space point (CSS pixels) to PDF-space coordinates.
 * Useful for translating click positions to PDF object placement.
 */
export function screenPointToPdf(
  screenX: number,
  screenY: number,
  page: PageDimensions,
  view: ViewTransform,
): { x: number; y: number } {
  const zoom = view.zoom;
  return {
    x: screenX / zoom,
    // Invert Y: screen top → PDF bottom.
    y: page.heightPts - screenY / zoom,
  };
}

/**
 * Convert a screen-space rect to PDF-space rect.
 */
export function screenRectToPdf(
  rect: EditorRect,
  page: PageDimensions,
  view: ViewTransform,
): EditorRect {
  const zoom = view.zoom;
  const pdfWidth = rect.width / zoom;
  const pdfHeight = rect.height / zoom;
  const pdfX = rect.x / zoom;
  const pdfY = page.heightPts - rect.y / zoom - pdfHeight;
  return { x: pdfX, y: pdfY, width: pdfWidth, height: pdfHeight };
}
