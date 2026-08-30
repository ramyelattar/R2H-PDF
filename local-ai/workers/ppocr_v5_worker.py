"""PP-OCRv5 standard OCR worker for R2H PDF (fast CPU path).

Classic two-stage OCR pipeline, fully local/offline:

    page image -> PP-OCRv5_mobile_det (text boxes)
               -> crop regions -> arabic_PP-OCRv5_mobile_rec (batched CTC)
               -> text + bboxes + confidence (same wire schema as the
                  PaddleOCR-VL worker, so the Rust validation, overlay,
                  acceptance, persistence, and rollback stack is unchanged)

Wire protocol (identical contract to paddleocr_vl_worker.py):
  - request:  single JSON object on stdin (or --input <file>)
  - progress: JSON "stage" events, one per line, on stderr
  - result:   single JSON object on stdout; exit 0 on ok, 1 on failure

Only LOCAL model files are loaded (inference.json + inference.pdiparams).
The recognition dictionary is read from the model's own inference.yml.
No network access, no downloads, no Hugging Face.
"""

import argparse
import json
import math
import sys
import time
from contextlib import redirect_stdout
from pathlib import Path

PROCESS_STARTED_AT = time.perf_counter()
WORKER_VERSION = "1.0.0"
SCHEMA_VERSION = 1
ENGINE_NAME = "PP-OCRv5"

RECOGNITION_HEIGHT = 48
RECOGNITION_MAX_WIDTH = 320
DET_LONG_SIDE = 960
DET_PAD_MULTIPLE = 32

# Heavy dependencies are bound lazily by ensure_dependencies() so that
# `--version` and request validation work without the runtime installed.
cv2 = None
np = None


def ensure_dependencies():
    global cv2, np
    import cv2 as _cv2
    import numpy as _np

    cv2, np = _cv2, _np


# ---------------------------------------------------------------------------
# Stage events (stderr protocol)
# ---------------------------------------------------------------------------

def emit_stage(name, elapsed_ms=None, **extra):
    if elapsed_ms is None:
        elapsed_ms = (time.perf_counter() - PROCESS_STARTED_AT) * 1000
    event = {"event": "stage", "name": name, "elapsed_ms": round(elapsed_ms, 3)}
    if extra:
        event.update(extra)
    sys.stderr.write(json.dumps(event, separators=(",", ":")) + "\n")
    sys.stderr.flush()


# ---------------------------------------------------------------------------
# Result helpers
# ---------------------------------------------------------------------------

def make_error(code, message):
    return {
        "ok": False,
        "schema_version": SCHEMA_VERSION,
        "output_format_version": 1,
        "status": "failed",
        "worker_version": WORKER_VERSION,
        "engine": ENGINE_NAME,
        "error_code": code,
        "message": message,
        "details": "",
    }


def write_result_and_exit(result, code):
    sys.stdout.write(json.dumps(result, ensure_ascii=False))
    sys.stdout.write("\n")
    sys.stdout.flush()
    emit_stage("stdout_flush")
    sys.exit(code)


# ---------------------------------------------------------------------------
# Request validation
# ---------------------------------------------------------------------------

def validate_request(req):
    import os

    if "image_path" not in req:
        return "Missing required field: image_path"
    if not os.path.isfile(req["image_path"]):
        return f"Image file not found: {req['image_path']}"
    if "det_model_path" not in req:
        return "Missing required field: det_model_path"
    if "rec_model_path" not in req:
        return "Missing required field: rec_model_path"
    for key in ("det_model_path", "rec_model_path"):
        model_dir = Path(req[key])
        if not model_dir.is_dir():
            return f"Model directory not found: {model_dir}"
        if not (model_dir / "inference.pdiparams").is_file():
            return f"Model weights missing (inference.pdiparams) in: {model_dir}"
        has_program = (model_dir / "inference.json").is_file() or (
            model_dir / "inference.pdmodel"
        ).is_file()
        if not has_program:
            return f"Model program missing (inference.json) in: {model_dir}"
    return None


