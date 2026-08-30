# R2H PDF — Local OCR Worker

`paddleocr_vl_worker.py` is the offline worker boundary for the canonical R2H OCR workflow. It accepts a structured operation request on stdin (or from `--input`) and returns one structured JSON result on stdout. It makes no network calls and never falls back to cloud OCR.

## Required local pack

The worker expects the external optional OCR pack at:

```text
local-ai/models/ocr/PaddleOCR-VL/
```

The supported runtime must provide Python, PyTorch, Pillow, Transformers, and the installed Transformers `PaddleOCRVLForConditionalGeneration` class. The worker prefers that built-in class over the checkpoint's older remote-code copy, which targets a removed RoPE API. The model files themselves are not modified or replaced.

## Request

Production requests include operation and active-page context:

```json
{
  "image_path": "C:/temporary/r2h-ocr-operation.ppm",
  "model_path": "C:/supported/local-ai/models/ocr/PaddleOCR-VL",
  "schema_version": 1,
  "output_format_version": 1,
  "operation_id": "ocr-operation-id",
  "page_index": 0,
  "language": "auto",
  "model_id": "PaddleOCR-VL",
  "timeout_secs": 120,
  "cancellation_id": "ocr-operation-id"
}
```

The page image must come from the active native PDF render. A UI screenshot, thumbnail, synthetic route, or stale session is not a valid source.

## Result truthfulness

Results carry `schema_version`, `status`, operation/page association, worker/model metadata, text, and blocks. `no_text_detected` is distinct from failure. Confidence is omitted unless the model supplies a real value.

The current Transformers PaddleOCR-VL text-generation route returns text but does not expose model-supplied OCR block geometry. The adapter therefore returns `GEOMETRY_UNAVAILABLE` for non-empty text without boxes; it never manufactures full-page or line-wide rectangles. Such a result cannot be accepted as an editor overlay.

The explicit `template_ocr` engine is restricted to the release-smoke harness and is not selected by production construction. It is not a production OCR fallback.

## Commands

```powershell
python paddleocr_vl_worker.py --version
python paddleocr_vl_worker.py --input request.json
```

The native controller owns timeout, cancellation, non-zero-exit, malformed-output, and temporary-input cleanup behavior.
