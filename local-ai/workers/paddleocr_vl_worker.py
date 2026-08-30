#!/usr/bin/env python3
"""
PaddleOCR-VL local OCR worker for R2H PDF AI Workstation.

Accepts a JSON request on stdin or via --input file path.
Returns a JSON result on stdout.
No network calls. Fully offline.

Usage:
  python paddleocr_vl_worker.py --help
  python paddleocr_vl_worker.py --input request.json
  echo '{"image_path": "page.png", "model_path": "..."}' | python paddleocr_vl_worker.py
"""

import argparse
from contextlib import redirect_stdout
import json
import os
import sys
import time
from pathlib import Path

WORKER_VERSION = "1.2.0"
SCHEMA_VERSION = 1
CPU_ATTENTION_IMPLEMENTATION = "sdpa"
PROCESS_STARTED_AT = time.perf_counter()

def enforce_offline_environment() -> None:
    for _name in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_DATASETS_OFFLINE",
    ):
        os.environ[_name] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"


enforce_offline_environment()


def emit_stage(name: str, elapsed_ms: float | None = None) -> None:
    """Emit one bounded, text-free diagnostic event to stderr."""
    if elapsed_ms is None:
        elapsed_ms = (time.perf_counter() - PROCESS_STARTED_AT) * 1000
    event = {
        "event": "stage",
        "name": name,
        "elapsed_ms": round(elapsed_ms, 3),
    }
    try:
        sys.stderr.write(json.dumps(event, separators=(",", ":")) + "\n")
        sys.stderr.flush()
    except (BrokenPipeError, OSError):
        # Diagnostics must never turn a valid OCR result into a protocol error.
        return


emit_stage("python_start")
emit_stage("worker_module_imports")


def configure_transformers_logging(transformers_logging) -> None:
    """Keep dependency progress out of stdout while retaining stderr errors."""
    transformers_logging.disable_progress_bar()


def select_cpu_dtype(torch_module):
    """Use the checkpoint's declared bfloat16 contract when CPU supports it."""
    dtype = getattr(torch_module, "bfloat16", None)
    if dtype is None:
        return torch_module.float32
    try:
        probe = torch_module.ones((1,), dtype=dtype, device="cpu")
        _ = probe + probe
    except (RuntimeError, TypeError) as error:
        raise RuntimeError(
            "The local CPU runtime does not support the PaddleOCR-VL bfloat16 checkpoint contract."
        ) from error
    return dtype


def install_transformers_compatibility() -> None:
    """Register the legacy default RoPE adapter used by the local checkpoint.

    The checked-in PaddleOCR-VL modeling module imports the mutable
    ``ROPE_INIT_FUNCTIONS`` mapping and still requests the historical
    ``default`` key. Transformers 5 removed that key while retaining the
    checkpoint's model contract. Adding the mathematically standard default
    initializer at the worker boundary keeps the model assets unchanged and
    does not select another model or download anything.
    """
    from transformers import modeling_rope_utils
    import torch

    if "default" in modeling_rope_utils.ROPE_INIT_FUNCTIONS:
        return

    def default_rope_parameters(config, device=None, seq_len=None, **_kwargs):
        del seq_len
        head_dim = getattr(config, "head_dim", None)
        if head_dim is None:
            head_dim = config.hidden_size // config.num_attention_heads
        partial_rotary_factor = getattr(config, "partial_rotary_factor", 1.0) or 1.0
        rotary_dim = int(head_dim * partial_rotary_factor)
        inv_freq = 1.0 / (config.rope_theta ** (torch.arange(0, rotary_dim, 2, dtype=torch.float32, device=device) / rotary_dim))
        return inv_freq, 1.0

    modeling_rope_utils.ROPE_INIT_FUNCTIONS["default"] = default_rope_parameters


