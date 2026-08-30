import { describe, it, expect } from "vitest";
import { projectPdfBboxToScreen } from "./canvasHitTest";

describe("projectPdfBboxToScreen — Acrobat-UX bbox → screen math", () => {
  it("flips PDF Y-up to screen Y-down for a US Letter page at zoom 1", () => {
    const r = projectPdfBboxToScreen([72, 700, 200, 720], 792, 1);
    expect(r.left).toBe(72);
    // pageH - y1 = 792 - 720 = 72
    expect(r.top).toBe(72);
    expect(r.width).toBe(128);
    expect(r.height).toBe(20);
  });

  it("scales by zoom (regression: heightPts must be in points, not pixels)", () => {
    const r = projectPdfBboxToScreen([72, 700, 200, 720], 792, 1.5);
    expect(r.left).toBe(108);
    expect(r.top).toBe(108);
    expect(r.width).toBe(192);
    expect(r.height).toBe(30);
  });

  it("places a span near the bottom of the page at the bottom of the screen", () => {
    // Bottom-of-page text in PDF user space has small y values (close to 0).
    const r = projectPdfBboxToScreen([72, 20, 200, 40], 792, 1);
    // pageH - y1 = 792 - 40 = 752
    expect(r.top).toBe(752);
  });

  it("places a span near the top of the page near the top of the screen", () => {
    // Top-of-page text in PDF user space has large y values (close to pageH).
    const r = projectPdfBboxToScreen([72, 750, 200, 780], 792, 1);
    // pageH - y1 = 792 - 780 = 12
    expect(r.top).toBe(12);
  });

  it("enforces a 4px minimum hit size so micro-glyphs stay clickable", () => {
    const r = projectPdfBboxToScreen([100, 700, 100.5, 700.5], 792, 1);
    expect(r.width).toBe(4);
    expect(r.height).toBe(4);
  });
});
