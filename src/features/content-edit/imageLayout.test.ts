import { describe, expect, it } from "vitest";
import { computeImageLayout, computeInsetCropRect, computeRotationPreview } from "./imageLayout";

describe("imageLayout shared preview math", () => {
  it("stretch preview equals target bbox", () => {
    expect(computeImageLayout([10, 20, 110, 70], "stretch", 400, 200)).toEqual({
      drawRect: [10, 20, 110, 70],
      clipRect: null,
    });
  });

  it("fit preview preserves aspect and centers in bbox", () => {
    expect(computeImageLayout([10, 20, 110, 70], "fit", 100, 200).drawRect).toEqual([47.5, 20, 72.5, 70]);
  });

  it("fill preview preserves aspect and clips to bbox", () => {
    const preview = computeImageLayout([10, 20, 110, 70], "fill", 100, 200);
    expect(preview.drawRect).toEqual([10, -55, 110, 145]);
    expect(preview.clipRect).toEqual([10, 20, 110, 70]);
  });

  it("crop preview request uses inset crop rect", () => {
    expect(computeInsetCropRect([0, 0, 200, 100], 10)).toEqual([20, 10, 180, 90]);
  });

  it("rotate preview matches backend centered matrix", () => {
    expect(computeRotationPreview([10, 20, 110, 70], 90)).toEqual({
      matrix: [0, 100, -50, 0, 85, -5],
      clipRect: [10, 20, 110, 70],
    });
  });
});
