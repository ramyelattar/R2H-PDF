//! Find/replace preview and apply for decoded content objects.

use super::analysis::extract_page_content_objects;
use super::text_edit::apply_native_text_edit;
use super::types::*;
use crate::document_core::DocumentCoreState;

pub fn preview_find_replace(
    doc_state: &DocumentCoreState,
    request: &FindReplacePreviewRequest,
) -> Result<FindReplacePreviewResult, String> {
    if request.find_text.is_empty() {
        return Ok(FindReplacePreviewResult {
            session_id: request.session_id.clone(),
            matches: vec![],
            unsafe_count: 0,
            warnings: vec!["Find text is empty.".to_string()],
        });
    }
    let pages = pages_for_scope(doc_state, request)?;
    let mut matches = Vec::new();
    let mut unsafe_count = 0usize;
    for page_index in pages {
        let objects = extract_page_content_objects(doc_state, &request.session_id, page_index)?;
        for obj in objects.into_iter().filter(|o| {
            matches!(
                o.object_type,
                ContentObjectType::TextSpan | ContentObjectType::TextBlock
            )
        }) {
            let Some(info) = obj.text_info.as_ref() else {
                continue;
            };
            if !text_matches_find(
                &info.decoded_text,
                &request.find_text,
                request.case_sensitive,
                request.whole_word,
            ) {
                continue;
            }
            let (safe, method, reason) = classify_match(&obj);
            if !safe {
                unsafe_count += 1;
            }
            let id = format!("fr-{}-{}-{}", page_index, obj.id, matches.len());
            matches.push(FindReplaceMatchPreview {
                id,
                page_index,
                content_object_id: obj.id,
                text_preview: preview_text(
                    &info.decoded_text,
                    &request.find_text,
                    request.case_sensitive,
                ),
                bbox: obj.bbox,
                editable_status: if safe {
                    "safe".to_string()
                } else {
                    "not_safely_editable".to_string()
                },
                replacement_method: method,
                safe,
                reason,
                checked: safe,
            });
        }
    }
    Ok(FindReplacePreviewResult {
        session_id: request.session_id.clone(),
        matches,
        unsafe_count,
        warnings: vec![],
    })
}

pub fn apply_find_replace_item(
    doc_state: &DocumentCoreState,
    session_id: &str,
    replace_text: &str,
    item: &FindReplaceApplyItem,
) -> Result<NativeTextEditResult, String> {
    let objects = extract_page_content_objects(doc_state, session_id, item.page_index)?;
    let target = objects
        .iter()
        .find(|o| o.id == item.content_object_id)
        .ok_or_else(|| {
            format!(
                "Find/replace target no longer exists: {}",
                item.content_object_id
            )
        })?;
    let (_safe, _method, reason) = classify_match(target);
    if let Some(reason) = reason {
        return Ok(NativeTextEditResult {
            edit_id: format!("find-replace-rejected-{}", item.id),
            session_id: session_id.to_string(),
            page_index: item.page_index,
            content_object_id: item.content_object_id.clone(),
            method: EditMethod::Rejected,
            original_text: target
                .text_info
                .as_ref()
                .map(|t| t.decoded_text.clone())
                .unwrap_or_default(),
            replacement_text: replace_text.to_string(),
            success: false,
            warnings: vec![reason],
            verification: VerificationStatus::NotRun,
            verification_warnings: vec![],
        });
    }
    apply_native_text_edit(
        doc_state,
        &NativeTextEditRequest {
            session_id: session_id.to_string(),
            page_index: item.page_index,
            content_object_id: item.content_object_id.clone(),
            replacement_text: replace_text.to_string(),
            preserve_style: true,
        },
    )
}

fn pages_for_scope(
    doc_state: &DocumentCoreState,
    request: &FindReplacePreviewRequest,
) -> Result<Vec<usize>, String> {
    match request.scope {
        FindReplaceScope::CurrentPage => Ok(vec![request.current_page_index]),
        FindReplaceScope::WholeDocument => {
            let arc = doc_state
                .store
                .get_session_arc_pub(&request.session_id)
                .map_err(|e| e.to_string())?;
            let session = arc
                .lock()
                .map_err(|_| "session lock poisoned".to_string())?;
            Ok((0..session.document.pages.len()).collect())
        }
    }
}