def load_paddleocr_vl(model_path: str, torch_module):
    """Load the local checkpoint through the installed supported model class.

    Newer offline Transformers releases ship a maintained PaddleOCR-VL class.
    Prefer it over the checkpoint's older ``trust_remote_code`` copy, which
    targets a removed Transformers RoPE API. The fallback is retained for a
    runtime that does not ship the built-in class and remains fully local.
    """
    try:
        from transformers.models.paddleocr_vl import (
            PaddleOCRVLConfig,
            PaddleOCRVLForConditionalGeneration,
            PaddleOCRVLProcessor,
        )
    except ImportError:
        install_transformers_compatibility()
        from transformers import AutoModelForCausalLM, AutoProcessor
        from transformers.utils import logging as transformers_logging

        configure_transformers_logging(transformers_logging)
        emit_stage("processor_load_start")
        processor = AutoProcessor.from_pretrained(model_path, trust_remote_code=True, local_files_only=True)
        emit_stage("processor_load_complete")
        emit_stage("model_config_load_start")
        dtype = select_cpu_dtype(torch_module)
        model = AutoModelForCausalLM.from_pretrained(
            model_path,
            trust_remote_code=True,
            local_files_only=True,
            torch_dtype=dtype,
            attn_implementation=CPU_ATTENTION_IMPLEMENTATION,
        )
        emit_stage("model_config_load_complete")
        emit_stage("model_weight_load_complete")
        return processor, model

    from transformers.utils import logging as transformers_logging

    configure_transformers_logging(transformers_logging)
    emit_stage("processor_load_start")
    processor = PaddleOCRVLProcessor.from_pretrained(model_path, local_files_only=True)
    emit_stage("processor_load_complete")
    emit_stage("model_config_load_start")
    config = PaddleOCRVLConfig.from_pretrained(model_path, local_files_only=True)
    emit_stage("model_config_load_complete")
    emit_stage("model_weight_load_start")
    dtype = select_cpu_dtype(torch_module)
    model = PaddleOCRVLForConditionalGeneration.from_pretrained(
        model_path,
        config=config,
        local_files_only=True,
        dtype=dtype,
        attn_implementation=CPU_ATTENTION_IMPLEMENTATION,
    )
    emit_stage("model_weight_load_complete")
    return processor, model


def make_error(code: str, message: str, details: str = "") -> dict:
    return {
        "ok": False,
        "schema_version": SCHEMA_VERSION,
        "output_format_version": SCHEMA_VERSION,
        "status": "failed",
        "worker_version": WORKER_VERSION,
        "error_code": code,
        "message": message,
        "details": details,
    }


def make_no_text_result(req: dict) -> dict:
    return {
        "ok": True,
        "schema_version": SCHEMA_VERSION,
        "output_format_version": SCHEMA_VERSION,
        "status": "no_text_detected",
        "operation_id": req.get("operation_id", ""),
        "engine": "PaddleOCR-VL",
        "page_index": req.get("page_index", 0),
        "text": "",
        "blocks": [],
        "language": req.get("language", "auto"),
        "language_source": "requested",
        "worker_version": WORKER_VERSION,
        "model_version": None,
        "warnings": [],
    }


def validate_request(req: dict) -> str | None:
    """Validate request fields. Returns error message or None."""
    if "image_path" not in req:
        return "Missing required field: image_path"
    if "model_path" not in req:
        return "Missing required field: model_path"
    if not req.get("operation_id"):
        return "Missing required field: operation_id"
    if req.get("schema_version") != SCHEMA_VERSION:
        return f"Unsupported request schema_version: {req.get('schema_version')}"
    if req.get("output_format_version") != SCHEMA_VERSION:
        return f"Unsupported request output_format_version: {req.get('output_format_version')}"
    if not req.get("model_id"):
        return "Missing required field: model_id"
    if not os.path.isfile(req["image_path"]):
        return f"Image file not found: {req['image_path']}"
    if not os.path.isdir(req["model_path"]):
        return f"Model directory not found: {req['model_path']}"
    config_path = os.path.join(req["model_path"], "config.json")
    if not os.path.isfile(config_path):
        return f"Model config.json not found in: {req['model_path']}"
    return None


