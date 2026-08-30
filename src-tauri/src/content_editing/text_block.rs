//! Phase 28D — block-level text editing.
//!
//! A "block" is a cluster of nearby text spans on a page that share a
//! similar font size and sit on consecutive lines. The block editor
//! supports two strategies:
//!
//! 1. `NativeMultiOperator` — replace each native operator with the
//!    corresponding line of the new block text, where every member span
//!    is native-edit-safe. The new text is split into N lines (one per
//!    original span); if line counts mismatch, we degrade to visual
//!    reflow rather than guessing.
//!
//! 2. `VisualReflow` — redact the entire block bbox and re-draw the
//!    replacement with Helvetica, wrapping to the original bbox width.
//!    The wrap is honest: we tell the user we wrapped at bbox width and
//!    do not claim Acrobat-level paragraph reflow.

use std::time::{SystemTime, UNIX_EPOCH};

use mupdf::Buffer;

use super::analysis::extract_page_content_objects;
use super::stream_parser::{find_text_operations, replace_text_in_stream_encoded, text_matches, EncodingTarget};
use super::text_edit::classify_font_replacement_safety;
use super::types::*;
use crate::document_core::DocumentCoreState;

/// Maximum vertical gap (in points) between two spans that we'll still
/// merge into the same block at the same font size.
const BLOCK_VERTICAL_TOLERANCE_PTS: f32 = 4.0;
/// Maximum font-size delta between two spans in the same block.
const BLOCK_FONT_SIZE_TOLERANCE_PTS: f32 = 1.0;

