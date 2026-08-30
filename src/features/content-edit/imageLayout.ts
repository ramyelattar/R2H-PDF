export type ImageLayoutMode = "stretch" | "fit" | "fill";
export type PdfRect = [number, number, number, number];

export interface StagedImageReplacement {
  sessionId: string;
  pageIndex: number;
  contentObjectId: string;
  bbox: PdfRect;
  dataUrl: string;
  imageBytesBase64: string;
  layoutMode: ImageLayoutMode;
  naturalWidth: number;
  naturalHeight: number;
  preserveAspect: boolean;
}

export interface ImageDrawPreview {
  drawRect: PdfRect;
  clipRect: PdfRect | null;
}

export interface RotationPreview {
  matrix: [number, number, number, number, number, number];
  clipRect: PdfRect;
}

export function computeImageLayout(
  bbox: PdfRect,
  mode: ImageLayoutMode,
  imageWidth: number,
  imageHeight: number,
): ImageDrawPreview {
  validateRect(bbox, "image bbox");
  if (imageWidth <= 0 || imageHeight <= 0 || !Number.isFinite(imageWidth) || !Number.isFinite(imageHeight)) {
    throw new Error("replacement image dimensions are invalid");
  }
  const bboxW = bbox[2] - bbox[0];
  const bboxH = bbox[3] - bbox[1];
  const imageRatio = imageWidth / imageHeight;
  const bboxRatio = bboxW / bboxH;
  let drawW = bboxW;
  let drawH = bboxH;
  let clipRect: PdfRect | null = null;

  if (mode === "fit") {
    if (imageRatio > bboxRatio) drawH = bboxW / imageRatio;
    else drawW = bboxH * imageRatio;
  } else if (mode === "fill") {
    clipRect = bbox;
    if (imageRatio > bboxRatio) drawW = bboxH * imageRatio;
    else drawH = bboxW / imageRatio;
  }

  const x = bbox[0] + (bboxW - drawW) / 2;
  const y = bbox[1] + (bboxH - drawH) / 2;
  return { drawRect: [x, y, x + drawW, y + drawH], clipRect };
}

export function computeInsetCropRect(bbox: PdfRect, insetPercent: number): PdfRect {
  validateRect(bbox, "image bbox");
  const inset = Math.max(0, Math.min(45, insetPercent)) / 100;
  const w = bbox[2] - bbox[0];
  const h = bbox[3] - bbox[1];
  const crop: PdfRect = [bbox[0] + w * inset, bbox[1] + h * inset, bbox[2] - w * inset, bbox[3] - h * inset];
  validateRect(crop, "crop rect");
  return crop;
}

export function computeRotationPreview(rect: PdfRect, degrees: -90 | 90 | 180 | 270): RotationPreview {
  validateRect(rect, "rotation rect");
  const normalized = ((degrees % 360) + 360) % 360;
  const [cos, sin] =
    normalized === 90 ? [0, 1] :
    normalized === 180 ? [-1, 0] :
    normalized === 270 ? [0, -1] :
    [1, 0];
  const w = rect[2] - rect[0];
  const h = rect[3] - rect[1];
  const a = w * cos;
  const b = w * sin;
  const c = -h * sin;
  const d = h * cos;
  const cx = rect[0] + w / 2;
  const cy = rect[1] + h / 2;
  const e = cx - (a + c) / 2;
  const f = cy - (b + d) / 2;
  return { matrix: [a, b, c, d, e, f], clipRect: rect };
}

function validateRect(rect: PdfRect, label: string) {
  if (rect.some((v) => !Number.isFinite(v)) || rect[2] <= rect[0] || rect[3] <= rect[1]) {
    throw new Error(`${label} is degenerate`);
  }
}
