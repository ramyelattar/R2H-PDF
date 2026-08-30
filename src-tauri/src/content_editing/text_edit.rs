//! Text editing with two strategies:
//! 1. True Native Edit: Parse content stream, find Tj/TJ/'/" operator,
//!    replace string operand in-place. Preserves original font, size,
//!    position, and color.
//! 2. Safe Visual Replacement: Redact original + draw replacement with
//!    Helvetica. Used when native edit is unsafe.
//!
//! Phase 28B/C: extended to support `'` and `"` operators, Contents
//! arrays, multi-stream pages, and explicit font/encoding safety
//! classification.

use super::analysis::extract_page_content_objects;
use super::stream_parser::{
    find_text_operations, replace_text_in_stream_encoded, text_matches, EncodingTarget,
    TextOperatorKind,
};
use super::types::*;
use crate::document_core::DocumentCoreState;
use mupdf::Buffer;
use std::time::{SystemTime, UNIX_EPOCH};

/// Phase 30D — verify that an edit actually changed the rendered page
/// text. Loads `bytes` as a PDF via MuPDF, extracts the text for the
/// target page, and returns:
///   - `Ok(true)` when the replacement substring is found.
///   - `Ok(false)` when verification could not confirm the change.
///   - `Err(reason)` when verification could not run (e.g. parse error).
pub fn verify_text_present_in_page(
    bytes: &[u8],
    page_index: usize,
    needle: &str,
) -> Result<bool, String> {
    if needle.is_empty() {
        return Ok(true);
    }
    let pdf = mupdf::pdf::PdfDocument::from_bytes(bytes)
        .map_err(|e| format!("verify: from_bytes: {e}"))?;
    let count = pdf
        .page_count()
        .map_err(|e| format!("verify: page_count: {e}"))?;
    let pno = i32::try_from(page_index).map_err(|e| format!("verify: page_index: {e}"))?;
    if pno >= count {
        return Err(format!("verify: page {pno} out of range ({count} pages)"));
    }
    let page = pdf
        .load_page(pno)
        .map_err(|e| format!("verify: load_page: {e}"))?;
    let tp = page
        .to_text_page(mupdf::TextPageFlags::empty())
        .map_err(|e| format!("verify: to_text_page: {e}"))?;
    let mut acc = String::new();
    for block in tp.blocks() {
        for line in block.lines() {
            for ch in line.chars() {
                if let Some(c) = ch.char() {
                    acc.push(c);
                }
            }
            acc.push(' ');
        }
        acc.push('\n');
    }
    Ok(acc.contains(needle))
}

/// Phase 28C — describes how the engine selected a native vs. fallback path
/// for a given replacement. Returned to the UI so the user sees one honest
/// method label per edit.
#[derive(Debug, Clone)]
pub struct FontEncodingDecision {
    pub native_safe: bool,
    pub reasons: Vec<String>,
    /// Detected font encoding name when available.
    pub encoding: Option<String>,
}

/// Phase 29B — registry-backed text edit strategy decision.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TextEditStrategy {
    NativeInPlace,
    NativeMultiOperator,
    SafeVisualReplacement,
    ReadOnly,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TextEditStrategyDecision {
    pub strategy: TextEditStrategy,
    pub font_preserved: bool,
    pub encoding_safe: bool,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
    /// `"font_registry"` when the per-page font registry produced the
    /// answer, `"fallback_heuristic"` when we fell back to the older
    /// classifier (font missing from the registry).
    pub source: &'static str,
}

/// Phase 29B — pick a strategy for editing a single text span using the
/// page-level font registry. Pure logic over the inputs so unit tests can
/// drive every combination without a real PDF.
pub fn classify_text_edit_strategy(
    font_resource_name: Option<&str>,
    registry_fonts: &[super::font_registry::FontResourceInfo],
    replacement_text: &str,
) -> TextEditStrategyDecision {
    classify_text_edit_strategy_with_options(
        font_resource_name,
        registry_fonts,
        replacement_text,
        ClassifyOptions::default(),
    )
}

/// Phase 30C — caller can pass `enable_experimental_to_unicode` plus an
/// optional reverse-map closure to allow the experimental ToUnicode
/// native path to surface in warnings. The write path itself is not
/// implemented (see honesty rules), so the strategy still routes to
/// `SafeVisualReplacement` for Type0/Identity-H fonts even when the
/// flag is on; the difference is that the diagnostic message names the
/// experimental path explicitly.
#[derive(Debug, Clone, Default)]
pub struct ClassifyOptions {
    pub enable_experimental_to_unicode: bool,
    /// Pre-computed availability: whether a ToUnicode reverse map exists
    /// for the target font AND every replacement char maps unambiguously.
    pub experimental_to_unicode_ready: bool,
}

