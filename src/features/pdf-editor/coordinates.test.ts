import { describe, it, expect } from "vitest";
import { pdfRectToScreen, screenPointToPdf, screenRectToPdf } from "./coordinates";

describe("pdfRectToScreen", () => {
  const page = { widthPts: 612, heightPts: 792 };

  it("converts a rect at bottom-left of page to top-right area on screen", () => {
    // PDF rect at bottom-left: x=0, y=0, 100x50
    // Screen: x=0, y=(792-0-50)*1 = 742, w=100, h=50
    const result = pdfRectToScreen({ x: 0, y: 0, width: 100, height: 50 }, page, { zoom: 1 });
    expect(result.x).toBe(0);
    expect(result.y).toBe(742);
    expect(result.width).toBe(100);
    expect(result.height).toBe(50);
  });

  it("converts a rect at top-left of page to top-left on screen", () => {
    // PDF rect at top-left: x=0, y=742, 100x50 (y+h = 792 = page top)
    // Screen: x=0, y=(792-742-50)*1 = 0
    const result = pdfRectToScreen({ x: 0, y: 742, width: 100, height: 50 }, page, { zoom: 1 });
    expect(result.x).toBe(0);
    expect(result.y).toBe(0);
    expect(result.width).toBe(100);
    expect(result.height).toBe(50);
  });

  it("scales with zoom", () => {
    const result = pdfRectToScreen({ x: 100, y: 100, width: 50, height: 30 }, page, { zoom: 2 });
    expect(result.x).toBe(200);
    expect(result.width).toBe(100);
    expect(result.height).toBe(60);
    // y = (792 - 100 - 30) * 2 = 662 * 2 = 1324
    expect(result.y).toBe(1324);
  });
});

describe("screenPointToPdf", () => {
  const page = { widthPts: 612, heightPts: 792 };

  it("converts screen top-left to PDF top-left", () => {
    // Screen (0, 0) at zoom 1 → PDF (0, 792) which is top-left in PDF space
    const result = screenPointToPdf(0, 0, page, { zoom: 1 });
    expect(result.x).toBe(0);
    expect(result.y).toBe(792);
  });

  it("converts screen bottom-left to PDF bottom-left", () => {
    // Screen (0, 792) at zoom 1 → PDF (0, 0)
    const result = screenPointToPdf(0, 792, page, { zoom: 1 });
    expect(result.x).toBe(0);
    expect(result.y).toBe(0);
  });

  it("accounts for zoom", () => {
    // Screen (200, 400) at zoom 2 → PDF (100, 792 - 200) = (100, 592)
    const result = screenPointToPdf(200, 400, page, { zoom: 2 });
    expect(result.x).toBe(100);
    expect(result.y).toBe(592);
  });
});

describe("screenRectToPdf", () => {
  const page = { widthPts: 612, heightPts: 792 };

  it("round-trips through pdfRectToScreen and back", () => {
    const original = { x: 100, y: 200, width: 150, height: 60 };
    const screen = pdfRectToScreen(original, page, { zoom: 1.5 });
    const roundTripped = screenRectToPdf(screen, page, { zoom: 1.5 });

    expect(roundTripped.x).toBeCloseTo(original.x, 5);
    expect(roundTripped.y).toBeCloseTo(original.y, 5);
    expect(roundTripped.width).toBeCloseTo(original.width, 5);
    expect(roundTripped.height).toBeCloseTo(original.height, 5);
  });
});