# ---------------------------------------------------------------------------
# Minimal YAML subset parser: enough for inference.yml `character_dict`
# (a list of single-character scalars) and top-level scalar keys.
# ---------------------------------------------------------------------------

def _parse_yaml_scalar(token):
    token = token.strip()
    if token.startswith("'") and token.endswith("'") and len(token) >= 2:
        return token[1:-1].replace("''", "'")
    if token.startswith('"') and token.endswith('"') and len(token) >= 2:
        return json.loads(token)
    return token


def parse_recognition_dict(inference_yml_path):
    """Extract PostProcess.character_dict (and use_space_char) from yml."""
    characters = []
    use_space_char = True
    in_dict = False
    with open(inference_yml_path, "r", encoding="utf-8") as handle:
        for raw_line in handle:
            line = raw_line.rstrip("\n")
            stripped = line.strip()
            if not stripped or stripped.startswith("#"):
                continue
            if in_dict:
                # A less-indented non-item line ends the list.
                if not line.startswith(" ") and not line.startswith("-"):
                    in_dict = False
                    continue
                if stripped.startswith("- "):
                    characters.append(_parse_yaml_scalar(stripped[2:]))
                    continue
                # Any other indented content inside the list is unexpected.
                in_dict = False
                continue
            if stripped.startswith("character_dict:"):
                in_dict = True
                continue
            if stripped.startswith("use_space_char:"):
                use_space_char = stripped.split(":", 1)[1].strip().lower() == "true"
    return characters, use_space_char


class CtcDecoder:
    """Greedy CTC decode identical in spirit to PP-OCR CTCLabelDecode.

    Index 0 is the CTC blank; dictionary characters start at 1; the space
    character (when enabled) is the final index. The model predicts Arabic
    in logical reading order, so no manual reversal is applied.
    """

    def __init__(self, characters, use_space_char=True):
        self.characters = ["blank"] + list(characters)
        if use_space_char:
            self.characters.append(" ")

    def decode(self, indexes, confidences):
        text_chars = []
        conf_values = []
        previous = -1
        for index, conf in zip(indexes, confidences):
            if index != previous and index != 0:
                text_chars.append(self.characters[index])
                conf_values.append(float(conf))
            previous = index
        return "".join(text_chars), conf_values


# ---------------------------------------------------------------------------
# Geometry helpers (detection post-processing, DB / unclip)
# ---------------------------------------------------------------------------

def polygon_area(points):
    total = 0.0
    count = len(points)
    for i in range(count):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % count]
        total += x0 * y1 - x1 * y0
    return abs(total) / 2.0


def polygon_perimeter(points):
    total = 0.0
    count = len(points)
    for i in range(count):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % count]
        total += math.hypot(x1 - x0, y1 - y0)
    return total


def unclip_polygon(points, unclip_ratio):
    """Offset a convex quad outward by area*ratio/perimeter (DB unclip).

    Each edge is moved outward along its normal; adjacent moved edges are
    intersected to rebuild the corner points. The normal direction adapts to
    the polygon winding so the offset always expands the shape.
    """
    area = polygon_area(points)
    distance = area * unclip_ratio / max(polygon_perimeter(points), 1e-6)
    # Signed area (shoelace): positive = counter-clockwise in math coords,
    # which is clockwise in image coords (y grows down).
    signed = 0.0
    count = len(points)
    for i in range(count):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % count]
        signed += x0 * y1 - x1 * y0
    sign = 1.0 if signed > 0 else -1.0

    moved = []
    for i in range(count):
        x0, y0 = points[i]
        x1, y1 = points[(i + 1) % count]
        edge = (x1 - x0, y1 - y0)
        length = math.hypot(*edge)
        if length < 1e-6:
            moved.append(None)
            continue
        normal = (sign * edge[1] / length, -sign * edge[0] / length)
        moved.append(((x0 + normal[0] * distance, y0 + normal[1] * distance),
                      (x1 + normal[0] * distance, y1 + normal[1] * distance)))
    result = []
    for i in range(count):
        prev = moved[(i - 1) % count]
        current = moved[i]
        if prev is None or current is None:
            result.append(points[i])
            continue
        intersection = _line_intersection(prev, current)
        result.append(intersection if intersection else points[i])
    return result