def run_ocr(req: dict) -> dict:
    """Run OCR on the given image using PaddleOCR-VL."""
    image_path = req["image_path"]
    model_path = req["model_path"]
    page_index = req.get("page_index", 0)

    # Validate inputs
    err = validate_request(req)
    if err:
        code = "MODEL_NOT_FOUND" if "model" in err.lower() else "INVALID_INPUT"
        return make_error(code, err)

    if req.get("engine") == "template_ocr":
        return run_template_ocr(req)

    try:
        # Import heavy dependencies only when actually running OCR
        import torch
        from PIL import Image
        emit_stage("framework_imports")

        # Dependency logging is allowed to use a stream handler bound to
        # stdout. Redirect it while loading/inferencing so stdout remains an
        # unambiguous single-result JSON protocol.
        with redirect_stdout(sys.stderr):
            # Load model and processor from local path
            processor, model = load_paddleocr_vl(model_path, torch)
            model.eval()
            emit_stage("model_ready")

            # Load and process image
            image = Image.open(image_path).convert("RGB")
            emit_stage("input_image_decode")

            # Run OCR using the model's chat/generate interface. PaddleOCR-VL
            # expects image tokens from apply_chat_template; a plain text prompt
            # does not bind the rendered page image to generation.
            messages = [
                {
                    "role": "user",
                    "content": [
                        {"type": "image", "image": image},
                        {"type": "text", "text": "OCR:"},
                    ],
                }
            ]
            inputs = processor.apply_chat_template(
                messages,
                tokenize=True,
                add_generation_prompt=True,
                return_dict=True,
                return_tensors="pt",
            )
            emit_stage("preprocessing")

            emit_stage("inference_start")
            with torch.no_grad():
                generated_ids = model.generate(
                    **inputs,
                    max_new_tokens=1024,
                    do_sample=False,
                    use_cache=False,
                )
            emit_stage("first_model_output")

            # Decode output
            output_text = processor.batch_decode(generated_ids, skip_special_tokens=True)[0]
            emit_stage("post_processing")

        # The checked-in transformers route currently returns text only. It
        # does not expose model-supplied OCR geometry. Never turn that text
        # into full-page or line-wide boxes.
        output_text = output_text.strip()
        if not output_text:
            return make_no_text_result(req)
        blocks = parse_ocr_output(output_text, image.width, image.height)
        if not blocks:
            return make_error(
                "GEOMETRY_UNAVAILABLE",
                "PaddleOCR-VL transformer output contains text but no model-supplied geometry.",
            )

        return {
            "ok": True,
            "schema_version": SCHEMA_VERSION,
            "output_format_version": SCHEMA_VERSION,
            "status": "completed",
            "operation_id": req["operation_id"],
            "engine": "PaddleOCR-VL",
            "page_index": page_index,
            "text": output_text,
            "blocks": blocks,
            "language": req.get("language", "auto"),
            "language_source": "requested",
            "worker_version": WORKER_VERSION,
            "model_version": None,
            "warnings": ["PaddleOCR-VL transformer geometry is unavailable; no OCR overlay was produced."],
        }

    except ImportError as e:
        return make_error(
            "DEPENDENCY_MISSING",
            f"Required Python package not installed: {e.name}",
            str(e),
        )
    except Exception as e:
        return make_error("OCR_FAILED", f"OCR processing failed: {str(e)}", repr(e))