/// Build text blocks from the page's content objects. Pure function so
/// it's straightforward to unit-test against synthetic inputs.
pub fn group_text_blocks(objects: &[ContentObject]) -> Vec<TextBlock> {
    let mut spans: Vec<&ContentObject> = objects
        .iter()
        .filter(|o| matches!(o.object_type, ContentObjectType::TextSpan))
        .collect();
    // Sort by y descending (top → bottom), then x ascending.
    spans.sort_by(|a, b| {
        b.bbox[3].partial_cmp(&a.bbox[3])
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.bbox[0].partial_cmp(&b.bbox[0]).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut blocks: Vec<Vec<&ContentObject>> = Vec::new();
    for span in spans {
        let pushed = blocks.last_mut().and_then(|cluster| {
            let last = cluster.last()?;
            let last_size = last.text_info.as_ref().map(|t| t.font_size).unwrap_or(0.0);
            let this_size = span.text_info.as_ref().map(|t| t.font_size).unwrap_or(0.0);
            if (last_size - this_size).abs() > BLOCK_FONT_SIZE_TOLERANCE_PTS {
                return None;
            }
            // y0 of `span` should be close to y0 of `last - line_height`.
            // We use y-distance between rows.
            let vgap = (last.bbox[1] - span.bbox[3]).abs();
            // Accept when vgap is within tolerance OR rows overlap.
            let overlap_y = span.bbox[3] > last.bbox[1];
            if vgap <= BLOCK_VERTICAL_TOLERANCE_PTS + last_size || overlap_y {
                cluster.push(span);
                Some(())
            } else {
                None
            }
        });
        if pushed.is_none() {
            blocks.push(vec![span]);
        }
    }

    let mut out = Vec::new();
    for (i, cluster) in blocks.iter().enumerate() {
        if cluster.is_empty() {
            continue;
        }
        let page_index = cluster[0].page_index;
        let session_id = cluster[0].session_id.clone();
        // Compute union bbox.
        let mut x0 = f32::INFINITY;
        let mut y0 = f32::INFINITY;
        let mut x1 = f32::NEG_INFINITY;
        let mut y1 = f32::NEG_INFINITY;
        for s in cluster {
            x0 = x0.min(s.bbox[0]);
            y0 = y0.min(s.bbox[1]);
            x1 = x1.max(s.bbox[2]);
            y1 = y1.max(s.bbox[3]);
        }
        let lines: Vec<String> = cluster
            .iter()
            .map(|s| {
                s.text_info
                    .as_ref()
                    .map(|t| t.decoded_text.clone())
                    .unwrap_or_default()
            })
            .collect();
        let combined_text = lines.join("\n");
        let median_size = {
            let mut sizes: Vec<f32> = cluster
                .iter()
                .map(|s| s.text_info.as_ref().map(|t| t.font_size).unwrap_or(0.0))
                .collect();
            sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            if sizes.is_empty() {
                0.0
            } else {
                sizes[sizes.len() / 2]
            }
        };
        let all_native_editable = cluster
            .iter()
            .all(|s| s.editable_level == EditableLevel::NativeEditable);
        let mut diagnostics = vec![format!("{} member span(s) grouped.", cluster.len())];
        if !all_native_editable {
            diagnostics.push(
                "At least one member span is not native-editable — block edit will use safe visual reflow.".to_string(),
            );
        }
        out.push(TextBlock {
            block_id: format!("blk-{page_index}-{i}"),
            session_id,
            page_index,
            bbox: [x0, y0, x1, y1],
            member_ids: cluster.iter().map(|s| s.id.clone()).collect(),
            combined_text,
            font_size: median_size,
            all_native_editable,
            diagnostics,
        });
    }
    out
}

/// Apply a block-level text edit. Picks the strategy automatically when
/// `Auto` is requested.
pub fn apply_text_block_edit(
    doc_state: &DocumentCoreState,
    request: &TextBlockEditRequest,
) -> Result<TextBlockEditResult, String> {
    let edit_id = format!("blk-edit-{}", epoch_ms());
    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let blocks = group_text_blocks(&objects);
    let block = blocks
        .iter()
        .find(|b| b.block_id == request.block_id)
        .ok_or_else(|| format!("Block {} not found on page {}", request.block_id, request.page_index + 1))?;

    let before_text = block.combined_text.clone();

    let strategy = match &request.strategy {
        BlockEditStrategy::Auto => {
            if block.all_native_editable && lines_match_count(&before_text, &request.replacement_text) {
                BlockEditStrategy::NativeMultiOperator
            } else {
                BlockEditStrategy::VisualReflow
            }
        }
        BlockEditStrategy::NativeMultiOperator => BlockEditStrategy::NativeMultiOperator,
        BlockEditStrategy::VisualReflow => BlockEditStrategy::VisualReflow,
    };

    match strategy {
        BlockEditStrategy::NativeMultiOperator => {
            apply_native_multi_operator(doc_state, request, block, &edit_id, &before_text, &objects)
        }
        BlockEditStrategy::VisualReflow => {
            apply_visual_reflow(doc_state, request, block, &edit_id, &before_text)
        }
        BlockEditStrategy::Auto => unreachable!(),
    }
}

fn lines_match_count(before: &str, after: &str) -> bool {
    before.lines().count() == after.lines().count()
}

fn apply_native_multi_operator(
    doc_state: &DocumentCoreState,
    request: &TextBlockEditRequest,
    block: &TextBlock,
    edit_id: &str,
    before_text: &str,
    objects: &[ContentObject],
) -> Result<TextBlockEditResult, String> {
    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let session_bytes = {
        let s = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
        s.document.bytes.clone()
    };

    let pdf = mupdf::pdf::PdfDocument::from_bytes(&session_bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;
    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no).map_err(|e| format!("load_page: {e}"))?;
    let pdf_page = mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage: {e}"))?;
    let page_obj = pdf_page.object();
    let contents = page_obj.get_dict("Contents").ok().flatten()
        .ok_or_else(|| "Page has no /Contents".to_string())?;

    // Only support single-stream pages for multi-operator native edit
    // in this phase; the algorithm itself is identical for arrays but
    // the bookkeeping is fragile across multiple streams.
    if !contents.is_stream().unwrap_or(false) {
        return Ok(TextBlockEditResult {
            edit_id: edit_id.to_string(),
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            block_id: request.block_id.clone(),
            method: EditMethod::Rejected,
            edited_object_ids: vec![],
            before_text: before_text.to_string(),
            after_text: request.replacement_text.clone(),
            success: false,
            warnings: vec![
                "Multi-operator native block edit currently only supports single-stream pages. Use visual reflow instead.".to_string(),
            ],
        });
    }

    let mut stream_bytes = contents.read_stream()
        .map_err(|e| format!("read_stream: {e}"))?;

    let new_lines: Vec<&str> = request.replacement_text.lines().collect();
    let old_lines: Vec<&str> = before_text.lines().collect();
    if new_lines.len() != old_lines.len() {
        return Ok(TextBlockEditResult {
            edit_id: edit_id.to_string(),
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            block_id: request.block_id.clone(),
            method: EditMethod::Rejected,
            edited_object_ids: vec![],
            before_text: before_text.to_string(),
            after_text: request.replacement_text.clone(),
            success: false,
            warnings: vec![
                "Native multi-operator edit requires the new text to keep the same number of lines as the original. Use visual reflow for length changes.".to_string(),
            ],
        });
    }

    // For each member span, find its operator in the (potentially
    // already mutated) stream by text match + occurrence index relative
    // to the current stream contents. We re-parse on every iteration
    // so byte offsets stay valid as we mutate.
    let mut edited_ids: Vec<String> = Vec::new();
    for (member_id, new_line) in block.member_ids.iter().zip(new_lines.iter()) {
        let target_obj = objects.iter().find(|o| &o.id == member_id);
        let original_line = target_obj
            .and_then(|o| o.text_info.as_ref())
            .map(|t| t.decoded_text.clone())
            .unwrap_or_default();
        let occurrence = target_obj
            .and_then(|o| o.text_info.as_ref())
            .map(|t| t.occurrence_index)
            .unwrap_or(0);

        // Phase 28C — refuse if this individual line is unsafe to encode.
        let font_name = target_obj
            .and_then(|o| o.text_info.as_ref())
            .map(|t| t.font_name.clone())
            .unwrap_or_else(|| "Unknown".to_string());
        let dec = classify_font_replacement_safety(&font_name, None, new_line);
        if !dec.native_safe {
            return Ok(TextBlockEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                block_id: request.block_id.clone(),
                method: EditMethod::Rejected,
                edited_object_ids: edited_ids,
                before_text: before_text.to_string(),
                after_text: request.replacement_text.clone(),
                success: false,
                warnings: dec.reasons,
            });
        }

        let ops = find_text_operations(&stream_bytes);
        let matches: Vec<&super::stream_parser::TextOperation> = ops.iter()
            .filter(|op| text_matches(op, &original_line))
            .collect();
        let chosen = if matches.is_empty() {
            return Ok(TextBlockEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                block_id: request.block_id.clone(),
                method: EditMethod::Rejected,
                edited_object_ids: edited_ids,
                before_text: before_text.to_string(),
                after_text: request.replacement_text.clone(),
                success: false,
                warnings: vec![format!(
                    "Could not find operator for member span text \"{original_line}\" in stream."
                )],
            });
        } else if matches.len() == 1 {
            matches[0]
        } else if occurrence < matches.len() {
            matches[occurrence]
        } else {
            matches[0]
        };

        // Phase 29D — pick the encoding target from the member span's
        // detected font_encoding (set by the analysis layer). Default to
        // WinAnsi which covers ASCII + Latin-1 octal escapes.
        let target = match target_obj
            .and_then(|o| o.text_info.as_ref())
            .and_then(|t| t.font_encoding.clone())
            .as_deref()
        {
            Some(e) if e.eq_ignore_ascii_case("MacRomanEncoding") => EncodingTarget::MacRoman,
            Some(_) | None => EncodingTarget::WinAnsi,
        };
        let next = replace_text_in_stream_encoded(&stream_bytes, chosen, new_line, target);
        if next == stream_bytes {
            return Ok(TextBlockEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                block_id: request.block_id.clone(),
                method: EditMethod::Rejected,
                edited_object_ids: edited_ids,
                before_text: before_text.to_string(),
                after_text: request.replacement_text.clone(),
                success: false,
                warnings: vec!["Multi-op edit produced no byte change; aborting.".to_string()],
            });
        }
        stream_bytes = next;
        edited_ids.push(member_id.clone());
    }

    let buf = Buffer::from_bytes(&stream_bytes).map_err(|e| format!("Buffer: {e}"))?;
    let mut contents_mut = contents;
    contents_mut.write_stream_buffer(&buf)
        .map_err(|e| format!("write_stream_buffer: {e}"))?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;
    if new_bytes == session_bytes {
        return Ok(TextBlockEditResult {
            edit_id: edit_id.to_string(),
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            block_id: request.block_id.clone(),
            method: EditMethod::Rejected,
            edited_object_ids: vec![],
            before_text: before_text.to_string(),
            after_text: request.replacement_text.clone(),
            success: false,
            warnings: vec!["Native block edit serialized identical bytes; refusing to claim success.".to_string()],
        });
    }
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(TextBlockEditResult {
        edit_id: edit_id.to_string(),
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        block_id: request.block_id.clone(),
        method: EditMethod::NativeMultiOperator,
        edited_object_ids: edited_ids,
        before_text: before_text.to_string(),
        after_text: request.replacement_text.clone(),
        success: true,
        warnings: vec![
            "Native multi-operator block edit applied. Original font and per-line position preserved."
                .to_string(),
        ],
    })
}