fn classify_match(obj: &ContentObject) -> (bool, String, Option<String>) {
    let Some(info) = obj.text_info.as_ref() else {
        return (
            false,
            "Not safely editable".to_string(),
            Some("Text metadata is missing.".to_string()),
        );
    };
    if info.decoding_quality == "garbled"
        || !info.encoding_safe && info.decoded_text.contains('\u{fffd}')
    {
        return (
            false,
            "Not safely editable".to_string(),
            Some("Text encoding could not be decoded safely.".to_string()),
        );
    }
    if obj.editable_level == EditableLevel::ReadOnly {
        return (
            false,
            "Not safely editable".to_string(),
            Some("Text object is read-only.".to_string()),
        );
    }
    if info.editable_strategy == "native_in_place"
        || obj.editable_level == EditableLevel::NativeEditable
    {
        return (true, "Native text edit".to_string(), None);
    }
    (true, "Visual replacement".to_string(), None)
}

fn text_matches_find(text: &str, find: &str, case_sensitive: bool, whole_word: bool) -> bool {
    let hay = if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    };
    let needle = if case_sensitive {
        find.to_string()
    } else {
        find.to_lowercase()
    };
    if !whole_word {
        return hay.contains(&needle);
    }
    find_occurrences(&hay, &needle).into_iter().any(|idx| {
        let before = hay[..idx].chars().next_back();
        let after = hay[idx + needle.len()..].chars().next();
        !before.map(is_word_char).unwrap_or(false) && !after.map(is_word_char).unwrap_or(false)
    })
}

fn find_occurrences(hay: &str, needle: &str) -> Vec<usize> {
    if needle.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    while let Some(pos) = hay[start..].find(needle) {
        let idx = start + pos;
        out.push(idx);
        start = idx + needle.len();
    }
    out
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn preview_text(text: &str, find: &str, case_sensitive: bool) -> String {
    let hay = if case_sensitive {
        text.to_string()
    } else {
        text.to_lowercase()
    };
    let needle = if case_sensitive {
        find.to_string()
    } else {
        find.to_lowercase()
    };
    let idx = hay.find(&needle).unwrap_or(0);
    let start = text[..idx]
        .char_indices()
        .rev()
        .nth(24)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let end = text[idx..]
        .char_indices()
        .nth(find.chars().count() + 24)
        .map(|(i, _)| idx + i)
        .unwrap_or(text.len());
    text[start..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_sensitive_and_whole_word_matching_work() {
        assert!(text_matches_find("Panel A", "Panel", true, true));
        assert!(!text_matches_find("panel A", "Panel", true, true));
        assert!(text_matches_find("panel A", "Panel", false, true));
        assert!(!text_matches_find("panelboard", "panel", false, true));
    }

    #[test]
    fn garbled_match_is_not_safe() {
        let obj = ContentObject {
            id: "t1".into(),
            session_id: "s1".into(),
            page_index: 0,
            object_type: ContentObjectType::TextSpan,
            bbox: [0.0, 0.0, 10.0, 10.0],
            z_index: 0,
            editable_level: EditableLevel::NativeEditable,
            text_info: Some(TextInfo {
                raw_text: "\u{fffd}".into(),
                decoded_text: "\u{fffd}\u{fffd}".into(),
                glyph_count: 2,
                font_name: "F1".into(),
                font_size: 12.0,
                fill_color: None,
                stroke_color: None,
                writing_mode: "horizontal".into(),
                is_subset_font: false,
                encoding_safe: false,
                operator_offset: None,
                operator_length: None,
                occurrence_index: 0,
                operator_type: "tj".into(),
                content_stream_index: Some(0),
                operator_index: Some(0),
                editable_strategy: "native_in_place".into(),
                unsupported_reason: vec![],
                font_encoding: None,
                decoding_quality: "garbled".into(),
            }),
            image_info: None,
            style_info: None,
            diagnostics: vec![],
        };
        let (safe, method, reason) = classify_match(&obj);
        assert!(!safe);
        assert_eq!(method, "Not safely editable");
        assert_eq!(
            reason.as_deref(),
            Some("Text encoding could not be decoded safely.")
        );
    }
}