def _line_intersection(line_a, line_b):
    """Intersection of two infinite lines, each given by two points."""
    (ax0, ay0), (ax1, ay1) = line_a
    (bx0, by0), (bx1, by1) = line_b
    a_den = ax0 * ay1 - ay0 * ax1
    b_den = bx0 * by1 - by0 * bx1
    adx = ax0 - ax1
    ady = ay0 - ay1
    bdx = bx0 - bx1
    bdy = by0 - by1
    denominator = adx * bdy - ady * bdx
    if abs(denominator) < 1e-9:
        return None
    px = (a_den * bdx - adx * b_den) / denominator
    py = (a_den * bdy - ady * b_den) / denominator
    return (px, py)


def order_box_points(points):
    """Return [top_left, top_right, bottom_right, bottom_left]."""
    top_left = min(points, key=lambda p: p[0] + p[1])
    bottom_right = max(points, key=lambda p: p[0] + p[1])
    top_right = max(points, key=lambda p: p[0] - p[1])
    bottom_left = min(points, key=lambda p: p[0] - p[1])
    return [top_left, top_right, bottom_right, bottom_left]


# ---------------------------------------------------------------------------
# Model loading
# ---------------------------------------------------------------------------

def create_predictor(model_dir, cpu_threads):
    import paddle.inference as paddle_infer

    program = model_dir / "inference.json"
    if not program.is_file():
        program = model_dir / "inference.pdmodel"
    config = paddle_infer.Config(str(program), str(model_dir / "inference.pdiparams"))
    config.disable_gpu()
    # paddle 3.3 oneDNN + PIR new-executor combination crashes on these
    # models (ConvertPirAttribute2RuntimeAttribute Unimplemented); the
    # native CPU kernel path is the supported fallback on Windows CPU.
    config.disable_mkldnn()
    config.set_cpu_math_library_num_threads(max(1, int(cpu_threads)))
    return paddle_infer.create_predictor(config)


def run_predictor(predictor, input_array):
    input_handle = predictor.get_input_handle(predictor.get_input_names()[0])
    input_handle.reshape(list(input_array.shape))
    input_handle.copy_from_cpu(input_array)
    predictor.run()
    output_name = predictor.get_output_names()[0]
    output_handle = predictor.get_output_handle(output_name)
    return output_handle.copy_to_cpu()


# ---------------------------------------------------------------------------
# Detection
# ---------------------------------------------------------------------------