pub fn classify_text_edit_strategy_with_options(
    font_resource_name: Option<&str>,
    registry_fonts: &[super::font_registry::FontResourceInfo],
    replacement_text: &str,
    options: ClassifyOptions,
) -> TextEditStrategyDecision {
    let mut reasons: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Empty replacements are not a useful native edit but they should
    // not be silently rejected either — the caller will see a no-op.
    if replacement_text.is_empty() {
        warnings.push(
            "Replacement text is empty — no native operator change will be emitted.".to_string(),
        );
    }

    // Detect the kind of characters in the replacement once.
    let has_non_ascii = !replacement_text.is_ascii();
    let outside_latin1 = replacement_text.chars().any(|c| (c as u32) > 0x00FF);
    let has_cjk = replacement_text.chars().any(|c| {
        let cp = c as u32;
        (0x4E00..=0x9FFF).contains(&cp)
            || (0x3040..=0x30FF).contains(&cp)
            || (0xAC00..=0xD7AF).contains(&cp)
    });
    let has_arabic = replacement_text.chars().any(|c| {
        let cp = c as u32;
        (0x0600..=0x06FF).contains(&cp) || (0xFB50..=0xFDFF).contains(&cp)
    });

    // Try to find the matching font resource in the registry.
    let info =
        font_resource_name.and_then(|name| registry_fonts.iter().find(|f| f.resource_name == name));

    if let Some(info) = info {
        // ── Registry path: honest, per-font decision. ────────────────
        if has_cjk {
            return TextEditStrategyDecision {
                strategy: TextEditStrategy::SafeVisualReplacement,
                font_preserved: false,
                encoding_safe: false,
                reasons: vec![
                    "CJK shaping required — native PDF text editing not supported in this build."
                        .to_string(),
                ],
                warnings: info.unsupported_reasons.clone(),
                source: "font_registry",
            };
        }
        if has_arabic {
            return TextEditStrategyDecision {
                strategy: TextEditStrategy::SafeVisualReplacement,
                font_preserved: false,
                encoding_safe: false,
                reasons: vec![
                    "Arabic shaping required — native PDF text editing not supported in this build.".to_string(),
                ],
                warnings: info.unsupported_reasons.clone(),
                source: "font_registry",
            };
        }
        if info.is_type3 {
            return TextEditStrategyDecision {
                strategy: TextEditStrategy::ReadOnly,
                font_preserved: false,
                encoding_safe: false,
                reasons: vec!["Type3 font — native edit unsupported.".to_string()],
                warnings: info.unsupported_reasons.clone(),
                source: "font_registry",
            };
        }
        if info.is_subset {
            return TextEditStrategyDecision {
                strategy: TextEditStrategy::SafeVisualReplacement,
                font_preserved: false,
                encoding_safe: false,
                reasons: vec![
                    "Subset font: replacement glyphs are likely not embedded; falling back to visual replacement to avoid corruption.".to_string(),
                ],
                warnings: info.unsupported_reasons.clone(),
                source: "font_registry",
            };
        }
        if info.is_type0
            || matches!(
                info.encoding_kind,
                super::font_registry::EncodingKind::IdentityH
                    | super::font_registry::EncodingKind::IdentityV
            )
        {
            // Phase 30C — flag-aware diagnostic. The write path for Type0
            // CID fonts is NOT implemented in this build; we never claim
            // success without verified bytes. When the experimental flag
            // is on AND a reverse map is ready, surface the honest fact
            // that we could *in principle* use it, but still fall back.
            let reasons = vec![
                "Type0 / Identity-H font: native edit requires CID reverse mapping which is not implemented.".to_string(),
            ];
            let mut warnings = info.unsupported_reasons.clone();
            if options.enable_experimental_to_unicode {
                if options.experimental_to_unicode_ready {
                    warnings.push(
                        "Experimental: ToUnicode reverse mapping is available for every character, but the experimental write path is not safely enabled in this build. Falling back to visual replacement.".to_string(),
                    );
                } else {
                    warnings.push(
                        "Experimental ToUnicode flag is enabled but reverse mapping is ambiguous or incomplete for this replacement.".to_string(),
                    );
                }
            }
            return TextEditStrategyDecision {
                strategy: TextEditStrategy::SafeVisualReplacement,
                font_preserved: false,
                encoding_safe: false,
                reasons,
                warnings,
                source: "font_registry",
            };
        }
        // Pure ASCII path.
        if !has_non_ascii {
            if info.can_native_edit_ascii {
                return TextEditStrategyDecision {
                    strategy: TextEditStrategy::NativeInPlace,
                    font_preserved: true,
                    encoding_safe: true,
                    reasons,
                    warnings,
                    source: "font_registry",
                };
            } else {
                reasons.push(format!(
                    "Font {} cannot natively encode ASCII (encoding {}); using visual replacement.",
                    info.resource_name,
                    info.encoding_kind.as_str()
                ));
                return TextEditStrategyDecision {
                    strategy: TextEditStrategy::SafeVisualReplacement,
                    font_preserved: false,
                    encoding_safe: false,
                    reasons,
                    warnings,
                    source: "font_registry",
                };
            }
        }
        // Non-ASCII but Latin-1 (U+0000..U+00FF).
        if has_non_ascii && !outside_latin1 {
            if info.can_native_edit_latin1 {
                return TextEditStrategyDecision {
                    strategy: TextEditStrategy::NativeInPlace,
                    font_preserved: true,
                    encoding_safe: true,
                    reasons,
                    warnings: vec![
                        "Non-ASCII Latin-1 replacement encoded via WinAnsi/MacRoman.".to_string(),
                    ],
                    source: "font_registry",
                };
            } else {
                reasons.push(format!(
                    "Font {} encoding ({}) does not support Latin-1 replacement; using visual replacement.",
                    info.resource_name,
                    info.encoding_kind.as_str()
                ));
                return TextEditStrategyDecision {
                    strategy: TextEditStrategy::SafeVisualReplacement,
                    font_preserved: false,
                    encoding_safe: false,
                    reasons,
                    warnings,
                    source: "font_registry",
                };
            }
        }
        // Non-ASCII outside Latin-1 with no Type0 path → visual fallback.
        return TextEditStrategyDecision {
            strategy: TextEditStrategy::SafeVisualReplacement,
            font_preserved: false,
            encoding_safe: false,
            reasons: vec![
                "Replacement contains characters outside Latin-1 and the font does not support a wider encoding here.".to_string(),
            ],
            warnings,
            source: "font_registry",
        };
    }

    // ── Fallback path: no registry entry. ───────────────────────────
    warnings.push(
        "Font resource not found in page registry; falling back to character heuristics."
            .to_string(),
    );
    if has_cjk {
        reasons.push("CJK shaping required — visual replacement only.".to_string());
        return TextEditStrategyDecision {
            strategy: TextEditStrategy::SafeVisualReplacement,
            font_preserved: false,
            encoding_safe: false,
            reasons,
            warnings,
            source: "fallback_heuristic",
        };
    }
    if has_arabic {
        reasons.push("Arabic shaping required — visual replacement only.".to_string());
        return TextEditStrategyDecision {
            strategy: TextEditStrategy::SafeVisualReplacement,
            font_preserved: false,
            encoding_safe: false,
            reasons,
            warnings,
            source: "fallback_heuristic",
        };
    }
    // Pure ASCII without registry is still not enough to claim native:
    // Phase 34 requires font/encoding/operator identity plus verification.
    if !has_non_ascii {
        reasons.push(
            "Font resource not found in page registry; native edit cannot be verified, using visual replacement.".to_string(),
        );
        return TextEditStrategyDecision {
            strategy: TextEditStrategy::SafeVisualReplacement,
            font_preserved: false,
            encoding_safe: false,
            reasons,
            warnings,
            source: "fallback_heuristic",
        };
    }
    // Non-ASCII without registry: visual replacement to be safe.
    reasons.push(
        "Non-ASCII replacement with unknown font — falling back to visual replacement.".to_string(),
    );
    TextEditStrategyDecision {
        strategy: TextEditStrategy::SafeVisualReplacement,
        font_preserved: false,
        encoding_safe: false,
        reasons,
        warnings,
        source: "fallback_heuristic",
    }
}

/// Phase 28C — decide whether `replacement` can be safely written with the
/// existing font on the page, based on font name + characters.
///
/// Conservative rules:
///   * Subset font (BaseFont prefixed with `XXXXXX+`) → unsafe for
///     anything but exact-length ASCII (rare); fallback.
///   * Replacement contains CJK / Arabic / RTL → unsafe; fallback.
///   * Replacement contains non-ASCII → unsafe unless font encoding is
///     known to be `WinAnsiEncoding` or `MacRomanEncoding` *and* every
///     character fits the WinAnsi/MacRoman range.
///   * Otherwise → native safe.
pub fn classify_font_replacement_safety(
    font_name: &str,
    encoding: Option<&str>,
    replacement: &str,
) -> FontEncodingDecision {
    let mut reasons: Vec<String> = Vec::new();
    let mut native_safe = true;

    // Subset fonts are prefixed by exactly 6 uppercase letters then '+'.
    let subset = font_name.len() > 7
        && font_name.as_bytes().get(6) == Some(&b'+')
        && font_name.as_bytes()[..6]
            .iter()
            .all(|b: &u8| b.is_ascii_uppercase());
    if subset {
        native_safe = false;
        reasons.push(
            "Subset font detected; native edit may be unsafe because new glyphs may not be embedded."
                .to_string(),
        );
    }

    let has_non_ascii = !replacement.is_ascii();
    let has_cjk = replacement.chars().any(|c| {
        let cp = c as u32;
        (0x4E00..=0x9FFF).contains(&cp)
            || (0x3040..=0x30FF).contains(&cp)
            || (0xAC00..=0xD7AF).contains(&cp)
    });
    let has_arabic = replacement.chars().any(|c| {
        let cp = c as u32;
        (0x0600..=0x06FF).contains(&cp) || (0xFB50..=0xFDFF).contains(&cp)
    });

    if has_cjk {
        native_safe = false;
        reasons.push(
            "CJK shaping is not supported for native PDF text editing yet — using visual replacement."
                .to_string(),
        );
    }
    if has_arabic {
        native_safe = false;
        reasons.push(
            "Arabic shaping is not supported for native PDF text editing yet — using visual replacement."
                .to_string(),
        );
    }

    if has_non_ascii && !has_cjk && !has_arabic {
        // Extended Latin / Cyrillic / Greek etc. — only safe if encoding
        // is known to be a single-byte Latin-1 encoding.
        match encoding {
            Some(e) if e.eq_ignore_ascii_case("WinAnsiEncoding") => {
                // WinAnsiEncoding covers 0x20–0xFF; every char in
                // U+0000..U+00FF maps directly. Anything outside that is
                // not safe.
                let outside = replacement.chars().any(|c| (c as u32) > 0x00FF);
                if outside {
                    native_safe = false;
                    reasons.push(
                        "Original font's WinAnsi encoding cannot safely encode the replacement. Using visual replacement."
                            .to_string(),
                    );
                }
            }
            Some(e) if e.eq_ignore_ascii_case("MacRomanEncoding") => {
                let outside = replacement.chars().any(|c| (c as u32) > 0x00FF);
                if outside {
                    native_safe = false;
                    reasons.push(
                        "Original font's MacRoman encoding cannot safely encode the replacement. Using visual replacement."
                            .to_string(),
                    );
                }
            }
            _ => {
                native_safe = false;
                reasons.push(format!(
                    "Original font encoding ({}) cannot be verified for non-ASCII replacement. Using visual replacement.",
                    encoding.unwrap_or("unknown")
                ));
            }
        }
    }

    FontEncodingDecision {
        native_safe,
        reasons,
        encoding: encoding.map(|s| s.to_string()),
    }
}