fn apply_visual_reflow(
    doc_state: &DocumentCoreState,
    request: &TextBlockEditRequest,
    block: &TextBlock,
    edit_id: &str,
    before_text: &str,
) -> Result<TextBlockEditResult, String> {
    // Phase 32A/35A — reject early if the block bbox cannot be safely covered.
    let font_size = block.font_size.max(8.0);
    let cover_rect = match super::text_edit::safe_visual_cover_rect(block.bbox, font_size) {
        Some(r) => r,
        None => {
            return Ok(TextBlockEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                block_id: request.block_id.clone(),
                method: EditMethod::Rejected,
                edited_object_ids: vec![],
                before_text: before_text.to_string(),
                after_text: request.replacement_text.clone(),
                success: false,
                warnings: vec![
                    "Cannot apply safe visual reflow: block bbox is degenerate or non-finite, so a cover rectangle cannot be placed safely. Edit refused to avoid leaving the original text visible under the replacement.".to_string(),
                ],
            });
        }
    };
    let mut layout = match compute_visual_reflow_layout(
        &request.replacement_text,
        block.bbox,
        font_size,
        &request.overflow_policy,
    ) {
        Ok(layout) => layout,
        Err(warning) => {
            return Ok(TextBlockEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                block_id: request.block_id.clone(),
                method: EditMethod::Rejected,
                edited_object_ids: block.member_ids.clone(),
                before_text: before_text.to_string(),
                after_text: request.replacement_text.clone(),
                success: false,
                warnings: vec![warning],
            });
        }
    };

    let arc = doc_state.store.get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc.lock().map_err(|_| "session lock poisoned".to_string())?;
    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF for editing: {e}"))?;
    let page_no = i32::try_from(request.page_index)
        .map_err(|e| format!("page index: {e}"))?;
    let fz_page = pdf.load_page(page_no).map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page = mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage: {e}"))?;
    let bg_sample = super::background_sampling::sample_page_background(
        doc_state,
        &request.session_id,
        request.page_index,
        block.bbox,
    );
    layout.cover_rgb = bg_sample.rgb;
    if let Some(w) = bg_sample.warning {
        layout.warnings.push(w);
    } else {
        layout.warnings.push(format!(
            "Sampled background cover color rgb({:.3}, {:.3}, {:.3}) for visual paragraph reflow.",
            layout.cover_rgb[0], layout.cover_rgb[1], layout.cover_rgb[2]
        ));
    }

    // 1) Mark the padded block bbox for redaction (defense in depth — the
    //    explicit cover rectangle drawn below is the load-bearing mechanism).
    let rect = mupdf::Rect::new(cover_rect[0], cover_rect[1], cover_rect[2], cover_rect[3]);
    let mut annot = pdf_page.create_annotation(mupdf::pdf::PdfAnnotationType::Redact)
        .map_err(|e| format!("create redact: {e}"))?;
    annot.set_rect(rect).map_err(|e| format!("set_rect: {e}"))?;
    annot.set_color(mupdf::color::AnnotationColor::Rgb {
        red: layout.cover_rgb[0],
        green: layout.cover_rgb[1],
        blue: layout.cover_rgb[2],
    })
        .map_err(|e| format!("set_color: {e}"))?;
    drop(annot);
    pdf_page.redact().map_err(|e| format!("redact: {e}"))?;

    // 2) Ensure Helvetica resource.
    let page_obj = pdf_page.object();
    ensure_helvetica_resource(&pdf, &page_obj)?;

    // 3) Draw wrapped text inside the block bbox.
    let top_y = block.bbox[3] - layout.font_size * 0.85;
    let [cx0, cy0, cx1, cy1] = cover_rect;
    let cw = cx1 - cx0;
    let ch = cy1 - cy0;
    // Explicit background-fill cover rectangle ALWAYS comes first so the
    // original glyphs are completely hidden before the replacement text is
    // drawn on top. Phase 35C currently uses conservative white fallback
    // when no reliable rendered-pixel sampler is available in this path.
    let mut text_block = String::new();
    text_block.push_str(&format!(
        "\nq\n{} {} {} rg\n{cx0} {cy0} {cw} {ch} re\nf\nQ\n",
        fmt_color(layout.cover_rgb[0]), fmt_color(layout.cover_rgb[1]), fmt_color(layout.cover_rgb[2]),
    ));
    text_block.push_str("q\nBT\n");
    text_block.push_str(&format!("/R2HHelv {} Tf\n", layout.font_size));
    for (i, line) in layout.lines.iter().enumerate() {
        let y = top_y - layout.line_height * i as f32;
        if y < block.bbox[1] && request.overflow_policy != BlockOverflowPolicy::AllowOverflow {
            break;
        }
        text_block.push_str(&format!("1 0 0 1 {x} {y} Tm\n", x = block.bbox[0]));
        text_block.push_str(&format!("({}) Tj\n", pdf_escape_string(line)));
    }
    text_block.push_str("ET\nQ\n");
    append_text_ops_to_page_contents(&mut pdf, request.page_index, &text_block)?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes).map_err(|e| format!("serialize: {e}"))?;
    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(TextBlockEditResult {
        edit_id: edit_id.to_string(),
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        block_id: request.block_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        edited_object_ids: block.member_ids.clone(),
        before_text: before_text.to_string(),
        after_text: request.replacement_text.clone(),
        success: true,
        warnings: layout.warnings,
    })
}