def preprocess_detection(image_bgr, long_side=DET_LONG_SIDE):
    height, width = image_bgr.shape[:2]
    scale = min(long_side / max(height, width), 1.0)
    resized_w = max(int(round(width * scale)), DET_PAD_MULTIPLE)
    resized_h = max(int(round(height * scale)), DET_PAD_MULTIPLE)
    resized = cv2.resize(image_bgr, (resized_w, resized_h), interpolation=cv2.INTER_LINEAR)
    padded_w = ((resized_w + DET_PAD_MULTIPLE - 1) // DET_PAD_MULTIPLE) * DET_PAD_MULTIPLE
    padded_h = ((resized_h + DET_PAD_MULTIPLE - 1) // DET_PAD_MULTIPLE) * DET_PAD_MULTIPLE
    padded = np.zeros((padded_h, padded_w, 3), dtype=resized.dtype)
    padded[:resized_h, :resized_w, :] = resized
    normalized = padded.astype(np.float32) / 255.0
    mean = np.array([0.485, 0.456, 0.406], dtype=np.float32)
    std = np.array([0.229, 0.224, 0.225], dtype=np.float32)
    normalized = (normalized - mean) / std
    tensor = normalized.transpose(2, 0, 1)[np.newaxis, ...].astype(np.float32)
    return tensor, (resized_w, resized_h), (width, height)


def postprocess_detection(prob_map, resized_size, original_size,
                          thresh=0.3, box_thresh=0.6, unclip_ratio=1.5,
                          max_candidates=1000):
    resized_w, resized_h = resized_size
    original_w, original_h = original_size
    bitmap = (prob_map[0] > thresh).astype(np.uint8)
    contours, _ = cv2.findContours(bitmap, cv2.RETR_LIST, cv2.CHAIN_APPROX_SIMPLE)
    boxes = []
    scale_x = original_w / resized_w
    scale_y = original_h / resized_h
    for contour in contours[:max_candidates]:
        if len(contour) < 4 or cv2.contourArea(contour) < 4.0:
            continue
        rect = cv2.minAreaRect(contour)
        points = cv2.boxPoints(rect)
        # Score: mean probability inside the candidate box.
        mask = np.zeros(bitmap.shape, dtype=np.uint8)
        cv2.fillPoly(mask, [np.round(points).astype(np.int32)], 1)
        score = prob_map[0][mask == 1].mean()
        if score < box_thresh:
            continue
        unclipped = unclip_polygon([tuple(float(v) for v in p) for p in points], unclip_ratio)
        # Map back to original image pixels and clip to bounds.
        mapped = []
        for x, y in unclipped:
            mapped.append((min(max(float(x) * scale_x, 0.0), float(original_w) - 1.0),
                           min(max(float(y) * scale_y, 0.0), float(original_h) - 1.0)))
        ordered = order_box_points(mapped)
        if polygon_area(ordered) < 12.0:
            continue
        boxes.append([(float(p[0]), float(p[1])) for p in ordered])
    boxes.sort(key=lambda box: (min(p[1] for p in box), min(p[0] for p in box)))
    return boxes


def crop_quad(image_bgr, box):
    """Perspective-crop a 4-point box into an upright BGR image."""
    top_left, top_right, bottom_right, bottom_left = box
    width_top = math.hypot(top_right[0] - top_left[0], top_right[1] - top_left[1])
    width_bottom = math.hypot(bottom_right[0] - bottom_left[0], bottom_right[1] - bottom_left[1])
    height_left = math.hypot(bottom_left[0] - top_left[0], bottom_left[1] - top_left[1])
    height_right = math.hypot(bottom_right[0] - top_right[0], bottom_right[1] - top_right[1])
    crop_w = max(int(round(max(width_top, width_bottom))), 2)
    crop_h = max(int(round(max(height_left, height_right))), 2)
    source = np.array([top_left, top_right, bottom_right, bottom_left], dtype=np.float32)
    target = np.array(
        [[0, 0], [crop_w - 1, 0], [crop_w - 1, crop_h - 1], [0, crop_h - 1]],
        dtype=np.float32,
    )
    transform = cv2.getPerspectiveTransform(source, target)
    return cv2.warpPerspective(image_bgr, transform, (crop_w, crop_h))


# ---------------------------------------------------------------------------
# Recognition
# ---------------------------------------------------------------------------

def preprocess_recognition_batch(crops):
    tensors = []
    for crop in crops:
        height, width = crop.shape[:2]
        if height < 2:
            crop = cv2.copyMakeBorder(crop, 0, 2, 0, 0, cv2.BORDER_CONSTANT, value=(0, 0, 0))
            height = crop.shape[0]
        target_w = min(max(int(round(width * RECOGNITION_HEIGHT / height)), 8), RECOGNITION_MAX_WIDTH)
        resized = cv2.resize(crop, (target_w, RECOGNITION_HEIGHT), interpolation=cv2.INTER_LINEAR)
        normalized = resized.astype(np.float32) / 255.0
        normalized = (normalized - 0.5) / 0.5
        tensors.append(normalized.transpose(2, 0, 1))
    batch_width = max(t.shape[2] for t in tensors)
    batch = np.zeros((len(tensors), 3, RECOGNITION_HEIGHT, batch_width), dtype=np.float32)
    for i, tensor in enumerate(tensors):
        batch[i, :, :, : tensor.shape[2]] = tensor
    return batch


def recognize_regions(predictor, decoder, image_bgr, boxes, batch_size, on_progress):
    crops = [crop_quad(image_bgr, box) for box in boxes]
    results = []
    total = len(crops)
    completed = 0
    for start in range(0, total, batch_size):
        chunk = crops[start : start + batch_size]
        batch = preprocess_recognition_batch(chunk)
        output = run_predictor(predictor, batch)
        # Output: [batch, time_steps, charset_size] softmax logits.
        for row in output:
            indexes = row.argmax(axis=-1)
            confidences = row.max(axis=-1)
            text, conf_values = decoder.decode(indexes, confidences)
            confidence = sum(conf_values) / len(conf_values) if conf_values else 0.0
            results.append((text, confidence))
        completed += len(chunk)
        on_progress(completed, total)
    return results


# ---------------------------------------------------------------------------
# Main OCR flow
# ---------------------------------------------------------------------------

def run_ocr(req):
    image_path = req["image_path"]
    det_dir = Path(req["det_model_path"])
    rec_dir = Path(req["rec_model_path"])
    page_index = req.get("page_index", 0)
    language = req.get("language", "auto")
    model_id = req.get("model_id", ENGINE_NAME)
    batch_size = max(1, int(req.get("batch_size", 8)))
    cpu_threads = max(1, int(req.get("cpu_threads", 4)))

    err = validate_request(req)
    if err:
        return make_error("MODEL_NOT_FOUND" if "model" in err.lower() else "INVALID_INPUT", err)

    emit_stage("model_loading")
    try:
        ensure_dependencies()
        with redirect_stdout(sys.stderr):
            import paddle  # noqa: F401  (runtime initialisation)
    except ImportError as error:
        return make_error("DEPENDENCY_MISSING", f"Local OCR runtime dependency missing: {error}")
    emit_stage("framework_imports")

    characters, use_space_char = parse_recognition_dict(rec_dir / "inference.yml")
    if not characters:
        return make_error("INVALID_MODEL", "Recognition dictionary in inference.yml is empty.")
    decoder = CtcDecoder(characters, use_space_char)

    try:
        with redirect_stdout(sys.stderr):
            det_predictor = create_predictor(det_dir, cpu_threads)
            rec_predictor = create_predictor(rec_dir, cpu_threads)
    except Exception as error:  # noqa: BLE001 - reported verbatim to the host
        return make_error("MODEL_INIT_FAILED", f"Model initialisation failed: {error}")

    # Warm up both predictors so page timings measure real work, not JIT.
    try:
        with redirect_stdout(sys.stderr):
            run_predictor(det_predictor, np.zeros((1, 3, DET_PAD_MULTIPLE, DET_PAD_MULTIPLE), dtype=np.float32))
            run_predictor(rec_predictor, np.zeros((1, 3, RECOGNITION_HEIGHT, RECOGNITION_MAX_WIDTH), dtype=np.float32))
    except Exception as error:  # noqa: BLE001
        return make_error("MODEL_INIT_FAILED", f"Model warm-up failed: {error}")
    emit_stage("model_ready")

    emit_stage("page_started", page_index=page_index)
    image_bgr = cv2.imread(str(image_path), cv2.IMREAD_COLOR)
    if image_bgr is None:
        return make_error("INPUT_NOT_FOUND", f"Image could not be decoded: {image_path}")
    emit_stage("input_image_decode")

    emit_stage("detection_started")
    det_input, resized_size, original_size = preprocess_detection(image_bgr)
    prob_map = run_predictor(det_predictor, det_input)
    # Det output may be [1,H,W] or [1,1,H,W]; normalise to [1,H,W].
    if prob_map.ndim == 4:
        prob_map = prob_map[0]
    emit_stage("detection_complete", threshold_pixels=int((prob_map[0] > 0.3).sum()))
    boxes = postprocess_detection(prob_map, resized_size, original_size)
    emit_stage("detection_boxes", count=len(boxes))

    if not boxes:
        return {
            "ok": True,
            "schema_version": SCHEMA_VERSION,
            "output_format_version": 1,
            "status": "no_text_detected",
            "worker_version": WORKER_VERSION,
            "engine": ENGINE_NAME,
            "model_version": str(model_id),
            "operation_id": req.get("operation_id", ""),
            "page_index": page_index,
            "text": "",
            "blocks": [],
            "confidence": None,
            "language": language,
            "language_source": "requested",
            "warnings": [],
            "timings": {"inference_ms": 0},
        }

    emit_stage("recognition_started", total_regions=len(boxes))

    def report_progress(completed, total):
        emit_stage("recognition_progress", completed_regions=completed, total_regions=total)

    original_h, original_w = image_bgr.shape[:2]
    recognized = recognize_regions(rec_predictor, decoder, image_bgr, boxes, batch_size, report_progress)

    blocks = []
    texts = []
    confidences = []
    for index, (box, (text, confidence)) in enumerate(zip(boxes, recognized)):
        cleaned = text.strip()
        if not cleaned:
            continue
        xs = [p[0] for p in box]
        ys = [p[1] for p in box]
        x0 = max(min(xs), 0.0)
        y0 = max(min(ys), 0.0)
        x1 = min(max(xs), float(original_w))
        y1 = min(max(ys), float(original_h))
        if x1 - x0 < 1.0 or y1 - y0 < 1.0:
            continue
        blocks.append({
            "id": f"ocr-{page_index}-{len(blocks)}",
            "text": cleaned,
            "bbox": {"x": round(x0, 2), "y": round(y0, 2), "width": round(x1 - x0, 2), "height": round(y1 - y0, 2)},
            "confidence": round(min(max(confidence, 0.0), 1.0), 4),
            "block_type": "text_line",
            "reading_order": index,
        })
        texts.append(cleaned)
        confidences.append(confidence)

    if not blocks:
        return {
            "ok": True,
            "schema_version": SCHEMA_VERSION,
            "output_format_version": 1,
            "status": "no_text_detected",
            "worker_version": WORKER_VERSION,
            "engine": ENGINE_NAME,
            "model_version": str(model_id),
            "operation_id": req.get("operation_id", ""),
            "page_index": page_index,
            "text": "",
            "blocks": [],
            "confidence": None,
            "language": language,
            "language_source": "requested",
            "warnings": [],
            "timings": {"inference_ms": int((time.perf_counter() - PROCESS_STARTED_AT) * 1000)},
        }

    emit_stage("page_complete", blocks=len(blocks))
    elapsed_ms = int((time.perf_counter() - PROCESS_STARTED_AT) * 1000)
    return {
        "ok": True,
        "schema_version": SCHEMA_VERSION,
        "output_format_version": 1,
        "status": "completed",
        "worker_version": WORKER_VERSION,
        "engine": ENGINE_NAME,
        "model_version": str(model_id),
        "operation_id": req.get("operation_id", ""),
        "page_index": page_index,
        "text": "\n".join(texts),
        "blocks": blocks,
        "confidence": round(sum(confidences) / len(confidences), 4),
        "language": language,
        "language_source": "requested",
        "warnings": [],
        "timings": {"inference_ms": elapsed_ms},
    }


def main():
    parser = argparse.ArgumentParser(description="PP-OCRv5 local OCR worker for R2H PDF")
    parser.add_argument("--input", type=str, help="Path to JSON request file (reads stdin if omitted)")
    parser.add_argument("--version", action="store_true", help="Print version and exit")
    args = parser.parse_args()

    if args.version:
        write_result_and_exit(
            {"version": WORKER_VERSION, "schema_version": SCHEMA_VERSION, "engine": ENGINE_NAME},
            0,
        )

    emit_stage("worker_started")
    try:
        if args.input:
            with open(args.input, "r", encoding="utf-8") as handle:
                req = json.load(handle)
        else:
            req = json.load(sys.stdin)
    except json.JSONDecodeError as error:
        write_result_and_exit(make_error("INVALID_JSON", f"Failed to parse input JSON: {error}"), 1)
    except FileNotFoundError:
        write_result_and_exit(make_error("INPUT_NOT_FOUND", f"Input file not found: {args.input}"), 1)

    emit_stage("worker_module_imports")
    result = run_ocr(req)
    write_result_and_exit(result, 0 if result.get("ok") else 1)


if __name__ == "__main__":
    main()