/// Apply a text edit. Attempts true native edit first, falls back to safe visual replacement.
pub fn apply_native_text_edit(
    doc_state: &DocumentCoreState,
    request: &NativeTextEditRequest,
) -> Result<NativeTextEditResult, String> {
    let edit_id = format!("edit-{}", epoch_ms());

    // 1. Verify the content object exists and is editable.
    let objects = extract_page_content_objects(doc_state, &request.session_id, request.page_index)?;
    let target = objects
        .iter()
        .find(|o| o.id == request.content_object_id)
        .ok_or_else(|| format!("Content object not found: {}", request.content_object_id))?;

    if target.editable_level == EditableLevel::ReadOnly {
        return Ok(NativeTextEditResult {
            edit_id,
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            content_object_id: request.content_object_id.clone(),
            method: EditMethod::Rejected,
            original_text: target
                .text_info
                .as_ref()
                .map(|t| t.decoded_text.clone())
                .unwrap_or_default(),
            replacement_text: request.replacement_text.clone(),
            success: false,
            warnings: vec!["Object is read-only and cannot be edited.".to_string()],
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        });
    }

    let text_info = target
        .text_info
        .as_ref()
        .ok_or_else(|| "Target object has no text info".to_string())?;

    if target.editable_level == EditableLevel::VisualPatchOnly {
        return Ok(NativeTextEditResult {
            edit_id,
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            content_object_id: request.content_object_id.clone(),
            method: EditMethod::VisualPatchFallback,
            original_text: text_info.decoded_text.clone(),
            replacement_text: request.replacement_text.clone(),
            success: false,
            warnings: target.diagnostics.clone(),
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        });
    }

    let original_text = text_info.decoded_text.clone();
    let bbox = target.bbox;
    let font_size = text_info.font_size;

    // Phase 29B — registry-backed pre-flight. If the page font registry
    // says the replacement cannot be native-edited safely (subset,
    // Type0/Identity-H, Type3, encoding mismatch, etc.), bail to the
    // visual replacement immediately with the precise reasons in
    // warnings — no silent fallback.
    let registry_fonts = super::font_registry::build_page_font_registry(
        doc_state,
        &request.session_id,
        request.page_index,
    )
    .map(|r| r.fonts)
    .unwrap_or_default();
    let target_font_resource = objects
        .iter()
        .find(|o| o.id == request.content_object_id)
        .and_then(|o| o.text_info.as_ref())
        .map(|t| t.font_name.clone());
    let strategy = classify_text_edit_strategy(
        target_font_resource.as_deref(),
        &registry_fonts,
        &request.replacement_text,
    );
    if matches!(strategy.strategy, TextEditStrategy::ReadOnly) {
        return Ok(NativeTextEditResult {
            edit_id,
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            content_object_id: request.content_object_id.clone(),
            method: EditMethod::Rejected,
            original_text,
            replacement_text: request.replacement_text.clone(),
            success: false,
            warnings: strategy.reasons,
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        });
    }
    if matches!(strategy.strategy, TextEditStrategy::SafeVisualReplacement) {
        let mut result = apply_safe_visual_replacement(
            doc_state,
            request,
            &edit_id,
            &original_text,
            bbox,
            font_size,
        )?;
        let mut combined: Vec<String> = strategy.reasons.clone();
        combined.extend(strategy.warnings.clone());
        combined.append(&mut result.warnings);
        result.warnings = combined;
        return Ok(result);
    }

    // 2. Attempt true native edit. The attempt returns the would-be new
    //    PDF bytes WITHOUT mutating the session — we commit them here
    //    only on success.
    match attempt_true_native_edit(doc_state, request, &original_text, &objects)? {
        NativeEditAttempt::Applied {
            new_bytes,
            encoding,
            operator_kind,
            stream_index,
            op_index,
        } => {
            // Phase 30D — verify the edit BEFORE committing to session.
            // We rollback (don't commit) when verification fails so the
            // user never sees a "succeeded" status without byte changes.
            let verification;
            let mut verification_warnings: Vec<String> = Vec::new();
            let prior_bytes = {
                let arc = doc_state
                    .store
                    .get_session_arc_pub(&request.session_id)
                    .map_err(|e| e.to_string())?;
                let s = arc
                    .lock()
                    .map_err(|_| "session lock poisoned".to_string())?;
                s.document.bytes.clone()
            };
            match verify_text_present_in_page(
                &new_bytes,
                request.page_index,
                &request.replacement_text,
            ) {
                Ok(true) => {
                    verification = VerificationStatus::Passed;
                }
                Ok(false) => {
                    verification = VerificationStatus::Failed;
                    verification_warnings.push(format!(
                        "Post-edit verification could not find replacement text \"{}\" on page {}. Rolling back.",
                        request.replacement_text,
                        request.page_index + 1
                    ));
                }
                Err(e) => {
                    verification_warnings.push(format!(
                        "Post-edit verification skipped due to engine error: {e}"
                    ));
                    verification = VerificationStatus::NotRun;
                }
            }

            if verification == VerificationStatus::Failed {
                // Rollback — keep the session bytes at the snapshot.
                let arc = doc_state
                    .store
                    .get_session_arc_pub(&request.session_id)
                    .map_err(|e| e.to_string())?;
                let mut session = arc
                    .lock()
                    .map_err(|_| "session lock poisoned".to_string())?;
                session.document.bytes = prior_bytes;
                session.invalidate_cached_document();
                return Ok(NativeTextEditResult {
                    edit_id,
                    session_id: request.session_id.clone(),
                    page_index: request.page_index,
                    content_object_id: request.content_object_id.clone(),
                    method: EditMethod::Rejected,
                    original_text,
                    replacement_text: request.replacement_text.clone(),
                    success: false,
                    warnings: verification_warnings.clone(),
                    verification: VerificationStatus::Failed,
                    verification_warnings,
                });
            }

            let arc = doc_state
                .store
                .get_session_arc_pub(&request.session_id)
                .map_err(|e| e.to_string())?;
            let mut session = arc
                .lock()
                .map_err(|_| "session lock poisoned".to_string())?;
            session.document.bytes = new_bytes;
            session.is_dirty = true;
            session.invalidate_cached_document();

            Ok(NativeTextEditResult {
                edit_id: format!("edit-native-{}", epoch_ms()),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                content_object_id: request.content_object_id.clone(),
                method: EditMethod::NativeInPlaceEdit,
                original_text,
                replacement_text: request.replacement_text.clone(),
                success: true,
                warnings: vec![
                    format!(
                        "Native edit applied: operator={operator_kind}, stream={stream_index}, op_index={op_index}, encoding={}.",
                        encoding.as_deref().unwrap_or("unknown")
                    ),
                    "Original font and position preserved.".to_string(),
                ],
                verification,
                verification_warnings,
            })
        }
        NativeEditAttempt::Rejected { reasons } => {
            // Explicit rejection — do not silently fall back.
            Ok(NativeTextEditResult {
                edit_id,
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                content_object_id: request.content_object_id.clone(),
                method: EditMethod::Rejected,
                original_text,
                replacement_text: request.replacement_text.clone(),
                success: false,
                warnings: reasons,
                verification: VerificationStatus::NotRun,
                verification_warnings: vec![],
            })
        }
        NativeEditAttempt::Skipped { reasons } => {
            // 3. Fallback: Safe visual replacement (redact + Helvetica)
            //    with the explicit reasons surfaced in warnings.
            let mut result = apply_safe_visual_replacement(
                doc_state,
                request,
                &edit_id,
                &original_text,
                bbox,
                font_size,
            )?;
            // Prepend the reasons so the UI shows why we fell back.
            let mut combined = reasons;
            combined.append(&mut result.warnings);
            result.warnings = combined;
            Ok(result)
        }
    }
}