#[derive(Debug, Clone)]
struct VisualReflowLayout {
    lines: Vec<String>,
    font_size: f32,
    line_height: f32,
    cover_rgb: [f32; 3],
    warnings: Vec<String>,
}

fn compute_visual_reflow_layout(
    replacement_text: &str,
    bbox: [f32; 4],
    requested_font_size: f32,
    overflow_policy: &BlockOverflowPolicy,
) -> Result<VisualReflowLayout, String> {
    if bbox.iter().any(|v| !v.is_finite()) || bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
        return Err("Cannot apply visual paragraph reflow: block bbox is degenerate.".to_string());
    }
    let block_w = (bbox[2] - bbox[0]).max(20.0);
    let block_h = (bbox[3] - bbox[1]).max(1.0);
    let mut font_size = requested_font_size.max(8.0);
    let mut lines = wrap_text_to_width(replacement_text, block_w, font_size);
    let mut line_height = font_size * 1.2;
    let mut used_height = lines.len() as f32 * line_height;
    let mut warnings = vec![
        "Visual paragraph reflow: original block covered and replacement text wrapped inside the block bounds. Native paragraph operator reflow was not claimed.".to_string(),
    ];

    if used_height > block_h {
        match overflow_policy {
            BlockOverflowPolicy::Reject => {
                return Err("Replacement text does not fit inside the original block height. Choose shrink-to-fit or allow overflow to apply.".to_string());
            }
            BlockOverflowPolicy::ShrinkToFit => {
                while used_height > block_h && font_size > 6.0 {
                    font_size = (font_size - 0.5).max(6.0);
                    lines = wrap_text_to_width(replacement_text, block_w, font_size);
                    line_height = font_size * 1.2;
                    used_height = lines.len() as f32 * line_height;
                }
                if used_height > block_h {
                    return Err("Replacement text still does not fit after shrinking to the safe minimum font size.".to_string());
                }
                warnings.push(format!("Shrink-to-fit reduced visual reflow font size to {:.1} pt.", font_size));
            }
            BlockOverflowPolicy::AllowOverflow => {
                warnings.push("Replacement text exceeds the original block height; visual overflow was explicitly allowed.".to_string());
            }
        }
    }

    Ok(VisualReflowLayout {
        lines,
        font_size,
        line_height,
        cover_rgb: [1.0, 1.0, 1.0],
        warnings,
    })
}

