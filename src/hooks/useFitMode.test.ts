import { describe, it, expect } from "vitest";
import { calculateFitToPageZoom, calculateFitToWidthZoom, clampZoom } from "../lib/fitMode";

/**
 * Phase 1/2 tests for fit-to-width and fit-to-page zoom calculations.
 * Now tests the pure utility functions in src/lib/fitMode.ts.
 */

describe("clampZoom", () => {
  it("clamps below minimum to 25", () => {
    expect(clampZoom(5)).toBe(25);
  });

  it("clamps above maximum to 400", () => {
    expect(clampZoom(2000)).toBe(400);
  });

  it("rounds to nearest integer", () => {
    expect(clampZoom(99.6)).toBe(100);
    expect(clampZoom(99.4)).toBe(99);
  });

  it("passes through values in range", () => {
    expect(clampZoom(150)).toBe(150);
  });
});

describe("calculateFitToPageZoom", () => {
  it("calculates correct zoom for US Letter in 1024x768 viewport", () => {
    const zoom = calculateFitToPageZoom({
      viewportWidth: 1024,
      viewportHeight: 768,
      pageWidth: 612,
      pageHeight: 792,
    });
    // Width ratio: 1024/612 = 1.67, Height ratio: 768/792 = 0.97
    // Min is 0.97 → 97%
    expect(zoom).toBe(97);
  });

  it("calculates correct zoom for landscape page", () => {
    const zoom = calculateFitToPageZoom({
      viewportWidth: 1024,
      viewportHeight: 768,
      pageWidth: 792,
      pageHeight: 612,
    });
    // Width ratio: 1024/792 = 1.29, Height ratio: 768/612 = 1.25
    // Min is 1.25 → 125%
    expect(zoom).toBe(125);
  });

  it("clamps to minimum 25%", () => {
    const zoom = calculateFitToPageZoom({
      viewportWidth: 100,
      viewportHeight: 100,
      pageWidth: 2000,
      pageHeight: 2000,
    });
    expect(zoom).toBe(25);
  });

  it("clamps to maximum 400%", () => {
    const zoom = calculateFitToPageZoom({
      viewportWidth: 2000,
      viewportHeight: 2000,
      pageWidth: 100,
      pageHeight: 100,
    });
    expect(zoom).toBe(400);
  });
});

describe("calculateFitToWidthZoom", () => {
  it("calculates correct zoom for US Letter width in 1024 viewport", () => {
    const zoom = calculateFitToWidthZoom({ viewportWidth: 1024, pageWidth: 612 });
    // 1024/612 = 1.67 → 167%
    expect(zoom).toBe(167);
  });

  it("calculates correct zoom for narrow viewport", () => {
    const zoom = calculateFitToWidthZoom({ viewportWidth: 400, pageWidth: 612 });
    // 400/612 = 0.65 → 65%
    expect(zoom).toBe(65);
  });

  it("clamps to minimum 25%", () => {
    const zoom = calculateFitToWidthZoom({ viewportWidth: 50, pageWidth: 612 });
    expect(zoom).toBe(25);
  });
});