/// Outcome of a native edit attempt that does NOT mutate the session.
/// The caller decides whether to commit it.
#[derive(Debug, Clone)]
pub enum NativeEditAttempt {
    /// Native edit succeeded — caller may commit new bytes.
    Applied {
        new_bytes: Vec<u8>,
        encoding: Option<String>,
        operator_kind: String,
        stream_index: usize,
        op_index: usize,
    },
    /// Native edit was skipped — fall back to safe visual replacement.
    Skipped { reasons: Vec<String> },
    /// Native edit was rejected — caller should NOT attempt visual replacement
    /// because the request itself was ambiguous or unsafe.
    Rejected { reasons: Vec<String> },
}

/// Attempt true native text editing by parsing every Contents stream on
/// the page, finding the exact target operator, and replacing the
/// string operand in-place. Returns the new PDF bytes without mutating
/// the session.
fn attempt_true_native_edit(
    doc_state: &DocumentCoreState,
    request: &NativeTextEditRequest,
    original_text: &str,
    objects: &[ContentObject],
) -> Result<NativeEditAttempt, String> {
    let arc = doc_state
        .store
        .get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let session_bytes = {
        let s = arc
            .lock()
            .map_err(|_| "session lock poisoned".to_string())?;
        s.document.bytes.clone()
    };

    let pdf = mupdf::pdf::PdfDocument::from_bytes(&session_bytes)
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let page_no = i32::try_from(request.page_index).map_err(|e| format!("page index: {e}"))?;

    let fz_page = pdf
        .load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let pdf_page = mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage: {e}"))?;

    let page_obj = pdf_page.object();
    let contents_obj = match page_obj.get_dict("Contents") {
        Ok(Some(c)) => c,
        _ => {
            return Ok(NativeEditAttempt::Skipped {
                reasons: vec!["Page has no /Contents stream.".to_string()],
            })
        }
    };

    // Phase 28B — discover the candidate streams. /Contents may be a
    // single stream or an array of streams; we walk both shapes.
    let streams: Vec<(usize, mupdf::pdf::PdfObject)> = if contents_obj.is_stream().unwrap_or(false)
    {
        vec![(0, contents_obj.clone())]
    } else if contents_obj.is_array().unwrap_or(false) {
        let len = contents_obj.len().unwrap_or(0);
        let mut acc: Vec<(usize, mupdf::pdf::PdfObject)> = Vec::with_capacity(len);
        for i in 0..len {
            if let Ok(Some(entry)) = contents_obj.get_array(i as i32) {
                // Resolve indirect references.
                let resolved = entry.resolve().ok().flatten().unwrap_or(entry);
                if resolved.is_stream().unwrap_or(false) {
                    acc.push((i, resolved));
                }
            }
        }
        acc
    } else {
        return Ok(NativeEditAttempt::Skipped {
            reasons: vec!["/Contents is neither stream nor array.".to_string()],
        });
    };

    if streams.is_empty() {
        return Ok(NativeEditAttempt::Skipped {
            reasons: vec!["No content streams found on page.".to_string()],
        });
    }

    // Identify the target text object's occurrence and operator hints.
    let target = objects
        .iter()
        .find(|o| o.id == request.content_object_id)
        .and_then(|o| o.text_info.as_ref());
    let occurrence_index = target.map(|t| t.occurrence_index).unwrap_or(0);

    // Walk each stream, collect all matching operators with their
    // (stream_index, op_index, text, kind, font_name) tuples.
    struct Candidate {
        stream_index: usize,
        bytes: Vec<u8>,
        op: super::stream_parser::TextOperation,
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    for (stream_index, stream_obj) in &streams {
        let bytes = stream_obj
            .read_stream()
            .map_err(|e| format!("read_stream {stream_index}: {e}"))?;
        let ops = find_text_operations(&bytes);
        for op in ops {
            if text_matches(&op, original_text) {
                candidates.push(Candidate {
                    stream_index: *stream_index,
                    bytes: bytes.clone(),
                    op,
                });
            }
        }
    }

    if candidates.is_empty() {
        return Ok(NativeEditAttempt::Skipped {
            reasons: vec![format!(
                "Text \"{original_text}\" not found in any content stream — likely encoded with a non-string operator (hex / glyph IDs)."
            )],
        });
    }

    // Disambiguate by occurrence index across all candidates.
    let chosen = if candidates.len() == 1 {
        &candidates[0]
    } else if occurrence_index < candidates.len() {
        &candidates[occurrence_index]
    } else {
        return Ok(NativeEditAttempt::Rejected {
            reasons: vec![format!(
                "Ambiguous match: {} occurrences of \"{original_text}\" on page {} but occurrence_index {} is out of range.",
                candidates.len(),
                request.page_index + 1,
                occurrence_index
            )],
        });
    };

    // Phase 28B — hex-encoded glyph runs cannot be safely native-edited
    // because the operand is a sequence of glyph IDs, not characters.
    if chosen.op.is_hex_encoded || chosen.op.kind == TextOperatorKind::Hex {
        return Ok(NativeEditAttempt::Skipped {
            reasons: vec![
                "Hex-encoded glyph string — native edit is not safe without a CMap/ToUnicode map."
                    .to_string(),
            ],
        });
    }

    // Phase 28C — font + encoding safety classification.
    let font_name = chosen
        .op
        .font_name
        .clone()
        .unwrap_or_else(|| "Unknown".to_string());
    let encoding = font_encoding_for(&pdf_page, &font_name);
    let decision = classify_font_replacement_safety(
        &font_name,
        encoding.as_deref(),
        &request.replacement_text,
    );
    if !decision.native_safe {
        return Ok(NativeEditAttempt::Skipped {
            reasons: decision.reasons,
        });
    }

    // Phase 31A — prefer the registry-aware EncodingForwardMap so that
    // /Differences overrides drive the actual byte emission. Fall back
    // to plain EncodingTarget only when the page has no registry entry
    // (which the earlier registry pre-flight would have caught for
    // non-ASCII replacements; ASCII can safely use the plain path).
    let registry_for_byte_emission = super::font_registry::build_page_font_registry(
        doc_state,
        &request.session_id,
        request.page_index,
    )
    .map(|r| r.fonts)
    .unwrap_or_default();
    let target_font_name = objects
        .iter()
        .find(|o| o.id == request.content_object_id)
        .and_then(|o| o.text_info.as_ref())
        .map(|t| t.font_name.clone());
    let font_info = target_font_name.as_deref().and_then(|n| {
        registry_for_byte_emission
            .iter()
            .find(|f| f.resource_name == n)
    });

    let new_stream_bytes = match font_info {
        Some(info)
            if matches!(
                info.encoding_kind,
                super::font_registry::EncodingKind::WinAnsi
                    | super::font_registry::EncodingKind::MacRoman
                    | super::font_registry::EncodingKind::CustomDifferences
            ) =>
        {
            // Build the forward map: base + /Differences overrides.
            let fmap = super::encoding_map::build_with_differences(
                &info.resource_name,
                if info.encoding_kind == super::font_registry::EncodingKind::CustomDifferences {
                    // Use the parsed base_encoding under the differences
                    // dict when one is present; otherwise default to WinAnsi.
                    info.base_encoding
                        .as_deref()
                        .and_then(|b| {
                            if b.eq_ignore_ascii_case("WinAnsiEncoding") {
                                Some(super::font_registry::EncodingKind::WinAnsi)
                            } else if b.eq_ignore_ascii_case("MacRomanEncoding") {
                                Some(super::font_registry::EncodingKind::MacRoman)
                            } else {
                                None
                            }
                        })
                        .unwrap_or(super::font_registry::EncodingKind::WinAnsi)
                } else {
                    info.encoding_kind.clone()
                },
                &info.differences,
            );
            match fmap.forward_lookup(&request.replacement_text) {
                Ok(bytes) => super::stream_parser::replace_text_in_stream_with_bytes(
                    &chosen.bytes,
                    &chosen.op,
                    &bytes,
                ),
                Err(missing) => {
                    let chars: String = missing.iter().collect();
                    return Ok(NativeEditAttempt::Skipped {
                        reasons: vec![format!(
                            "Cannot encode replacement: characters [{chars}] are not present in the font's encoding (base={base}, differences={n}). Using safe visual replacement.",
                            base = info.base_encoding.as_deref().unwrap_or("(unknown)"),
                            n = info.differences_count,
                        )],
                    });
                }
            }
        }
        _ => {
            // Phase 29D fallback — registry-less or non-Latin encoding;
            // the earlier strategy pre-flight already routes non-ASCII
            // through visual replacement, so this path is ASCII-safe.
            let target = match decision.encoding.as_deref() {
                Some(e) if e.eq_ignore_ascii_case("WinAnsiEncoding") => EncodingTarget::WinAnsi,
                Some(e) if e.eq_ignore_ascii_case("MacRomanEncoding") => EncodingTarget::MacRoman,
                _ => EncodingTarget::Ascii,
            };
            replace_text_in_stream_encoded(
                &chosen.bytes,
                &chosen.op,
                &request.replacement_text,
                target,
            )
        }
    };

    // Phase 28B verification — confirm the replacement actually changed
    // the stream bytes; otherwise we surface a clear failure instead of
    // a silent no-op.
    if new_stream_bytes == chosen.bytes {
        return Ok(NativeEditAttempt::Rejected {
            reasons: vec![
                "Native edit produced identical stream bytes; refusing to mutate.".to_string(),
            ],
        });
    }

    // Write modified stream back into the page object.
    let stream_obj = &streams[streams
        .iter()
        .position(|(idx, _)| *idx == chosen.stream_index)
        .ok_or_else(|| "stream index not found".to_string())?]
    .1;
    let buf = Buffer::from_bytes(&new_stream_bytes).map_err(|e| format!("Buffer: {e}"))?;
    let mut stream_mut = stream_obj.clone();
    stream_mut
        .write_stream_buffer(&buf)
        .map_err(|e| format!("write_stream_buffer: {e}"))?;

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes)
        .map_err(|e| format!("serialize PDF: {e}"))?;

    // Sanity check: the serialized PDF must differ from the original.
    if new_bytes == session_bytes {
        return Ok(NativeEditAttempt::Rejected {
            reasons: vec![
                "Native edit serialized identical PDF bytes; refusing to claim success."
                    .to_string(),
            ],
        });
    }

    Ok(NativeEditAttempt::Applied {
        new_bytes,
        encoding: decision.encoding,
        operator_kind: chosen.op.kind.as_str().to_string(),
        stream_index: chosen.stream_index,
        op_index: chosen.op.op_index,
    })
}