fn append_text_ops_to_page_contents(
    pdf: &mut mupdf::pdf::PdfDocument,
    page_index: usize,
    ops: &str,
) -> Result<(), String> {
    let stream_dict = pdf.new_dict().map_err(|e| format!("new text stream dict: {e}"))?;
    let mut new_stream = pdf.add_object(&stream_dict).map_err(|e| format!("add text stream: {e}"))?;
    new_stream.write_stream_string(ops)
        .map_err(|e| format!("write text stream: {e}"))?;

    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    let mut page_dict = pdf.find_page(page_no).map_err(|e| format!("find_page text append: {e}"))?;
    match page_dict.get_dict("Contents").map_err(|e| format!("get Contents text append: {e}"))? {
        Some(contents) if contents.is_array().unwrap_or(false) => {
            let mut arr = contents;
            arr.array_push(new_stream).map_err(|e| format!("array_push text stream: {e}"))?;
        }
        Some(contents) => {
            let mut arr = pdf.new_array().map_err(|e| format!("new Contents array: {e}"))?;
            arr.array_push(contents).map_err(|e| format!("array_push existing Contents: {e}"))?;
            arr.array_push(new_stream).map_err(|e| format!("array_push text stream: {e}"))?;
            page_dict.dict_put("Contents", arr).map_err(|e| format!("dict_put Contents array: {e}"))?;
        }
        None => {
            page_dict.dict_put("Contents", new_stream).map_err(|e| format!("dict_put text Contents: {e}"))?;
        }
    }
    Ok(())
}

