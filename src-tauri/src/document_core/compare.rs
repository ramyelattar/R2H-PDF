//! Phase 23E + Phase 24B/E: document compare / redline.
//!
//! - 23E: text-only LCS line diff per page.
//! - 24E: OCR-aware text source per page (native_text | ocr_text | mixed | none).
//! - 24B: visual page-diff foundation — pixel difference at low DPI with
//!        connected-region bbox extraction; runs alongside text in `combined`
//!        mode.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareMode {
    TextOnly,
    PageVisual,
    Combined,
}

impl Default for CompareMode {
    fn default() -> Self { Self::TextOnly }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDocumentsRequest {
    pub base_session_id: String,
    /// Either an open session id or an absolute path to a PDF on disk.
    /// If both are provided, `revised_session_id` wins.
    pub revised_session_id: Option<String>,
    pub revised_file_path: Option<String>,
    #[serde(default)]
    pub mode: CompareMode,
    pub max_pages: Option<usize>,
    /// Phase 24E: when true (default), include cached OCR text for pages
    /// that lack native text. When false, scanned pages contribute no
    /// text and are reported via a warning.
    #[serde(default = "default_include_ocr")]
    pub include_ocr: bool,
    /// Phase 24B: cap the number of pages submitted to visual diff. Each
    /// page rendered at low DPI is comparatively expensive, so the
    /// frontend can request a smaller cap to keep the UI responsive on
    /// large documents.
    pub visual_max_pages: Option<usize>,
    /// Phase 24B: render DPI for visual diff. Defaults to 72.
    pub visual_dpi: Option<u32>,
    /// Phase 25A: when true (and `include_ocr` is also true), the compare
    /// engine will auto-run OCR on scanned pages whose cache is empty so
    /// the text diff can see them. Capped by `auto_ocr_max_pages`.
    #[serde(default = "default_auto_ocr_scanned")]
    pub auto_ocr_scanned: bool,
    /// Phase 25A: hard cap on the number of pages auto-OCR will process
    /// per compare call. Defaults to 25.
    pub auto_ocr_max_pages: Option<usize>,
}

fn default_include_ocr() -> bool { true }
fn default_auto_ocr_scanned() -> bool { false }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareDocumentInfo {
    pub session_id: Option<String>,
    pub source_path: Option<String>,
    pub page_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareSummary {
    pub pages_compared: usize,
    pub pages_with_changes: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
    pub lines_modified: usize,
    pub identical: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareTextChange {
    pub page_index: usize,
    pub change_type: ChangeType,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub bbox: Option<[f32; 4]>,
    pub confidence: f32,
    pub citation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareVisualChange {
    pub page_index: usize,
    pub change_type: ChangeType,
    pub bbox: [f32; 4],
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Removed,
    Modified,
    /// Phase 24B: a region of the rendered page that differs visually
    /// between base and revised (used for visual_changes entries).
    VisualModified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparePageResult {
    pub page_index: usize,
    pub base_line_count: usize,
    pub revised_line_count: usize,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    /// Phase 24E: indicates which text source(s) were used for this page.
    /// `native_text` | `ocr_text` | `mixed` | `none` (pre-existing field-less
    /// callers will see `native_text` by default).
    #[serde(default = "default_text_source")]
    pub text_source: String,
    /// Phase 24B: number of visual change regions found on this page.
    #[serde(default)]
    pub visual_regions: usize,
}

fn default_text_source() -> String { "native_text".to_string() }

/// Phase 24E: per-page text source provided to the diff engine, so each
/// page result can declare whether the lines came from native extraction,
/// OCR cache, or both.
///
/// Phase 25A: optional `lines` field carries per-line bboxes so the diff
/// can attach pixel-accurate rects to change rows. When absent, the engine
/// falls back to splitting `text` on newlines and reports None bbox.
#[derive(Debug, Clone)]
pub struct PageTextWithSource {
    pub text: String,
    pub source: PageTextSource,
    pub lines: Option<Vec<super::types::LineBbox>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageTextSource {
    NativeText,
    OcrText,
    Mixed,
    None,
}

impl PageTextSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NativeText => "native_text",
            Self::OcrText => "ocr_text",
            Self::Mixed => "mixed",
            Self::None => "none",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    pub compare_id: String,
    pub base_document: CompareDocumentInfo,
    pub revised_document: CompareDocumentInfo,
    pub mode: CompareMode,
    pub summary: CompareSummary,
    pub page_results: Vec<ComparePageResult>,
    pub text_changes: Vec<CompareTextChange>,
    pub visual_changes: Vec<CompareVisualChange>,
    pub warnings: Vec<String>,
    pub created_at_ms: u128,
}

/// In-memory store of completed compare runs. Keyed by `compare_id`.
pub struct CompareResultStore {
    inner: Mutex<HashMap<String, CompareResult>>,
}

impl CompareResultStore {
    pub fn new() -> Self {
        Self { inner: Mutex::new(HashMap::new()) }
    }

    pub fn insert(&self, result: CompareResult) -> Result<(), String> {
        let mut g = self.inner.lock().map_err(|_| "compare store lock poisoned".to_string())?;
        g.insert(result.compare_id.clone(), result);
        Ok(())
    }

    pub fn get(&self, compare_id: &str) -> Result<Option<CompareResult>, String> {
        let g = self.inner.lock().map_err(|_| "compare store lock poisoned".to_string())?;
        Ok(g.get(compare_id).cloned())
    }

    pub fn remove(&self, compare_id: &str) -> Result<bool, String> {
        let mut g = self.inner.lock().map_err(|_| "compare store lock poisoned".to_string())?;
        Ok(g.remove(compare_id).is_some())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }
}

#[derive(Clone)]
pub struct CompareState {
    pub store: std::sync::Arc<CompareResultStore>,
}

impl CompareState {
    pub fn new() -> Self {
        Self { store: std::sync::Arc::new(CompareResultStore::new()) }
    }
}

// ------------------- Diff core -------------------

/// Run a per-page line diff between `base_pages` and `revised_pages`.
/// `max_pages` caps the number of pages compared (defaults to the smaller
/// of the two page counts; trailing pages on the longer side are reported
/// as fully added/removed without an LCS pass to avoid quadratic blowups
/// on documents with very different lengths).
pub fn run_text_compare(
    base_pages: &[String],
    revised_pages: &[String],
    max_pages: Option<usize>,
    base_info: CompareDocumentInfo,
    revised_info: CompareDocumentInfo,
    mode: CompareMode,
) -> CompareResult {
    // Adapt strings into PageTextWithSource defaulting to NativeText.
    let base: Vec<PageTextWithSource> = base_pages.iter().map(|t| PageTextWithSource {
        text: t.clone(), source: PageTextSource::NativeText, lines: None,
    }).collect();
    let revised: Vec<PageTextWithSource> = revised_pages.iter().map(|t| PageTextWithSource {
        text: t.clone(), source: PageTextSource::NativeText, lines: None,
    }).collect();
    run_text_compare_with_sources(&base, &revised, max_pages, base_info, revised_info, mode)
}

/// Phase 24E: same as `run_text_compare` but each page carries its text
/// source (native / OCR / mixed / none). Source is mirrored into the
/// per-page result so the frontend can warn users which pages relied on
/// OCR — and which had no text at all.
pub fn run_text_compare_with_sources(
    base_pages: &[PageTextWithSource],
    revised_pages: &[PageTextWithSource],
    max_pages: Option<usize>,
    base_info: CompareDocumentInfo,
    revised_info: CompareDocumentInfo,
    mode: CompareMode,
) -> CompareResult {
    let mut warnings = Vec::new();

    let max_compare = max_pages
        .unwrap_or(usize::MAX)
        .min(base_pages.len().max(revised_pages.len()));

    let mut page_results = Vec::new();
    let mut text_changes = Vec::new();
    let mut lines_added = 0;
    let mut lines_removed = 0;
    let mut lines_modified = 0;
    let mut pages_with_changes = 0;

    for page_idx in 0..max_compare {
        let base_page = base_pages.get(page_idx);
        let revised_page = revised_pages.get(page_idx);

        // Phase 25A: prefer the per-line structured view when available — it
        // carries bboxes. Otherwise fall back to splitting `text` by lines
        // and emit changes with no bbox.
        let base_line_texts: Vec<String> = match base_page.and_then(|p| p.lines.as_ref()) {
            Some(lines) => lines.iter().map(|l| l.text.clone()).collect(),
            None => base_page
                .map(|p| p.text.lines().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        };
        let revised_line_texts: Vec<String> = match revised_page.and_then(|p| p.lines.as_ref()) {
            Some(lines) => lines.iter().map(|l| l.text.clone()).collect(),
            None => revised_page
                .map(|p| p.text.lines().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
        };

        let text_source = combine_sources(
            base_page.map(|p| &p.source),
            revised_page.map(|p| &p.source),
        );

        if text_source == "none" {
            warnings.push(format!(
                "Page {} appears scanned and has no native or OCR text — text changes for this page were skipped.",
                page_idx + 1
            ));
        }

        let base_str_refs: Vec<&str> = base_line_texts.iter().map(|s| s.as_str()).collect();
        let revised_str_refs: Vec<&str> = revised_line_texts.iter().map(|s| s.as_str()).collect();
        let diff = diff_lines_indexed(&base_str_refs, &revised_str_refs);

        let page_added = diff.iter().filter(|d| matches!(d, IdxDiffOp::Add { .. })).count();
        let page_removed = diff.iter().filter(|d| matches!(d, IdxDiffOp::Remove { .. })).count();

        let base_bboxes = base_page.and_then(|p| p.lines.as_ref()).map(|v| v.as_slice());
        let revised_bboxes = revised_page.and_then(|p| p.lines.as_ref()).map(|v| v.as_slice());

        let (changes_for_page, page_modified) = collapse_modifications_indexed(
            page_idx, &diff, base_bboxes, revised_bboxes,
        );
        text_changes.extend(changes_for_page);

        let net_added = page_added.saturating_sub(page_modified);
        let net_removed = page_removed.saturating_sub(page_modified);

        lines_added += net_added;
        lines_removed += net_removed;
        lines_modified += page_modified;

        if net_added + net_removed + page_modified > 0 { pages_with_changes += 1; }

        page_results.push(ComparePageResult {
            page_index: page_idx,
            base_line_count: base_line_texts.len(),
            revised_line_count: revised_line_texts.len(),
            added: net_added,
            removed: net_removed,
            modified: page_modified,
            text_source,
            visual_regions: 0,
        });
    }

    let identical = lines_added == 0 && lines_removed == 0 && lines_modified == 0;

    let compare_id = format!(
        "cmp-{}-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0),
        next_compare_seq()
    );

    CompareResult {
        compare_id,
        base_document: base_info,
        revised_document: revised_info,
        mode,
        summary: CompareSummary {
            pages_compared: max_compare,
            pages_with_changes,
            lines_added,
            lines_removed,
            lines_modified,
            identical,
        },
        page_results,
        text_changes,
        visual_changes: Vec::new(),
        warnings,
        created_at_ms: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum DiffOp<'a> {
    Equal(&'a str),
    Add(&'a str),
    Remove(&'a str),
}

/// Phase 25A: indexed diff carries the source-side line index so callers
/// can look up bboxes after the LCS pass.
#[derive(Debug, PartialEq, Eq)]
enum IdxDiffOp<'a> {
    Equal { text: &'a str },
    Add { text: &'a str, revised_idx: usize },
    Remove { text: &'a str, base_idx: usize },
}

/// Hunt–McIlroy / Myers-style line diff using a longest-common-subsequence
/// table. Suitable for page-sized text (typical pages: <500 lines).
fn diff_lines<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<DiffOp<'a>> {
    let n = a.len();
    let m = b.len();
    // dp[i][j] = length of LCS of a[..i] and b[..j]
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if normalize(a[i]) == normalize(b[j]) {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut ops = Vec::with_capacity(n + m);
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        if normalize(a[i - 1]) == normalize(b[j - 1]) {
            ops.push(DiffOp::Equal(a[i - 1]));
            i -= 1; j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            ops.push(DiffOp::Remove(a[i - 1]));
            i -= 1;
        } else {
            ops.push(DiffOp::Add(b[j - 1]));
            j -= 1;
        }
    }
    while i > 0 { ops.push(DiffOp::Remove(a[i - 1])); i -= 1; }
    while j > 0 { ops.push(DiffOp::Add(b[j - 1])); j -= 1; }
    ops.reverse();
    ops
}

fn normalize(s: &str) -> String {
    s.trim().to_string()
}

/// Phase 25A: same algorithm as `diff_lines` but the Add/Remove variants
/// carry the source-side line index so the caller can fetch the matching
/// `LineBbox` after the diff. Equal entries don't need an index.
fn diff_lines_indexed<'a>(a: &[&'a str], b: &[&'a str]) -> Vec<IdxDiffOp<'a>> {
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in 0..n {
        for j in 0..m {
            dp[i + 1][j + 1] = if normalize(a[i]) == normalize(b[j]) {
                dp[i][j] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut ops = Vec::with_capacity(n + m);
    let mut i = n;
    let mut j = m;
    while i > 0 && j > 0 {
        if normalize(a[i - 1]) == normalize(b[j - 1]) {
            ops.push(IdxDiffOp::Equal { text: a[i - 1] });
            i -= 1; j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            ops.push(IdxDiffOp::Remove { text: a[i - 1], base_idx: i - 1 });
            i -= 1;
        } else {
            ops.push(IdxDiffOp::Add { text: b[j - 1], revised_idx: j - 1 });
            j -= 1;
        }
    }
    while i > 0 { ops.push(IdxDiffOp::Remove { text: a[i - 1], base_idx: i - 1 }); i -= 1; }
    while j > 0 { ops.push(IdxDiffOp::Add { text: b[j - 1], revised_idx: j - 1 }); j -= 1; }
    ops.reverse();
    ops
}

/// Phase 25A: variant of `collapse_modifications` that pulls bboxes from
/// the provided per-side `LineBbox` arrays. For Removed/Modified rows it
/// uses the base-side bbox (the bbox the reviewer is viewing). Added rows
/// get the revised-side bbox if available (lower priority for
/// annotation overlay placement, but useful in reports).
fn collapse_modifications_indexed(
    page_index: usize,
    ops: &[IdxDiffOp<'_>],
    base_lines: Option<&[super::types::LineBbox]>,
    revised_lines: Option<&[super::types::LineBbox]>,
) -> (Vec<CompareTextChange>, usize) {
    let mut out = Vec::new();
    let mut modified = 0usize;
    let mut i = 0;
    while i < ops.len() {
        // Detect Modified pair (Remove+Add or Add+Remove with high similarity).
        let pair = match (&ops[i], ops.get(i + 1)) {
            (IdxDiffOp::Remove { text: o, base_idx }, Some(IdxDiffOp::Add { text: n, revised_idx })) =>
                Some((*o, *base_idx, *n, *revised_idx)),
            (IdxDiffOp::Add { text: n, revised_idx }, Some(IdxDiffOp::Remove { text: o, base_idx })) =>
                Some((*o, *base_idx, *n, *revised_idx)),
            _ => None,
        };
        if let Some((old, base_idx, new_text, _rev_idx)) = pair {
            let sim = line_similarity(old, new_text);
            if sim >= 0.4 {
                let bbox = base_lines.and_then(|v| v.get(base_idx)).map(|l| l.bbox);
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Modified,
                    old_text: Some(old.to_string()),
                    new_text: Some(new_text.to_string()),
                    bbox,
                    confidence: sim,
                    citation: format!("page {}", page_index + 1),
                });
                modified += 1;
                i += 2;
                continue;
            }
        }
        match &ops[i] {
            IdxDiffOp::Equal { .. } => { i += 1; }
            IdxDiffOp::Remove { text: old, base_idx } => {
                let bbox = base_lines.and_then(|v| v.get(*base_idx)).map(|l| l.bbox);
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Removed,
                    old_text: Some(old.to_string()),
                    new_text: None,
                    bbox,
                    confidence: 1.0,
                    citation: format!("page {}", page_index + 1),
                });
                i += 1;
            }
            IdxDiffOp::Add { text: new_text, revised_idx } => {
                let bbox = revised_lines.and_then(|v| v.get(*revised_idx)).map(|l| l.bbox);
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Added,
                    old_text: None,
                    new_text: Some(new_text.to_string()),
                    bbox,
                    confidence: 1.0,
                    citation: format!("page {}", page_index + 1),
                });
                i += 1;
            }
        }
    }
    (out, modified)
}

/// Collapse adjacent Remove+Add (or Add+Remove) into a single Modified change
/// when the two lines are similar (share at least 40% tokens). Returns the
/// list of resulting changes and the count of modified pairs detected.
fn collapse_modifications(page_index: usize, ops: &[DiffOp<'_>]) -> (Vec<CompareTextChange>, usize) {
    let mut out = Vec::new();
    let mut modified = 0usize;
    let mut i = 0;
    while i < ops.len() {
        // Try to detect a Modified pair regardless of whether Remove or Add
        // appears first — Myers/LCS can produce either order depending on
        // how ties are broken.
        let pair: Option<(&str, &str)> = match (&ops[i], ops.get(i + 1)) {
            (DiffOp::Remove(o), Some(DiffOp::Add(n))) => Some((o, n)),
            (DiffOp::Add(n), Some(DiffOp::Remove(o))) => Some((o, n)),
            _ => None,
        };
        if let Some((old, new_text)) = pair {
            let sim = line_similarity(old, new_text);
            if sim >= 0.4 {
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Modified,
                    old_text: Some(old.to_string()),
                    new_text: Some(new_text.to_string()),
                    bbox: None,
                    confidence: sim,
                    citation: format!("page {}", page_index + 1),
                });
                modified += 1;
                i += 2;
                continue;
            }
        }
        match &ops[i] {
            DiffOp::Equal(_) => { i += 1; }
            DiffOp::Remove(old) => {
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Removed,
                    old_text: Some(old.to_string()),
                    new_text: None,
                    bbox: None,
                    confidence: 1.0,
                    citation: format!("page {}", page_index + 1),
                });
                i += 1;
            }
            DiffOp::Add(new_text) => {
                out.push(CompareTextChange {
                    page_index,
                    change_type: ChangeType::Added,
                    old_text: None,
                    new_text: Some(new_text.to_string()),
                    bbox: None,
                    confidence: 1.0,
                    citation: format!("page {}", page_index + 1),
                });
                i += 1;
            }
        }
    }
    (out, modified)
}

fn line_similarity(a: &str, b: &str) -> f32 {
    let an: Vec<&str> = a.split_whitespace().collect();
    let bn: Vec<&str> = b.split_whitespace().collect();
    if an.is_empty() && bn.is_empty() { return 1.0; }
    if an.is_empty() || bn.is_empty() { return 0.0; }
    let set_a: std::collections::HashSet<&&str> = an.iter().collect();
    let common = bn.iter().filter(|t| set_a.contains(t)).count() as f32;
    let denom = an.len().max(bn.len()) as f32;
    if denom == 0.0 { 0.0 } else { common / denom }
}

static COMPARE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn next_compare_seq() -> u64 {
    COMPARE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ------------------- 24C HTML redline report -------------------

/// Build a self-contained HTML redline report for a `CompareResult`.
/// All styling is inlined so the report works fully offline — there are
/// no external script, stylesheet, font, or image URLs. All user-supplied
/// text is HTML-escaped to avoid injection from the document content.
pub fn build_redline_html_report(r: &CompareResult) -> String {
    let title = "PDF Redline Report";
    let mut html = String::new();
    html.push_str("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str(&format!("<title>{}</title>", esc(title)));
    html.push_str("<style>\
body{font:14px/1.5 system-ui,sans-serif;max-width:900px;margin:24px auto;color:#222;background:#fafafa;}\
h1{font-size:22px;margin-bottom:4px;}\
h2{font-size:18px;border-bottom:1px solid #ddd;padding-bottom:4px;margin-top:24px;}\
table{border-collapse:collapse;width:100%;margin:8px 0;}\
th,td{border:1px solid #ccc;padding:6px 8px;text-align:left;vertical-align:top;}\
th{background:#eef;}\
.added{color:#080;background:#e8ffe8;}\
.removed{color:#a00;background:#ffe8e8;}\
.modified{color:#a60;background:#fff7e0;}\
.visual_modified{color:#369;background:#e8eeff;}\
.warning{background:#fff3cd;border:1px solid #f6c75a;padding:6px 8px;margin:4px 0;border-radius:4px;}\
.muted{color:#666;}\
code{font-family:Menlo,Consolas,monospace;font-size:12px;}\
</style></head><body>");

    html.push_str(&format!("<h1>{}</h1>", esc(title)));
    html.push_str(&format!(
        "<p class=\"muted\">Generated for compare <code>{}</code> at epoch ms {}</p>",
        esc(&r.compare_id), r.created_at_ms,
    ));

    // Documents.
    html.push_str("<h2>Documents</h2>");
    html.push_str("<table><tr><th>Side</th><th>Source</th><th>Pages</th></tr>");
    html.push_str(&format!(
        "<tr><td>Base</td><td>{}</td><td>{}</td></tr>",
        esc(&doc_info_label(&r.base_document)), r.base_document.page_count,
    ));
    html.push_str(&format!(
        "<tr><td>Revised</td><td>{}</td><td>{}</td></tr>",
        esc(&doc_info_label(&r.revised_document)), r.revised_document.page_count,
    ));
    html.push_str("</table>");

    // Summary.
    html.push_str("<h2>Summary</h2>");
    html.push_str("<table><tr><th>Metric</th><th>Value</th></tr>");
    html.push_str(&format!("<tr><td>Mode</td><td>{:?}</td></tr>", r.mode));
    html.push_str(&format!("<tr><td>Pages compared</td><td>{}</td></tr>", r.summary.pages_compared));
    html.push_str(&format!("<tr><td>Pages with changes</td><td>{}</td></tr>", r.summary.pages_with_changes));
    html.push_str(&format!("<tr><td class=\"added\">Lines added</td><td>{}</td></tr>", r.summary.lines_added));
    html.push_str(&format!("<tr><td class=\"removed\">Lines removed</td><td>{}</td></tr>", r.summary.lines_removed));
    html.push_str(&format!("<tr><td class=\"modified\">Lines modified</td><td>{}</td></tr>", r.summary.lines_modified));
    html.push_str(&format!("<tr><td class=\"visual_modified\">Visual regions</td><td>{}</td></tr>", r.visual_changes.len()));
    html.push_str(&format!("<tr><td>Identical?</td><td>{}</td></tr>", r.summary.identical));
    html.push_str("</table>");

    // Warnings.
    if !r.warnings.is_empty() {
        html.push_str("<h2>Warnings</h2>");
        for w in &r.warnings {
            html.push_str(&format!("<div class=\"warning\">{}</div>", esc(w)));
        }
    }

    // Per-page sources (24E).
    html.push_str("<h2>Per-page Detail</h2>");
    html.push_str("<table><tr><th>Page</th><th>Source</th><th>+</th><th>−</th><th>~</th><th>Visual</th></tr>");
    for pr in &r.page_results {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td class=\"added\">{}</td><td class=\"removed\">{}</td><td class=\"modified\">{}</td><td class=\"visual_modified\">{}</td></tr>",
            pr.page_index + 1, esc(&pr.text_source), pr.added, pr.removed, pr.modified, pr.visual_regions,
        ));
    }
    html.push_str("</table>");

    // Text changes.
    html.push_str("<h2>Text Changes</h2>");
    if r.text_changes.is_empty() {
        html.push_str("<p class=\"muted\">No text changes.</p>");
    } else {
        html.push_str("<table><tr><th>Page</th><th>Type</th><th>Old</th><th>New</th><th>Citation</th></tr>");
        for c in &r.text_changes {
            let ct = match c.change_type {
                ChangeType::Added => "added",
                ChangeType::Removed => "removed",
                ChangeType::Modified => "modified",
                ChangeType::VisualModified => "visual_modified",
            };
            html.push_str(&format!(
                "<tr class=\"{class}\"><td>{page}</td><td>{ctype}</td><td>{old}</td><td>{new}</td><td>{cite}</td></tr>",
                class = ct,
                page = c.page_index + 1,
                ctype = ct,
                old = esc(c.old_text.as_deref().unwrap_or("")),
                new = esc(c.new_text.as_deref().unwrap_or("")),
                cite = esc(&c.citation),
            ));
        }
        html.push_str("</table>");
    }

    // Visual changes.
    html.push_str("<h2>Visual Changes</h2>");
    if r.visual_changes.is_empty() {
        html.push_str("<p class=\"muted\">No visual changes.</p>");
    } else {
        html.push_str("<table><tr><th>Page</th><th>Type</th><th>BBox (pts)</th><th>Confidence</th></tr>");
        for v in &r.visual_changes {
            let [x0, y0, x1, y1] = v.bbox;
            let ctype = match v.change_type {
                ChangeType::Added => "added",
                ChangeType::Removed => "removed",
                ChangeType::Modified => "modified",
                ChangeType::VisualModified => "visual_modified",
            };
            html.push_str(&format!(
                "<tr class=\"visual_modified\"><td>{}</td><td>{}</td><td><code>[{:.1}, {:.1}, {:.1}, {:.1}]</code></td><td>{:.2}</td></tr>",
                v.page_index + 1, ctype, x0, y0, x1, y1, v.confidence,
            ));
        }
        html.push_str("</table>");
    }

    html.push_str("</body></html>");
    html
}

fn doc_info_label(info: &CompareDocumentInfo) -> String {
    if let Some(p) = &info.source_path { return p.clone(); }
    if let Some(s) = &info.session_id { return format!("session: {s}"); }
    "unknown".to_string()
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 16);
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

// ------------------- 24E helpers -------------------

/// Reduce two `PageTextSource` labels into a single per-page string.
pub fn combine_sources(
    a: Option<&PageTextSource>,
    b: Option<&PageTextSource>,
) -> String {
    use PageTextSource as P;
    let a_val = a.map(|s| s.clone()).unwrap_or(P::None);
    let b_val = b.map(|s| s.clone()).unwrap_or(P::None);

    match (a_val, b_val) {
        (P::None, P::None) => "none".into(),
        (P::None, other) | (other, P::None) => other.as_str().into(),
        (P::NativeText, P::NativeText) => "native_text".into(),
        (P::OcrText, P::OcrText) => "ocr_text".into(),
        (P::Mixed, _) | (_, P::Mixed) => "mixed".into(),
        // Cross-source pairing.
        (P::NativeText, P::OcrText) | (P::OcrText, P::NativeText) => "mixed".into(),
    }
}

// ------------------- 24B visual diff -------------------

/// A single rendered page expressed as RGBA pixels and explicit dimensions.
/// Both base and revised pages must be rendered at the same dimensions.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub page_index: usize,
    pub width_px: u32,
    pub height_px: u32,
    pub page_width_pts: f32,
    pub page_height_pts: f32,
    pub pixels_rgba: Vec<u8>,
}

/// Run a visual diff for the given page pair. Returns:
///   - a list of CompareVisualChange entries (page-space bboxes in PDF
///     points, not pixels)
///   - the number of distinct change regions found.
///
/// Strategy: divide the page into a fixed cell grid (default 32×32). For
/// each cell, sum the per-channel absolute difference. A cell is marked
/// "changed" when the difference per pixel exceeds `pixel_threshold`. We
/// then merge adjacent changed cells via 4-connected flood-fill and emit
/// one bbox per region.
///
/// Pixel-perfect bbox extraction would be more accurate but also far more
/// expensive; this grid approach gives reviewers usable hotspots while
/// keeping a low DPI render cheap.
pub fn run_visual_page_diff(
    base: &RenderedPage,
    revised: &RenderedPage,
    cell_grid: u32,
    pixel_threshold: u32,
) -> (Vec<CompareVisualChange>, usize) {
    if base.width_px == 0 || base.height_px == 0
        || revised.width_px == 0 || revised.height_px == 0
        || base.pixels_rgba.is_empty() || revised.pixels_rgba.is_empty()
    {
        return (Vec::new(), 0);
    }

    let w = base.width_px.min(revised.width_px) as usize;
    let h = base.height_px.min(revised.height_px) as usize;
    let cells = cell_grid.max(4).min(128) as usize;
    let cell_w = (w as f32 / cells as f32).ceil() as usize;
    let cell_h = (h as f32 / cells as f32).ceil() as usize;
    let stride_a = base.width_px as usize * 4;
    let stride_b = revised.width_px as usize * 4;

    // grid[cx][cy] = true when the cell has visible difference.
    let mut grid = vec![vec![false; cells]; cells];

    for cy in 0..cells {
        for cx in 0..cells {
            let x0 = cx * cell_w;
            let y0 = cy * cell_h;
            let x1 = (x0 + cell_w).min(w);
            let y1 = (y0 + cell_h).min(h);
            if x0 >= x1 || y0 >= y1 { continue; }
            let mut diff_sum: u64 = 0;
            let mut pixel_count: u64 = 0;
            for y in y0..y1 {
                let row_a = y * stride_a;
                let row_b = y * stride_b;
                for x in x0..x1 {
                    let i_a = row_a + x * 4;
                    let i_b = row_b + x * 4;
                    if i_a + 2 >= base.pixels_rgba.len() || i_b + 2 >= revised.pixels_rgba.len() {
                        continue;
                    }
                    let dr = (base.pixels_rgba[i_a] as i32 - revised.pixels_rgba[i_b] as i32).unsigned_abs() as u64;
                    let dg = (base.pixels_rgba[i_a + 1] as i32 - revised.pixels_rgba[i_b + 1] as i32).unsigned_abs() as u64;
                    let db = (base.pixels_rgba[i_a + 2] as i32 - revised.pixels_rgba[i_b + 2] as i32).unsigned_abs() as u64;
                    diff_sum += dr + dg + db;
                    pixel_count += 1;
                }
            }
            if pixel_count > 0 {
                let avg_per_pixel = (diff_sum / pixel_count) as u32;
                if avg_per_pixel >= pixel_threshold {
                    grid[cx][cy] = true;
                }
            }
        }
    }

    // 4-connected flood fill to merge adjacent changed cells into regions.
    let mut visited = vec![vec![false; cells]; cells];
    let mut regions: Vec<(usize, usize, usize, usize, u32)> = Vec::new();
    for cy in 0..cells {
        for cx in 0..cells {
            if !grid[cx][cy] || visited[cx][cy] { continue; }
            let (mut min_x, mut min_y) = (cx, cy);
            let (mut max_x, mut max_y) = (cx, cy);
            let mut cell_count: u32 = 0;
            let mut stack = vec![(cx, cy)];
            while let Some((x, y)) = stack.pop() {
                if visited[x][y] || !grid[x][y] { continue; }
                visited[x][y] = true;
                cell_count += 1;
                if x < min_x { min_x = x; }
                if x > max_x { max_x = x; }
                if y < min_y { min_y = y; }
                if y > max_y { max_y = y; }
                if x > 0 { stack.push((x - 1, y)); }
                if x + 1 < cells { stack.push((x + 1, y)); }
                if y > 0 { stack.push((x, y - 1)); }
                if y + 1 < cells { stack.push((x, y + 1)); }
            }
            regions.push((min_x, min_y, max_x, max_y, cell_count));
        }
    }

    // Convert each region back to PDF-point space.
    let px_per_pt_x = base.width_px as f32 / base.page_width_pts.max(1.0);
    let px_per_pt_y = base.height_px as f32 / base.page_height_pts.max(1.0);
    let mut out = Vec::with_capacity(regions.len());
    for (rx0, ry0, rx1, ry1, count) in &regions {
        let px_x0 = (*rx0 * cell_w) as f32;
        let px_y0 = (*ry0 * cell_h) as f32;
        let px_x1 = ((*rx1 + 1) * cell_w).min(w) as f32;
        let px_y1 = ((*ry1 + 1) * cell_h).min(h) as f32;
        // PDF coords have origin bottom-left; rendered pixels have origin
        // top-left. Flip Y.
        let pdf_x0 = px_x0 / px_per_pt_x;
        let pdf_x1 = px_x1 / px_per_pt_x;
        let pdf_y_top = px_y0 / px_per_pt_y;
        let pdf_y_bot = px_y1 / px_per_pt_y;
        let pdf_y0 = base.page_height_pts - pdf_y_bot;
        let pdf_y1 = base.page_height_pts - pdf_y_top;
        // Confidence: more cells = higher confidence (cap at 1.0).
        let confidence = ((*count as f32) / 8.0).min(1.0).max(0.1);
        out.push(CompareVisualChange {
            page_index: base.page_index,
            change_type: ChangeType::VisualModified,
            bbox: [pdf_x0, pdf_y0, pdf_x1, pdf_y1],
            confidence,
        });
    }
    let count = out.len();
    (out, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(path: &str, pages: usize) -> CompareDocumentInfo {
        CompareDocumentInfo {
            session_id: None,
            source_path: Some(path.to_string()),
            page_count: pages,
        }
    }

    #[test]
    fn identical_pages_yield_no_changes() {
        let base = vec!["alpha\nbeta\ngamma".to_string()];
        let rev = vec!["alpha\nbeta\ngamma".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::TextOnly);
        assert!(r.summary.identical);
        assert_eq!(r.summary.lines_added, 0);
        assert_eq!(r.summary.lines_removed, 0);
        assert_eq!(r.summary.lines_modified, 0);
        assert!(r.text_changes.is_empty());
    }

    #[test]
    fn detects_added_line() {
        let base = vec!["alpha\nbeta".to_string()];
        let rev = vec!["alpha\nbeta\ngamma".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::TextOnly);
        assert!(!r.summary.identical);
        assert_eq!(r.summary.lines_added, 1);
        assert_eq!(r.summary.lines_removed, 0);
        assert!(r.text_changes.iter().any(|c| matches!(c.change_type, ChangeType::Added) && c.new_text.as_deref() == Some("gamma")));
    }

    #[test]
    fn detects_removed_line() {
        let base = vec!["alpha\nbeta\ngamma".to_string()];
        let rev = vec!["alpha\ngamma".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::TextOnly);
        assert_eq!(r.summary.lines_removed, 1);
        assert!(r.text_changes.iter().any(|c| matches!(c.change_type, ChangeType::Removed) && c.old_text.as_deref() == Some("beta")));
    }

    #[test]
    fn detects_modified_line_via_similarity() {
        // "The quick brown fox" -> "The quick brown cat" — high token overlap
        // should collapse to a Modified change.
        let base = vec!["The quick brown fox".to_string()];
        let rev = vec!["The quick brown cat".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::TextOnly);
        assert_eq!(r.summary.lines_modified, 1, "expected modified line, got {:?}", r);
        assert!(r.text_changes.iter().any(|c| matches!(c.change_type, ChangeType::Modified)));
    }

    #[test]
    fn no_unconditional_visual_warning_when_text_only_runs_in_visual_mode() {
        // 24B: visual diff is run separately by the IPC layer; the text
        // engine no longer emits a misleading "visual not implemented"
        // warning regardless of mode.
        let base = vec!["a".to_string()];
        let rev = vec!["a".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::PageVisual);
        assert!(
            !r.warnings.iter().any(|w| w.to_lowercase().contains("visual diff was skipped")),
            "expected text engine to be quiet about visual mode now, got {:?}", r.warnings,
        );
    }

    #[test]
    fn page_results_default_text_source_native_text() {
        let base = vec!["a".to_string()];
        let rev = vec!["b".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 1), CompareMode::TextOnly);
        assert_eq!(r.page_results[0].text_source, "native_text");
    }

    #[test]
    fn ocr_text_source_is_tracked_per_page() {
        // 24E: when the base side uses native text and the revised side
        // only has OCR text, the per-page label is "mixed".
        let base = vec![PageTextWithSource { text: "hello world".into(), source: PageTextSource::NativeText, lines: None }];
        let rev = vec![PageTextWithSource { text: "hello universe".into(), source: PageTextSource::OcrText, lines: None }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        assert_eq!(r.page_results[0].text_source, "mixed");
    }

    #[test]
    fn no_text_on_either_side_emits_warning() {
        // 24E: scanned page with no OCR fallback should produce a clear
        // warning rather than silently emitting "no changes".
        let base = vec![PageTextWithSource { text: "".into(), source: PageTextSource::None, lines: None }];
        let rev = vec![PageTextWithSource { text: "".into(), source: PageTextSource::None, lines: None }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        assert_eq!(r.page_results[0].text_source, "none");
        assert!(r.warnings.iter().any(|w| w.contains("scanned")),
            "expected scanned-page warning, got {:?}", r.warnings);
    }

    #[test]
    fn combine_sources_handles_all_combinations() {
        use PageTextSource as P;
        assert_eq!(combine_sources(None, None), "none");
        assert_eq!(combine_sources(Some(&P::NativeText), None), "native_text");
        assert_eq!(combine_sources(None, Some(&P::OcrText)), "ocr_text");
        assert_eq!(combine_sources(Some(&P::NativeText), Some(&P::OcrText)), "mixed");
        assert_eq!(combine_sources(Some(&P::OcrText), Some(&P::OcrText)), "ocr_text");
        assert_eq!(combine_sources(Some(&P::Mixed), Some(&P::NativeText)), "mixed");
    }

    // -------- Visual diff (24B) --------

    fn rgba_solid(w: u32, h: u32, r: u8, g: u8, b: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..(w * h) {
            v.push(r); v.push(g); v.push(b); v.push(255);
        }
        v
    }

    fn rendered(idx: usize, w: u32, h: u32, pixels: Vec<u8>) -> RenderedPage {
        RenderedPage {
            page_index: idx,
            width_px: w, height_px: h,
            page_width_pts: 612.0, page_height_pts: 792.0,
            pixels_rgba: pixels,
        }
    }

    #[test]
    fn identical_rendered_pages_produce_no_visual_changes() {
        let base = rendered(0, 64, 64, rgba_solid(64, 64, 200, 200, 200));
        let rev  = rendered(0, 64, 64, rgba_solid(64, 64, 200, 200, 200));
        let (changes, count) = run_visual_page_diff(&base, &rev, 16, 5);
        assert_eq!(count, 0);
        assert!(changes.is_empty());
    }

    #[test]
    fn entirely_different_rendered_pages_produce_at_least_one_region() {
        let base = rendered(0, 64, 64, rgba_solid(64, 64, 0, 0, 0));
        let rev  = rendered(0, 64, 64, rgba_solid(64, 64, 255, 255, 255));
        let (changes, count) = run_visual_page_diff(&base, &rev, 16, 5);
        assert!(count >= 1, "expected at least one region, got {count}");
        assert!(changes.iter().all(|c| matches!(c.change_type, ChangeType::VisualModified)));
    }

    #[test]
    fn local_visual_change_returns_localized_bbox() {
        // Solid white page with a 16x16 black square in the top-left of the
        // rendered output. The change region should be in the upper portion
        // of the PDF (high Y in PDF coordinates).
        let w = 64u32; let h = 64u32;
        let mut base = rgba_solid(w, h, 255, 255, 255);
        for y in 0..16 {
            for x in 0..16 {
                let i = ((y * w + x) * 4) as usize;
                base[i] = 0; base[i + 1] = 0; base[i + 2] = 0;
            }
        }
        let rev_clean = rgba_solid(w, h, 255, 255, 255);
        let (changes, count) = run_visual_page_diff(
            &rendered(0, w, h, base),
            &rendered(0, w, h, rev_clean),
            16, 5,
        );
        assert!(count >= 1);
        // Pixel y0=0..16 maps to top of rendered image; in PDF coords that's
        // near the top of the page (y close to page_height_pts).
        let bbox = changes[0].bbox;
        let page_h = 792.0_f32;
        assert!(bbox[3] > page_h * 0.5, "expected change in upper half of PDF, got {:?}", bbox);
    }

    #[test]
    fn empty_pixel_buffers_yield_no_visual_changes() {
        let base = RenderedPage { page_index: 0, width_px: 0, height_px: 0, page_width_pts: 612.0, page_height_pts: 792.0, pixels_rgba: vec![] };
        let rev  = base.clone();
        let (changes, count) = run_visual_page_diff(&base, &rev, 16, 5);
        assert_eq!(count, 0);
        assert!(changes.is_empty());
    }

    #[test]
    fn store_round_trip() {
        let store = CompareResultStore::new();
        let r = run_text_compare(&vec![], &vec![], None, info("a", 0), info("b", 0), CompareMode::TextOnly);
        let id = r.compare_id.clone();
        store.insert(r).unwrap();
        assert!(store.get(&id).unwrap().is_some());
        assert!(store.remove(&id).unwrap());
        assert!(store.get(&id).unwrap().is_none());
    }

    #[test]
    fn extra_pages_on_revised_count_as_added() {
        let base = vec!["a".to_string()];
        let rev = vec!["a".to_string(), "x\ny\nz".to_string()];
        let r = run_text_compare(&base, &rev, None, info("a", 1), info("b", 2), CompareMode::TextOnly);
        assert_eq!(r.summary.pages_compared, 2);
        assert!(r.summary.lines_added >= 3, "expected ≥3 added lines, got {:?}", r.summary);
    }

    // -------- Redline HTML report (24C) --------

    fn synth_result_for_report(text_changes: Vec<CompareTextChange>, visual: Vec<CompareVisualChange>) -> CompareResult {
        CompareResult {
            compare_id: "cmp-test".into(),
            base_document: info("base.pdf", 1),
            revised_document: info("revised.pdf", 1),
            mode: CompareMode::Combined,
            summary: CompareSummary {
                pages_compared: 1, pages_with_changes: 1,
                lines_added: 1, lines_removed: 1, lines_modified: 0, identical: false,
            },
            page_results: vec![ComparePageResult {
                page_index: 0, base_line_count: 2, revised_line_count: 2,
                added: 1, removed: 1, modified: 0,
                text_source: "native_text".into(), visual_regions: visual.len(),
            }],
            text_changes,
            visual_changes: visual,
            warnings: vec!["sample warning".into()],
            created_at_ms: 0,
        }
    }

    #[test]
    fn redline_html_report_includes_summary_and_changes() {
        let r = synth_result_for_report(
            vec![
                CompareTextChange { page_index: 0, change_type: ChangeType::Added, old_text: None, new_text: Some("plus".into()), bbox: None, confidence: 1.0, citation: "page 1".into() },
                CompareTextChange { page_index: 0, change_type: ChangeType::Removed, old_text: Some("minus".into()), new_text: None, bbox: None, confidence: 1.0, citation: "page 1".into() },
            ],
            vec![CompareVisualChange { page_index: 0, change_type: ChangeType::VisualModified, bbox: [10.0, 10.0, 20.0, 20.0], confidence: 0.7 }],
        );
        let html = build_redline_html_report(&r);
        assert!(html.contains("<title>") && html.contains("Redline Report"));
        assert!(html.contains("plus"));
        assert!(html.contains("minus"));
        assert!(html.contains("visual_modified"));
        assert!(html.contains("sample warning"));
    }

    #[test]
    fn redline_html_report_escapes_change_text() {
        let r = synth_result_for_report(
            vec![CompareTextChange {
                page_index: 0, change_type: ChangeType::Added,
                old_text: None, new_text: Some("<script>alert(1)</script>".into()),
                bbox: None, confidence: 1.0, citation: "page 1".into(),
            }],
            vec![],
        );
        let html = build_redline_html_report(&r);
        assert!(!html.contains("<script>alert(1)</script>"),
            "raw script tag leaked into report: {html}");
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn redline_html_report_has_no_external_urls() {
        let r = synth_result_for_report(vec![], vec![]);
        let html = build_redline_html_report(&r);
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    // -------- Phase 25A: per-line bbox propagates into CompareTextChange --------

    use super::super::types::LineBbox;

    fn line(text: &str, bbox: [f32; 4]) -> LineBbox {
        LineBbox { text: text.to_string(), bbox }
    }

    #[test]
    fn removed_line_carries_base_side_bbox() {
        let base = vec![PageTextWithSource {
            text: "alpha\nbeta\ngamma".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![
                line("alpha", [10.0, 700.0, 60.0, 712.0]),
                line("beta", [10.0, 685.0, 50.0, 697.0]),
                line("gamma", [10.0, 670.0, 70.0, 682.0]),
            ]),
        }];
        let rev = vec![PageTextWithSource {
            text: "alpha\ngamma".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![
                line("alpha", [10.0, 700.0, 60.0, 712.0]),
                line("gamma", [10.0, 685.0, 70.0, 697.0]),
            ]),
        }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        let removed = r.text_changes.iter().find(|c| matches!(c.change_type, ChangeType::Removed)).unwrap();
        assert_eq!(removed.old_text.as_deref(), Some("beta"));
        assert_eq!(removed.bbox, Some([10.0, 685.0, 50.0, 697.0]),
            "expected base-side bbox of 'beta' attached to removed change");
    }

    #[test]
    fn added_line_carries_revised_side_bbox() {
        let base = vec![PageTextWithSource {
            text: "alpha".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![ line("alpha", [10.0, 700.0, 60.0, 712.0]) ]),
        }];
        let rev = vec![PageTextWithSource {
            text: "alpha\nbeta".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![
                line("alpha", [10.0, 700.0, 60.0, 712.0]),
                line("beta",  [10.0, 685.0, 50.0, 697.0]),
            ]),
        }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        let added = r.text_changes.iter().find(|c| matches!(c.change_type, ChangeType::Added)).unwrap();
        assert_eq!(added.new_text.as_deref(), Some("beta"));
        assert_eq!(added.bbox, Some([10.0, 685.0, 50.0, 697.0]),
            "expected revised-side bbox of 'beta' attached to added change");
    }

    #[test]
    fn modified_line_uses_base_side_bbox() {
        let base = vec![PageTextWithSource {
            text: "The quick brown fox".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![ line("The quick brown fox", [10.0, 700.0, 200.0, 715.0]) ]),
        }];
        let rev = vec![PageTextWithSource {
            text: "The quick brown cat".into(),
            source: PageTextSource::NativeText,
            lines: Some(vec![ line("The quick brown cat", [10.0, 700.0, 200.0, 715.0]) ]),
        }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        let modified = r.text_changes.iter().find(|c| matches!(c.change_type, ChangeType::Modified)).unwrap();
        assert_eq!(modified.bbox, Some([10.0, 700.0, 200.0, 715.0]));
        assert!(modified.confidence > 0.5);
    }

    #[test]
    fn changes_without_lines_still_emit_with_none_bbox() {
        // Backwards-compat: when neither side supplies lines, the diff still
        // works and emits bbox=None entries.
        let base = vec![PageTextWithSource { text: "a".into(), source: PageTextSource::NativeText, lines: None }];
        let rev = vec![PageTextWithSource { text: "b".into(), source: PageTextSource::NativeText, lines: None }];
        let r = run_text_compare_with_sources(
            &base, &rev, None,
            info("a", 1), info("b", 1),
            CompareMode::TextOnly,
        );
        assert!(r.text_changes.iter().all(|c| c.bbox.is_none()));
    }
}