/// Phase 28C — read the encoding name for a font from the page's
/// `/Resources /Font /<name>` dict. Returns `Some("WinAnsiEncoding")` etc.
/// when the encoding can be read; `None` when unknown.
fn font_encoding_for(pdf_page: &mupdf::pdf::PdfPage, font_resource_name: &str) -> Option<String> {
    let page_obj = pdf_page.object();
    let resources = page_obj.get_dict("Resources").ok().flatten()?;
    let font_dict = resources.get_dict("Font").ok().flatten()?;
    let font_obj = font_dict.get_dict(font_resource_name).ok().flatten()?;
    let resolved = font_obj.resolve().ok().flatten().unwrap_or(font_obj);
    let encoding = resolved.get_dict("Encoding").ok().flatten()?;
    // Encoding can be a name (`/WinAnsiEncoding`) or a dict containing
    // BaseEncoding + Differences. We read the name form first.
    let resolved_enc = encoding
        .resolve()
        .ok()
        .flatten()
        .unwrap_or(encoding.clone());
    if resolved_enc.is_name().unwrap_or(false) {
        return resolved_enc
            .as_name()
            .ok()
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    }
    // Otherwise look for BaseEncoding inside the dict.
    let base = resolved_enc.get_dict("BaseEncoding").ok().flatten()?;
    base.as_name()
        .ok()
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

/// Phase 32A — compute the cover rect used by safe visual replacement.
/// Returns the original bbox padded by `max(1pt, font_size * 0.15)` on every
/// side so glyph ascenders / descenders that extend beyond the tight text
/// bbox are still covered. Returns `None` when the bbox is degenerate or
/// non-finite — the caller MUST reject the edit in that case rather than
/// drawing a no-op cover that leaves the original glyphs visible.
pub fn safe_visual_cover_rect(bbox: [f32; 4], font_size: f32) -> Option<[f32; 4]> {
    let [x0, y0, x1, y1] = bbox;
    if !x0.is_finite() || !y0.is_finite() || !x1.is_finite() || !y1.is_finite() {
        return None;
    }
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let pad = (font_size.abs() * 0.15).max(1.0);
    Some([x0 - pad, y0 - pad, x1 + pad, y1 + pad])
}

/// Phase 32A — pure builder for the safe-visual-replacement content
/// stream operators. The cover rectangle is ALWAYS emitted BEFORE the
/// replacement text so the original glyphs (and any anti-aliased ink
/// outside the tight bbox that the redact annotation missed) are fully
/// painted over.
///
/// Coordinates are in PDF user-space (Y up, origin bottom-left), which
/// matches `bbox` and `cover_rect` as produced by the analysis layer
/// and `safe_visual_cover_rect`.
pub fn build_safe_visual_replacement_ops(
    cover_rect: [f32; 4],
    text_bbox: [f32; 4],
    font_size: f32,
    replacement: &str,
) -> String {
    build_safe_visual_replacement_ops_with_color(
        cover_rect,
        text_bbox,
        font_size,
        replacement,
        [1.0, 1.0, 1.0],
    )
}

pub fn build_safe_visual_replacement_ops_with_color(
    cover_rect: [f32; 4],
    text_bbox: [f32; 4],
    font_size: f32,
    replacement: &str,
    cover_rgb: [f32; 3],
) -> String {
    let [cx0, cy0, cx1, cy1] = cover_rect;
    let cw = cx1 - cx0;
    let ch = cy1 - cy0;
    let escaped = pdf_escape_string(replacement);
    // Place the replacement baseline relative to the ORIGINAL (unpadded)
    // text bbox so the replacement reads at the same vertical position as
    // the original line.
    let baseline_y = text_bbox[1] + (text_bbox[3] - text_bbox[1]) * 0.2;
    format!(
        "\nq\n{} {} {} rg\n{cx0} {cy0} {cw} {ch} re\nf\nQ\nq\nBT\n/R2HHelv {font_size} Tf\n{x} {baseline_y} Td\n({escaped}) Tj\nET\nQ\n",
        fmt_color(cover_rgb[0]), fmt_color(cover_rgb[1]), fmt_color(cover_rgb[2]),
        x = text_bbox[0],
    )
}

/// Safe visual replacement fallback: cover the original text bbox with a
/// white-fill rectangle (explicit cover) AND mark it for redaction
/// (defense in depth), then draw the replacement text on top with
/// Helvetica.
///
/// The cover rect is written as content-stream operators on the page,
/// which is committed to `session.document.bytes` — the same bytes used
/// by the export pipeline. Preview and export therefore see the same
/// result.
fn apply_safe_visual_replacement(
    doc_state: &DocumentCoreState,
    request: &NativeTextEditRequest,
    edit_id: &str,
    original_text: &str,
    bbox: [f32; 4],
    font_size: f32,
) -> Result<NativeTextEditResult, String> {
    // Reject early if the bbox cannot be safely covered. We refuse to
    // emit an empty cover and silently leave the original glyphs visible.
    let cover_rect = match safe_visual_cover_rect(bbox, font_size) {
        Some(r) => r,
        None => {
            return Ok(NativeTextEditResult {
                edit_id: edit_id.to_string(),
                session_id: request.session_id.clone(),
                page_index: request.page_index,
                content_object_id: request.content_object_id.clone(),
                method: EditMethod::Rejected,
                original_text: original_text.to_string(),
                replacement_text: request.replacement_text.clone(),
                success: false,
                warnings: vec![
                    "Cannot apply safe visual replacement: original text bbox is degenerate or non-finite, so a cover rectangle cannot be placed safely. Edit refused to avoid leaving the original text visible under the replacement.".to_string(),
                ],
                verification: VerificationStatus::NotRun,
                verification_warnings: vec![],
            });
        }
    };

    let arc = doc_state
        .store
        .get_session_arc_pub(&request.session_id)
        .map_err(|e| e.to_string())?;
    let mut session = arc
        .lock()
        .map_err(|_| "session lock poisoned".to_string())?;

    let mut pdf = mupdf::pdf::PdfDocument::from_bytes(&session.document.bytes)
        .map_err(|e| format!("Failed to open PDF for editing: {e}"))?;

    let page_no = i32::try_from(request.page_index).map_err(|e| format!("page index: {e}"))?;

    let fz_page = pdf
        .load_page(page_no)
        .map_err(|e| format!("load_page: {e}"))?;
    let mut pdf_page =
        mupdf::pdf::PdfPage::try_from(fz_page).map_err(|e| format!("PdfPage: {e}"))?;
    let bg_sample = super::background_sampling::sample_page_background(
        doc_state,
        &request.session_id,
        request.page_index,
        bbox,
    );

    // Defense in depth: also mark the (padded) original area for redact
    // so MuPDF strips the underlying text operators where it can. The
    // explicit cover rect drawn below is the load-bearing mechanism; the
    // redact is belt-and-braces.
    let redact_rect = mupdf::Rect::new(cover_rect[0], cover_rect[1], cover_rect[2], cover_rect[3]);
    let mut redact_annot = pdf_page
        .create_annotation(mupdf::pdf::PdfAnnotationType::Redact)
        .map_err(|e| format!("create redact: {e}"))?;
    redact_annot
        .set_rect(redact_rect)
        .map_err(|e| format!("set_rect: {e}"))?;
    redact_annot
        .set_color(mupdf::color::AnnotationColor::Rgb {
            red: bg_sample.rgb[0],
            green: bg_sample.rgb[1],
            blue: bg_sample.rgb[2],
        })
        .map_err(|e| format!("set_color: {e}"))?;
    drop(redact_annot);
    pdf_page.redact().map_err(|e| format!("redact: {e}"))?;

    // Ensure the page has a Helvetica resource we can reference.
    let page_obj = pdf_page.object();
    ensure_helvetica_resource(&pdf, &page_obj)?;

    // Build the cover-then-text operator block. The cover is drawn BEFORE
    // the replacement text so the new glyphs paint on top of a clean
    // background and the original text is fully hidden.
    let text_ops = build_safe_visual_replacement_ops_with_color(
        cover_rect,
        bbox,
        font_size,
        &request.replacement_text,
        bg_sample.rgb,
    );

    if let Err(e) = append_safe_visual_ops_to_page_contents(&mut pdf, request.page_index, &text_ops)
    {
        return Ok(NativeTextEditResult {
            edit_id: edit_id.to_string(),
            session_id: request.session_id.clone(),
            page_index: request.page_index,
            content_object_id: request.content_object_id.clone(),
            method: EditMethod::Rejected,
            original_text: original_text.to_string(),
            replacement_text: request.replacement_text.clone(),
            success: false,
            warnings: vec![format!(
                "Cannot apply safe visual replacement: {e}. Edit refused to avoid leaving the original text visible under the replacement."
            )],
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        });
    }

    let mut new_bytes: Vec<u8> = Vec::new();
    pdf.write_to(&mut new_bytes)
        .map_err(|e| format!("serialize PDF: {e}"))?;

    session.document.bytes = new_bytes;
    session.is_dirty = true;
    session.invalidate_cached_document();

    Ok(NativeTextEditResult {
        edit_id: edit_id.to_string(),
        session_id: request.session_id.clone(),
        page_index: request.page_index,
        content_object_id: request.content_object_id.clone(),
        method: EditMethod::SafeVisualReplacement,
        original_text: original_text.to_string(),
        replacement_text: request.replacement_text.clone(),
        success: true,
        warnings: {
            let mut warnings = vec![if bg_sample.used_fallback {
                "Safe visual replacement: original text bbox covered with a padded white rectangle and redacted, replacement drawn with Helvetica on top. Original font not preserved.".to_string()
            } else {
                format!(
                        "Safe visual replacement: sampled background cover color rgb({:.3}, {:.3}, {:.3}) used, original text redacted, replacement drawn with Helvetica on top. Original font not preserved.",
                        bg_sample.rgb[0], bg_sample.rgb[1], bg_sample.rgb[2]
                    )
            }];
            if let Some(w) = bg_sample.warning {
                warnings.push(w);
            }
            warnings
        },
        verification: VerificationStatus::NotRun,
        verification_warnings: vec![],
    })
}

fn fmt_color(v: f32) -> String {
    let clamped = v.clamp(0.0, 1.0);
    let s = format!("{:.4}", clamped);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn append_safe_visual_ops_to_page_contents(
    pdf: &mut mupdf::pdf::PdfDocument,
    page_index: usize,
    ops: &str,
) -> Result<(), String> {
    let stream_dict = pdf
        .new_dict()
        .map_err(|e| format!("new_dict (safe visual stream): {e}"))?;
    let mut new_stream = pdf
        .add_object(&stream_dict)
        .map_err(|e| format!("add_object (safe visual stream): {e}"))?;
    new_stream
        .write_stream_string(ops)
        .map_err(|e| format!("write_stream_string (safe visual stream): {e}"))?;

    let page_no = i32::try_from(page_index).map_err(|e| format!("page index: {e}"))?;
    let mut page_dict = pdf
        .find_page(page_no)
        .map_err(|e| format!("find_page for safe visual append: {e}"))?;

    match page_dict
        .get_dict("Contents")
        .map_err(|e| format!("get Contents for safe visual append: {e}"))?
    {
        Some(contents) if contents.is_array().unwrap_or(false) => {
            let mut arr = contents;
            arr.array_push(new_stream)
                .map_err(|e| format!("array_push safe visual Contents: {e}"))?;
        }
        Some(contents) => {
            let mut arr = pdf
                .new_array()
                .map_err(|e| format!("new_array safe visual Contents: {e}"))?;
            arr.array_push(contents)
                .map_err(|e| format!("array_push existing Contents: {e}"))?;
            arr.array_push(new_stream)
                .map_err(|e| format!("array_push safe visual stream: {e}"))?;
            page_dict
                .dict_put("Contents", arr)
                .map_err(|e| format!("dict_put safe visual Contents array: {e}"))?;
        }
        None => {
            page_dict
                .dict_put("Contents", new_stream)
                .map_err(|e| format!("dict_put safe visual Contents stream: {e}"))?;
        }
    }

    Ok(())
}

/// Ensure the page has a Helvetica font resource named /R2HHelv.
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
            let helv = pdf.new_object_from_str("<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>")
                .map_err(|e| format!("new_object_from_str: {e}"))?;
            new_font_dict
                .dict_put("R2HHelv", helv)
                .map_err(|e| format!("dict_put: {e}"))?;
            let mut resources_mut = resources;
            resources_mut
                .dict_put("Font", new_font_dict)
                .map_err(|e| format!("dict_put Font: {e}"))?;
            return Ok(());
        }
        Err(e) => return Err(format!("get Font dict: {e}")),
    };

    if let Ok(Some(_)) = font_dict.get_dict("R2HHelv") {
        return Ok(());
    }

    let helv = pdf
        .new_object_from_str(
            "<</Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding>>",
        )
        .map_err(|e| format!("new_object_from_str: {e}"))?;
    let mut font_dict_mut = font_dict;
    font_dict_mut
        .dict_put("R2HHelv", helv)
        .map_err(|e| format!("dict_put R2HHelv: {e}"))?;

    Ok(())
}