/// Phase 28D — primitive word-wrap to bbox width using a fixed glyph
/// advance estimate (font_size * 0.5). This is intentionally crude:
/// we tell the user it's "wrap to bbox width", not real paragraph reflow.
pub fn wrap_text_to_width(text: &str, max_width_pts: f32, font_size: f32) -> Vec<String> {
    let avg_glyph_w = font_size * 0.5;
    let max_chars = ((max_width_pts / avg_glyph_w).floor() as usize).max(1);
    let mut out: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if current.is_empty() {
                if word.len() > max_chars {
                    // Long word — emit a hard break.
                    for chunk in word.as_bytes().chunks(max_chars) {
                        out.push(String::from_utf8_lossy(chunk).to_string());
                    }
                    continue;
                }
                current.push_str(word);
            } else if current.len() + 1 + word.len() <= max_chars {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(current);
                current = word.to_string();
            }
        }
        if !current.is_empty() {
            out.push(current);
        }
    }
    out
}

fn ensure_helvetica_resource(
    pdf: &mupdf::pdf::PdfDocument,
    page_obj: &mupdf::pdf::PdfObject,
) -> Result<(), String> {
    let resources = match page_obj.get_dict("Resources") {
        Ok(Some(r)) => r,
        _ => return Ok(()),
    };
    let font_dict = match resources.get_dict("Font") {
        Ok(Some(f)) => f,
        Ok(None) => {
            let mut new_font_dict = pdf.new_dict().map_err(|e| format!("new_dict: {e}"))?;
            let helv = pdf.new_object_from_str(
                "<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>",
            ).map_err(|e| format!("new_object_from_str: {e}"))?;
            new_font_dict.dict_put("R2HHelv", helv).map_err(|e| format!("dict_put: {e}"))?;
            let mut resources_mut = resources;
            resources_mut.dict_put("Font", new_font_dict).map_err(|e| format!("dict_put Font: {e}"))?;
            return Ok(());
        }
        Err(e) => return Err(format!("get Font dict: {e}")),
    };
    if let Ok(Some(_)) = font_dict.get_dict("R2HHelv") {
        return Ok(());
    }
    let helv = pdf.new_object_from_str(
        "<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>",
    ).map_err(|e| format!("new_object_from_str: {e}"))?;
    let mut font_dict_mut = font_dict;
    font_dict_mut.dict_put("R2HHelv", helv).map_err(|e| format!("dict_put R2HHelv: {e}"))?;
    Ok(())
}