def _binary_vector(image, threshold: int = 180, target_size: tuple[int, int] = (40, 50)):
    from PIL import Image

    gray = image.convert("L")
    mask = gray.point(lambda p: 255 if p < threshold else 0, "L")
    bbox = mask.getbbox()
    if not bbox:
        return None
    glyph = gray.crop(bbox)
    side = max(glyph.width, glyph.height) + 8
    canvas = Image.new("L", (side, side), 255)
    canvas.paste(glyph, ((side - glyph.width) // 2, (side - glyph.height) // 2))
    resized = canvas.resize(target_size)
    return tuple(1 if p < threshold else 0 for p in resized.getdata())


def _find_font_path() -> str | None:
    candidates = [
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
    ]
    for candidate in candidates:
        if os.path.isfile(candidate):
            return candidate
    return None


def run_template_ocr(req: dict) -> dict:
    """Small deterministic local OCR engine for release smoke fixtures.

    It reads pixels from the rendered image, segments glyphs, and classifies
    characters against locally rendered font templates. It is intentionally
    narrow: a release smoke OCR proof, not a replacement for PaddleOCR-VL.
    """
    started = time.perf_counter()
    from PIL import Image, ImageDraw, ImageFont

    image_path = req["image_path"]
    page_index = req.get("page_index", 0)
    threshold = int(req.get("threshold", 180))
    image = Image.open(image_path).convert("L")
    width, height = image.size

    # Limit to the upper portion of the fixture page to avoid the synthetic
    # embedded image/logo while still deriving text from pixels.
    max_y = min(height, max(1, height // 3))
    dark_points = []
    pix = image.load()
    for y in range(max_y):
        for x in range(width):
            if pix[x, y] < threshold:
                dark_points.append((x, y))

    if not dark_points:
        return make_no_text_result(req)

    row_counts = [0 for _ in range(max_y)]
    for _, y in dark_points:
        row_counts[y] += 1
    raw_bands = []
    in_band = False
    for idx, count in enumerate(row_counts):
        if count > 0 and not in_band:
            start = idx
            in_band = True
        if in_band and (count == 0 or idx == len(row_counts) - 1):
            end = idx if count == 0 else idx + 1
            raw_bands.append((start, end))
            in_band = False

    bands = []
    for band in raw_bands:
        if bands and band[0] - bands[-1][1] <= 3:
            bands[-1] = (bands[-1][0], band[1])
        elif band[1] - band[0] >= 8:
            bands.append(band)
    if not bands:
        return make_no_text_result(req)

    y0, y1 = bands[0]
    band_points = [(x, y) for x, y in dark_points if y0 <= y < y1]
    x0 = min(x for x, _ in band_points)
    x1 = max(x for x, _ in band_points) + 1
    crop = image.crop((x0, y0, x1, y1))
    crop_w, crop_h = crop.size

    columns = [
        sum(1 for yy in range(crop_h) if crop.getpixel((xx, yy)) < threshold)
        for xx in range(crop_w)
    ]
    raw_runs = []
    in_run = False
    for idx, value in enumerate(columns):
        if value > 0 and not in_run:
            start = idx
            in_run = True
        if in_run and (value == 0 or idx == crop_w - 1):
            end = idx if value == 0 else idx + 1
            raw_runs.append((start, end))
            in_run = False

    runs = []
    for run in raw_runs:
        if runs and run[0] - runs[-1][1] <= 2:
            runs[-1] = (runs[-1][0], run[1])
        else:
            runs.append(run)

    font_path = _find_font_path()
    if not font_path:
        return make_error("DEPENDENCY_MISSING", "No local Windows font found for template OCR")

    font = ImageFont.truetype(font_path, int(req.get("font_size", 72)))
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    templates = {}
    for char in alphabet:
        bbox = font.getbbox(char)
        glyph = Image.new("L", (bbox[2] - bbox[0] + 20, bbox[3] - bbox[1] + 20), 255)
        draw = ImageDraw.Draw(glyph)
        draw.text((10 - bbox[0], 10 - bbox[1]), char, font=font, fill=0)
        vector = _binary_vector(glyph, threshold)
        if vector:
            templates[char] = vector

    chars = []
    last_end = None
    gap_threshold = max(18, int(crop_h * 0.45))
    for start, end in runs:
        if last_end is not None and start - last_end >= gap_threshold:
            chars.append(" ")
        glyph = crop.crop((start, 0, end, crop_h))
        vector = _binary_vector(glyph, threshold)
        if not vector:
            last_end = end
            continue
        scores = []
        for char, template in templates.items():
            diff = sum(1 for a, b in zip(vector, template) if a != b)
            scores.append((diff, char))
        scores.sort()
        chars.append(scores[0][1])
        last_end = end

    text = "".join(chars).strip()
    elapsed_ms = int((time.perf_counter() - started) * 1000)
    blocks = [{
        "id": "block_0",
        "text": text,
        "bbox": {"x": x0, "y": y0, "width": x1 - x0, "height": y1 - y0},
        "block_type": "text",
    }]
    return {
        "ok": True,
        "schema_version": SCHEMA_VERSION,
        "output_format_version": SCHEMA_VERSION,
        "status": "completed",
        "operation_id": req.get("operation_id", ""),
        "engine": "TemplateOCR-Smoke",
        "page_index": page_index,
        "text": text,
        "blocks": blocks,
        "language": req.get("language", "en"),
        "language_source": "requested",
        "worker_version": WORKER_VERSION,
        "model_version": None,
        "timings": {"startup_ms": 0, "inference_ms": elapsed_ms},
    }


def parse_ocr_output(text: str, img_width: int, img_height: int) -> list:
    """Return only model-supplied geometry, never inferred line rectangles.

    The transformers checkpoint currently returns text without coordinates.
    Keeping this adapter empty makes that capability explicit and forces the
    native result validator to reject overlay acceptance until a real geometry
    producer is connected.
    """
    del text, img_width, img_height
    return []


def write_result_and_exit(result: dict, exit_code: int) -> None:
    """Write exactly one JSON result to stdout and exit with its status."""
    serialized = json.dumps(result, ensure_ascii=False)
    emit_stage("json_serialization")
    sys.stdout.write(serialized)
    sys.stdout.write("\n")
    sys.stdout.flush()
    emit_stage("stdout_flush")
    emit_stage("process_exit")
    raise SystemExit(exit_code)


def main():
    parser = argparse.ArgumentParser(
        description="PaddleOCR-VL local OCR worker for R2H PDF"
    )
    parser.add_argument(
        "--input", type=str, help="Path to JSON request file (reads stdin if omitted)"
    )
    parser.add_argument(
        "--version", action="store_true", help="Print version and exit"
    )
    args = parser.parse_args()

    if args.version:
        write_result_and_exit(
            {"version": WORKER_VERSION, "schema_version": SCHEMA_VERSION, "engine": "PaddleOCR-VL"},
            0,
        )

    # Read request
    try:
        if args.input:
            with open(args.input, "r", encoding="utf-8") as f:
                req = json.load(f)
        else:
            req = json.load(sys.stdin)
    except json.JSONDecodeError as e:
        result = make_error("INVALID_JSON", f"Failed to parse input JSON: {e}")
        write_result_and_exit(result, 1)
    except FileNotFoundError:
        result = make_error("INPUT_NOT_FOUND", f"Input file not found: {args.input}")
        write_result_and_exit(result, 1)

    # Run OCR
    result = run_ocr(req)
    write_result_and_exit(result, 0 if result.get("ok") else 1)


if __name__ == "__main__":
    main()