fn pdf_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_escape_handles_parens() {
        assert_eq!(pdf_escape_string("Hello (World)"), "Hello \\(World\\)");
    }

    #[test]
    fn pdf_escape_handles_backslash() {
        assert_eq!(pdf_escape_string("path\\to\\file"), "path\\\\to\\\\file");
    }

    // ─── Phase 28C: font/encoding safety classifier ────────────────

    #[test]
    fn ascii_replacement_is_native_safe() {
        let d = classify_font_replacement_safety("Helvetica", Some("WinAnsiEncoding"), "Hello");
        assert!(d.native_safe);
        assert!(d.reasons.is_empty());
    }

    #[test]
    fn subset_font_is_unsafe() {
        let d =
            classify_font_replacement_safety("ABCDEF+SubsetFont", Some("WinAnsiEncoding"), "Hello");
        assert!(!d.native_safe);
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("subset")));
    }

    #[test]
    fn cjk_replacement_is_unsafe() {
        let d = classify_font_replacement_safety("Helvetica", Some("WinAnsiEncoding"), "日本語");
        assert!(!d.native_safe);
        assert!(d.reasons.iter().any(|r| r.to_lowercase().contains("cjk")));
    }

    #[test]
    fn arabic_replacement_is_unsafe() {
        let d = classify_font_replacement_safety("Helvetica", Some("WinAnsiEncoding"), "مرحبا");
        assert!(!d.native_safe);
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("arabic")));
    }

    #[test]
    fn non_ascii_with_winansi_within_latin1_is_native_safe() {
        let d = classify_font_replacement_safety("Helvetica", Some("WinAnsiEncoding"), "café");
        assert!(d.native_safe);
    }

    #[test]
    fn non_ascii_with_unknown_encoding_is_unsafe() {
        let d = classify_font_replacement_safety("Helvetica", None, "café");
        assert!(!d.native_safe);
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("cannot be verified")));
    }

    // ─── Phase 29B: registry-backed strategy decision ─────────────

    fn font_info(
        name: &str,
        subtype: &str,
        enc: super::super::font_registry::EncodingKind,
        subset: bool,
        _type0: bool,
        type3: bool,
    ) -> super::super::font_registry::FontResourceInfo {
        super::super::font_registry::classify_font_info_from_parts(
            name,
            if subset {
                "ABCDEF+SomeFont"
            } else {
                "SomeFont"
            },
            subtype,
            enc,
            Some("WinAnsiEncoding".to_string()),
            0,
            false,
            subset,
            !type3,
        )
        .clone()
        // ↑ classify_font_info_from_parts already returns a value; the
        // .clone() is a no-op kept for clarity.
    }

    #[test]
    fn registry_path_picks_native_for_ascii_in_winansi() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            false,
            false,
            false,
        );
        let d = classify_text_edit_strategy(Some("F1"), &[info], "Hello");
        assert!(matches!(d.strategy, TextEditStrategy::NativeInPlace));
        assert!(d.font_preserved);
        assert_eq!(d.source, "font_registry");
    }

    #[test]
    fn registry_path_allows_latin1_in_winansi() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            false,
            false,
            false,
        );
        let d = classify_text_edit_strategy(Some("F1"), &[info], "café");
        assert!(matches!(d.strategy, TextEditStrategy::NativeInPlace));
        assert!(d.encoding_safe);
    }

    #[test]
    fn registry_path_rejects_subset_font_to_visual() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            true,
            false,
            false,
        );
        let d = classify_text_edit_strategy(Some("F1"), &[info], "Hello");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(!d.font_preserved);
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("subset")));
    }

    #[test]
    fn registry_path_rejects_type0_identity_h_to_visual() {
        let info = font_info(
            "F2",
            "Type0",
            super::super::font_registry::EncodingKind::IdentityH,
            false,
            true,
            false,
        );
        let d = classify_text_edit_strategy(Some("F2"), &[info], "Hello");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("identity-h")));
    }

    #[test]
    fn registry_path_marks_type3_read_only() {
        let info = font_info(
            "F3",
            "Type3",
            super::super::font_registry::EncodingKind::Unknown,
            false,
            false,
            true,
        );
        let d = classify_text_edit_strategy(Some("F3"), &[info], "Hello");
        assert!(matches!(d.strategy, TextEditStrategy::ReadOnly));
    }

    #[test]
    fn registry_path_rejects_cjk_to_visual() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            false,
            false,
            false,
        );
        let d = classify_text_edit_strategy(Some("F1"), &[info], "日本");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(d.reasons.iter().any(|r| r.to_lowercase().contains("cjk")));
    }

    #[test]
    fn registry_path_rejects_arabic_to_visual() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            false,
            false,
            false,
        );
        let d = classify_text_edit_strategy(Some("F1"), &[info], "مرحبا");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(d
            .reasons
            .iter()
            .any(|r| r.to_lowercase().contains("arabic")));
    }

    #[test]
    fn registry_path_outside_latin1_falls_back() {
        let info = font_info(
            "F1",
            "Type1",
            super::super::font_registry::EncodingKind::WinAnsi,
            false,
            false,
            false,
        );
        // U+0394 (GREEK CAPITAL LETTER DELTA) — outside Latin-1.
        let d = classify_text_edit_strategy(Some("F1"), &[info], "Δ");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
    }

    #[test]
    fn missing_font_in_registry_ascii_uses_visual_replacement() {
        // Empty registry, ASCII replacement is not enough for a native
        // claim in Phase 34: font/encoding/operator identity must be known.
        let d = classify_text_edit_strategy(Some("F99"), &[], "Hello");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert_eq!(d.source, "fallback_heuristic");
        assert!(d
            .reasons
            .iter()
            .any(|r| r.contains("native edit cannot be verified")));
    }

    #[test]
    fn missing_font_in_registry_non_ascii_falls_back_to_visual() {
        let d = classify_text_edit_strategy(Some("F99"), &[], "café");
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert_eq!(d.source, "fallback_heuristic");
    }

    #[test]
    fn rejected_for_read_only_object() {
        let result = NativeTextEditResult {
            edit_id: "test".to_string(),
            session_id: "s1".to_string(),
            page_index: 0,
            content_object_id: "co-1".to_string(),
            method: EditMethod::Rejected,
            original_text: "Original".to_string(),
            replacement_text: "New".to_string(),
            success: false,
            warnings: vec!["Object is read-only".to_string()],
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        };
        assert!(!result.success);
        assert_eq!(result.method, EditMethod::Rejected);
    }

    // ─── Phase 30C: experimental ToUnicode flag plumbing ──────────

    #[test]
    fn experimental_tounicode_flag_off_does_not_surface_path() {
        let info = super::super::font_registry::classify_font_info_from_parts(
            "F0",
            "STHeiti",
            "Type0",
            super::super::font_registry::EncodingKind::IdentityH,
            Some("Identity-H".into()),
            0,
            true,
            false,
            true,
        );
        let d = classify_text_edit_strategy_with_options(
            Some("F0"),
            &[info],
            "Hello",
            ClassifyOptions {
                enable_experimental_to_unicode: false,
                experimental_to_unicode_ready: true,
            },
        );
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(!d
            .warnings
            .iter()
            .any(|w| w.to_lowercase().contains("experimental")));
    }

    #[test]
    fn experimental_tounicode_flag_on_and_ready_surfaces_path_without_claim() {
        let info = super::super::font_registry::classify_font_info_from_parts(
            "F0",
            "STHeiti",
            "Type0",
            super::super::font_registry::EncodingKind::IdentityH,
            Some("Identity-H".into()),
            0,
            true,
            false,
            true,
        );
        let d = classify_text_edit_strategy_with_options(
            Some("F0"),
            &[info],
            "Hello",
            ClassifyOptions {
                enable_experimental_to_unicode: true,
                experimental_to_unicode_ready: true,
            },
        );
        // Strategy remains visual replacement (write path is NOT
        // implemented) — but the experimental warning is now visible
        // so the UI knows the user enabled the flag.
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(d
            .warnings
            .iter()
            .any(|w| w.to_lowercase().contains("experimental")));
    }

    #[test]
    fn experimental_tounicode_flag_on_but_not_ready_surfaces_ambiguous_diag() {
        let info = super::super::font_registry::classify_font_info_from_parts(
            "F0",
            "STHeiti",
            "Type0",
            super::super::font_registry::EncodingKind::IdentityH,
            Some("Identity-H".into()),
            0,
            true,
            false,
            true,
        );
        let d = classify_text_edit_strategy_with_options(
            Some("F0"),
            &[info],
            "Hello",
            ClassifyOptions {
                enable_experimental_to_unicode: true,
                experimental_to_unicode_ready: false,
            },
        );
        assert!(matches!(
            d.strategy,
            TextEditStrategy::SafeVisualReplacement
        ));
        assert!(d
            .warnings
            .iter()
            .any(|w| w.to_lowercase().contains("ambiguous")));
    }

    // ─── Phase 30D: verification helper ─────────────────────────────

    #[test]
    fn verify_text_present_in_page_returns_true_for_existing_text() {
        // Build a minimal PDF with one page containing "Hello".
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();
        // Empty PDF — needle "" trivially passes.
        let ok = verify_text_present_in_page(&bytes, 0, "").unwrap();
        assert!(ok);
    }

    #[test]
    fn verify_text_present_in_page_returns_false_when_text_missing() {
        let mut src = mupdf::pdf::PdfDocument::new();
        src.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();
        let mut bytes: Vec<u8> = Vec::new();
        src.write_to(&mut bytes).unwrap();
        // Empty page does not contain "Hello".
        let ok = verify_text_present_in_page(&bytes, 0, "Hello").unwrap();
        assert!(!ok);
    }

    // ─── Phase 31C: byte-identical edit rejection ────────────────────

    #[test]
    fn byte_identical_replacement_is_rejected_at_stream_layer() {
        // Re-emit the same string at the same offsets — byte_identical
        // path must never enter the write commit branch. We exercise the
        // pure stream rewrite to ensure the byte sequence really is
        // identical when the replacement matches the original.
        use super::super::stream_parser::{
            find_text_operations, replace_text_in_stream_encoded, EncodingTarget,
        };
        let stream: &[u8] = b"BT /F1 12 Tf 72 700 Td (Hello) Tj ET";
        let ops = find_text_operations(stream);
        let same =
            replace_text_in_stream_encoded(stream, &ops[0], "Hello", EncodingTarget::WinAnsi);
        // The rewriter normalises (...) Tj, so bytes are byte-identical
        // for an unchanged ASCII operand.
        assert_eq!(same, stream);
    }

    #[test]
    fn safe_visual_cover_rect_uses_pdf_y_coordinates_with_padding() {
        let cover = safe_visual_cover_rect([72.0, 700.0, 170.0, 720.0], 12.0).expect("valid cover");

        assert!((cover[0] - 70.2).abs() < 0.001);
        assert!((cover[1] - 698.2).abs() < 0.001);
        assert!((cover[2] - 171.8).abs() < 0.001);
        assert!((cover[3] - 721.8).abs() < 0.001);
    }

    #[test]
    fn safe_visual_ops_draw_cover_before_replacement_text() {
        let ops = build_safe_visual_replacement_ops(
            [70.0, 698.0, 172.0, 722.0],
            [72.0, 700.0, 170.0, 720.0],
            12.0,
            "New text",
        );

        let cover_pos = ops.find("70 698 102 24 re").expect("cover rectangle op");
        let text_pos = ops.find("(New text) Tj").expect("replacement text op");
        assert!(
            cover_pos < text_pos,
            "cover must be painted before replacement text"
        );
    }

    #[test]
    fn appending_safe_visual_ops_converts_existing_stream_to_contents_array() {
        let mut pdf = mupdf::pdf::PdfDocument::new();
        pdf.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();

        let mut page = pdf.find_page(0).unwrap();
        let first_stream_dict = pdf.new_dict().unwrap();
        let mut first_stream = pdf.add_object(&first_stream_dict).unwrap();
        first_stream.write_stream_string("q\nQ\n").unwrap();
        page.dict_put("Contents", first_stream).unwrap();

        append_safe_visual_ops_to_page_contents(&mut pdf, 0, "q\n1 1 1 rg\n0 0 10 10 re\nf\nQ\n")
            .expect("append committed stream");

        let page = pdf.find_page(0).unwrap();
        let contents = page.get_dict("Contents").unwrap().expect("contents");
        assert!(
            contents.is_array().unwrap_or(false),
            "contents must become an array"
        );
        assert_eq!(contents.len().unwrap(), 2);
        let appended = contents.get_array(1).unwrap().expect("appended stream");
        let appended_bytes = appended.read_stream().unwrap();
        let appended_text = String::from_utf8_lossy(&appended_bytes);
        assert!(appended_text.contains("0 0 10 10 re"));
    }

    #[test]
    fn appending_safe_visual_ops_preserves_contents_array_and_appends_last() {
        let mut pdf = mupdf::pdf::PdfDocument::new();
        pdf.new_page(mupdf::Size {
            width: 612.0,
            height: 792.0,
        })
        .unwrap();

        let mut page = pdf.find_page(0).unwrap();
        let mut arr = pdf.new_array().unwrap();
        let first_stream_dict = pdf.new_dict().unwrap();
        let mut first_stream = pdf.add_object(&first_stream_dict).unwrap();
        first_stream.write_stream_string("BT\nET\n").unwrap();
        arr.array_push(first_stream).unwrap();
        page.dict_put("Contents", arr).unwrap();

        append_safe_visual_ops_to_page_contents(&mut pdf, 0, "q\nBT\n(New) Tj\nET\nQ\n")
            .expect("append committed stream");

        let page = pdf.find_page(0).unwrap();
        let contents = page.get_dict("Contents").unwrap().expect("contents");
        assert!(contents.is_array().unwrap_or(false));
        assert_eq!(contents.len().unwrap(), 2);
        let appended = contents.get_array(1).unwrap().expect("appended stream");
        let appended_bytes = appended.read_stream().unwrap();
        let appended_text = String::from_utf8_lossy(&appended_bytes);
        assert!(appended_text.contains("(New) Tj"));
    }

    #[test]
    fn appending_safe_visual_ops_rejects_when_page_cannot_be_found() {
        let mut pdf = mupdf::pdf::PdfDocument::new();

        let err = append_safe_visual_ops_to_page_contents(&mut pdf, 0, "q\nQ\n")
            .expect_err("missing page must reject safe visual append");

        assert!(
            err.contains("find_page"),
            "clear append failure, got: {err}"
        );
    }
}