fn pdf_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

fn fmt_color(v: f32) -> String {
    let clamped = v.clamp(0.0, 1.0);
    let s = format!("{:.4}", clamped);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn epoch_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(
        id: &str, text: &str, page: usize, size: f32, bbox: [f32; 4],
        level: EditableLevel,
    ) -> ContentObject {
        ContentObject {
            id: id.to_string(),
            session_id: "s1".to_string(),
            page_index: page,
            object_type: ContentObjectType::TextSpan,
            bbox,
            z_index: 0,
            editable_level: level,
            text_info: Some(TextInfo {
                raw_text: text.to_string(),
                decoded_text: text.to_string(),
                glyph_count: text.chars().count(),
                font_name: "Helvetica".to_string(),
                font_size: size,
                fill_color: None,
                stroke_color: None,
                writing_mode: "horizontal".to_string(),
                is_subset_font: false,
                encoding_safe: true,
                operator_offset: None,
                operator_length: None,
                occurrence_index: 0,
                operator_type: "tj".to_string(),
                content_stream_index: Some(0),
                operator_index: None,
                editable_strategy: "native_in_place".to_string(),
                unsupported_reason: vec![],
                font_encoding: Some("WinAnsiEncoding".to_string()),
                decoding_quality: "ok".to_string(),
            }),
            image_info: None,
            style_info: None,
            diagnostics: vec![],
        }
    }

    #[test]
    fn groups_three_consecutive_lines_into_one_block() {
        let objs = vec![
            span("a", "Line 1", 0, 12.0, [10.0, 70.0, 110.0, 82.0], EditableLevel::NativeEditable),
            span("b", "Line 2", 0, 12.0, [10.0, 56.0, 110.0, 68.0], EditableLevel::NativeEditable),
            span("c", "Line 3", 0, 12.0, [10.0, 42.0, 110.0, 54.0], EditableLevel::NativeEditable),
        ];
        let blocks = group_text_blocks(&objs);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].member_ids.len(), 3);
        assert_eq!(blocks[0].combined_text, "Line 1\nLine 2\nLine 3");
        assert!(blocks[0].all_native_editable);
    }

    #[test]
    fn far_apart_lines_get_separate_blocks() {
        let objs = vec![
            span("a", "Para 1", 0, 12.0, [10.0, 700.0, 200.0, 712.0], EditableLevel::NativeEditable),
            span("b", "Para 2", 0, 12.0, [10.0, 200.0, 200.0, 212.0], EditableLevel::NativeEditable),
        ];
        let blocks = group_text_blocks(&objs);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn different_font_size_splits_block() {
        let objs = vec![
            span("a", "Heading", 0, 18.0, [10.0, 690.0, 200.0, 710.0], EditableLevel::NativeEditable),
            span("b", "Body text", 0, 10.0, [10.0, 670.0, 200.0, 682.0], EditableLevel::NativeEditable),
        ];
        let blocks = group_text_blocks(&objs);
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn visual_patch_member_marks_block_not_native() {
        let objs = vec![
            span("a", "Hello", 0, 12.0, [10.0, 70.0, 110.0, 82.0], EditableLevel::NativeEditable),
            span("b", "日本語", 0, 12.0, [10.0, 56.0, 110.0, 68.0], EditableLevel::VisualPatchOnly),
        ];
        let blocks = group_text_blocks(&objs);
        assert_eq!(blocks.len(), 1);
        assert!(!blocks[0].all_native_editable);
        assert!(blocks[0].diagnostics.iter().any(|d| d.to_lowercase().contains("visual reflow")));
    }

    #[test]
    fn wrap_text_breaks_at_word_boundaries() {
        let lines = wrap_text_to_width("The quick brown fox jumps over the lazy dog", 60.0, 12.0);
        // With 12pt size and avg glyph width 6pt, max chars per line ≈ 10.
        assert!(lines.len() >= 2);
        for line in &lines {
            assert!(line.len() <= 10, "line '{line}' too wide");
        }
    }

    #[test]
    fn wrap_text_handles_explicit_newlines() {
        let lines = wrap_text_to_width("Line A\nLine B", 1000.0, 12.0);
        assert_eq!(lines, vec!["Line A".to_string(), "Line B".to_string()]);
    }

    #[test]
    fn visual_reflow_rejects_overflow_without_mutating() {
        let result = compute_visual_reflow_layout(
            "one two three four five six seven eight nine ten eleven twelve",
            [0.0, 0.0, 60.0, 18.0],
            12.0,
            &BlockOverflowPolicy::Reject,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not fit"));
    }

    #[test]
    fn visual_reflow_shrink_to_fit_reduces_font_size() {
        let layout = compute_visual_reflow_layout(
            "one two three four",
            [0.0, 0.0, 60.0, 18.0],
            12.0,
            &BlockOverflowPolicy::ShrinkToFit,
        ).unwrap();
        assert!(layout.font_size < 12.0);
        assert!(layout.warnings.iter().any(|w| w.contains("Shrink-to-fit")));
    }

    #[test]
    fn visual_reflow_defaults_to_white_cover_before_runtime_sampling() {
        let layout = compute_visual_reflow_layout(
            "short text",
            [0.0, 0.0, 200.0, 40.0],
            12.0,
            &BlockOverflowPolicy::Reject,
        ).unwrap();
        assert_eq!(layout.cover_rgb, [1.0, 1.0, 1.0]);
        assert!(layout.warnings.iter().any(|w| w.contains("Visual paragraph reflow")));
    }

    #[test]
    fn lines_match_count_compares_line_counts() {
        assert!(lines_match_count("A\nB", "X\nY"));
        assert!(!lines_match_count("A\nB", "X\nY\nZ"));
    }
}
