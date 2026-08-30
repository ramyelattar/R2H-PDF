/**
 * Pure utility functions for fit-to-page and fit-to-width zoom calculations.
 */

export interface FitInput {
  viewportWidth: number;
  viewportHeight: number;
  pageWidth: number;
  pageHeight: number;
}

/** Clamp a zoom percentage to the allowed range [25, 400]. */
export function clampZoom(value: number): number {
  return Math.max(25, Math.min(400, Math.round(value)));
}

/** Calculate zoom percentage to fit the full page within the viewport. */
export function calculateFitToPageZoom(input: FitInput): number {
  const zoomW = input.viewportWidth / input.pageWidth;
  const zoomH = input.viewportHeight / input.pageHeight;
  return clampZoom(Math.min(zoomW, zoomH) * 100);
}

/** Calculate zoom percentage to fit the page width within the viewport. */
export function calculateFitToWidthZoom(input: Pick<FitInput, "viewportWidth" | "pageWidth">): number {
  return clampZoom((input.viewportWidth / input.pageWidth) * 100);
}
