import type { OcrOverlaySpec } from "../../lib/ipc";
import type { TextBoxObject } from "../pdf-editor/types";
import type { OcrPageResult } from "./types";

export type OcrValidationResult =
  | { ok: true; blockCount: number }
  | { ok: false; code: string; message: string };

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

export function validateOcrResult(result: unknown): OcrValidationResult {
  if (!result || typeof result !== "object") {
    return { ok: false, code: "OCR_MALFORMED_RESULT", message: "OCR returned no structured result." };
  }
  const candidate = result as Partial<OcrPageResult>;
  if (candidate.schema_version !== 1 || typeof candidate.operation_id !== "string" || !candidate.operation_id.trim()
    || typeof candidate.document_id !== "string" || !candidate.document_id.trim()
    || typeof candidate.session_id !== "string" || !candidate.session_id.trim()
    || typeof candidate.page_index !== "number" || !Number.isInteger(candidate.page_index)
    || typeof candidate.rotation_degrees !== "number" || !Number.isFinite(candidate.rotation_degrees)) {
    return { ok: false, code: "OCR_UNSUPPORTED_SCHEMA", message: "OCR result schema is unsupported." };
  }
  if (candidate.status !== "completed" && candidate.status !== "no_text_detected") {
    return { ok: false, code: "OCR_UNSUPPORTED_STATUS", message: "OCR returned an unsupported status." };
  }
  if (!Array.isArray(candidate.blocks) || !isFiniteNumber(candidate.page_width_points) || !isFiniteNumber(candidate.page_height_points)
    || candidate.page_width_points <= 0 || candidate.page_height_points <= 0
    || !isFiniteNumber(candidate.image_width_px) || !isFiniteNumber(candidate.image_height_px)
    || candidate.image_width_px <= 0 || candidate.image_height_px <= 0) {
    return { ok: false, code: "OCR_INVALID_RESULT", message: "OCR result dimensions are invalid." };
  }
  if (candidate.bbox_coordinate_space !== "image_px") {
    return { ok: false, code: "OCR_UNSUPPORTED_COORDINATE_SPACE", message: "OCR geometry must be expressed in rendered image pixels." };
  }
  if (![0, 90, 180, 270].includes(candidate.rotation_degrees)) {
    return { ok: false, code: "OCR_INVALID_ROTATION", message: "OCR page rotation must be a normalized quarter-turn." };
  }
  if (candidate.status === "no_text_detected") {
    return candidate.text?.trim() || candidate.blocks.length
      ? { ok: false, code: "OCR_INVALID_NO_TEXT_RESULT", message: "no_text_detected contained text or blocks." }
      : { ok: true, blockCount: 0 };
  }
  if (!candidate.text?.trim()) {
    return { ok: false, code: "OCR_EMPTY_TEXT", message: "OCR completed without text." };
  }
  if (candidate.blocks.length === 0) {
    return { ok: false, code: "OCR_GEOMETRY_UNAVAILABLE", message: "OCR text has no model-supplied geometry." };
  }
  const ids = new Set<string>();
  for (const block of candidate.blocks) {
    if (!block || typeof block.id !== "string" || !block.id.trim() || ids.has(block.id)) {
      return { ok: false, code: "OCR_INVALID_BLOCK", message: "OCR block IDs must be non-empty and unique." };
    }
    ids.add(block.id);
    if (!block.text?.trim()) {
      return { ok: false, code: "OCR_INVALID_BLOCK", message: "OCR blocks must contain text." };
    }
    const bbox = block.bbox;
    if (!bbox || !isFiniteNumber(bbox.x) || !isFiniteNumber(bbox.y) || !isFiniteNumber(bbox.width) || !isFiniteNumber(bbox.height)
      || bbox.width <= 0 || bbox.height <= 0 || bbox.x < 0 || bbox.y < 0
      || bbox.x + bbox.width > candidate.image_width_px || bbox.y + bbox.height > candidate.image_height_px) {
      return { ok: false, code: "OCR_INVALID_GEOMETRY", message: "OCR block geometry is invalid or outside the rendered page." };
    }
    if (block.confidence !== null && block.confidence !== undefined
      && (!isFiniteNumber(block.confidence) || block.confidence < 0 || block.confidence > 1)) {
      return { ok: false, code: "OCR_INVALID_CONFIDENCE", message: "OCR confidence is not a real value in the range 0..1." };
    }
  }
  if (candidate.confidence !== null && candidate.confidence !== undefined
    && (!isFiniteNumber(candidate.confidence) || candidate.confidence < 0 || candidate.confidence > 1)) {
    return { ok: false, code: "OCR_INVALID_CONFIDENCE", message: "OCR aggregate confidence is invalid." };
  }
  return { ok: true, blockCount: candidate.blocks.length };
}

export function isOcrPageResult(result: unknown): result is OcrPageResult {
  return validateOcrResult(result).ok;
}

const stableIdPart = (value: string): string => {
  const clean = value.replace(/[^a-zA-Z0-9_-]/g, "").slice(0, 48);
  return clean || "unknown";
};

export function createOcrEditorObjects(
  result: OcrPageResult,
  overlays: OcrOverlaySpec[],
  zIndexStart: number,
): TextBoxObject[] {
  const byText = new Map(result.blocks.map((block) => [block.text, block]));
  const now = Date.now();
  return overlays
    .filter((overlay) => overlay.page_index === result.page_index && overlay.text.trim().length > 0)
    .map((overlay, index) => {
      const block = byText.get(overlay.text);
      const blockId = overlay.block_id ?? block?.id ?? overlay.id;
      const [x0, y0, x1, y1] = overlay.bbox;
      const width = x1 - x0;
      const height = y1 - y0;
      return {
        id: `ocr-${stableIdPart(result.document_id)}-p${result.page_index}-${stableIdPart(blockId)}`,
        sessionId: result.session_id,
        pageIndex: result.page_index,
        type: "textBox",
        rect: { x: x0, y: y0, width, height },
        rotation: result.rotation_degrees,
        zIndex: zIndexStart + index,
        locked: false,
        hidden: false,
        createdAt: now,
        updatedAt: now,
        metadata: {
          source: "ocr",
          ocr_block_id: blockId,
          ocr_confidence: overlay.confidence ?? null,
          ocr_operation_id: result.operation_id,
          ocr_language: result.language,
        },
        text: overlay.text,
        fontSize: Math.max(6, Math.min(72, height * 0.72)),
        fontFamily: "Helvetica",
        color: "#111111",
        backgroundColor: "rgba(230,245,255,0.84)",
        borderColor: "rgba(38,116,170,0.55)",
      } satisfies TextBoxObject;
    });
}
