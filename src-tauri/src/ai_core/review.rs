//! Document review: uses RAG + engineering findings to produce a structured review.

use std::time::Instant;
use serde::{Deserialize, Serialize};

use super::local_runtime::LocalRuntime;
use super::local_types::LocalGenerateRequest;
use super::rag::{Citation, RagEngine};
use super::vector_index::VectorSearchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReviewRequest {
    pub session_id: String,
    pub include_engineering: bool,
    pub include_ocr: bool,
    pub max_findings: Option<usize>,
    pub suggest_actions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub finding_id: String,
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub page_refs: Vec<usize>,
    pub citations: Vec<Citation>,
    pub recommendation: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSuggestedAction {
    pub action_id: String,
    pub action_type: String,
    pub page_index: usize,
    pub text: String,
    pub reason: String,
    pub citations: Vec<Citation>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentReviewResult {
    pub review_id: String,
    pub session_id: String,
    pub summary: String,
    pub findings: Vec<ReviewFinding>,
    pub risks: Vec<ReviewFinding>,
    pub missing_information: Vec<ReviewFinding>,
    pub engineering_findings: Vec<ReviewFinding>,
    pub suggested_actions: Vec<ReviewSuggestedAction>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    pub elapsed_ms: u64,
    pub created_at: u128,
}

const SAFE_ACTION_TYPES: &[&str] = &["add_comment", "add_highlight", "add_text_box", "add_redaction", "apply_stamp"];

/// Run a general document review using RAG context and local LLM.
pub fn run_review(
    request: &DocumentReviewRequest,
    rag_engine: &RagEngine,
    runtime: &LocalRuntime,
    embedding_runtime: Option<&super::embedding::EmbeddingRuntime>,
    engineering_findings: Vec<crate::engineering::types::EngineeringFinding>,
) -> Result<DocumentReviewResult, String> {
    let start = Instant::now();
    let mut warnings: Vec<String> = Vec::new();

    // 1. Retrieve broad document context.
    let queries = &[
        "key terms risks obligations deadlines scope",
        "important clauses conditions requirements specifications",
        "missing information unclear scope incomplete data",
    ];

    let mut all_results: Vec<VectorSearchResult> = Vec::new();
    for query in queries {
        let qe = embedding_runtime.and_then(|e| if e.is_server_ready() { e.embed_query(query).ok() } else { None });
        let results = rag_engine.search(&request.session_id, query, 5, qe.as_deref());
        all_results.extend(results);
    }

    // Deduplicate by chunk_id.
    let mut seen = std::collections::HashSet::new();
    all_results.retain(|r| seen.insert(r.chunk_id.clone()));

    if all_results.is_empty() {
        return Ok(DocumentReviewResult {
            review_id: gen_id("review"),
            session_id: request.session_id.clone(),
            summary: "Not enough document evidence was found to complete a reliable review.".to_string(),
            findings: vec![], risks: vec![], missing_information: vec![],
            engineering_findings: vec![], suggested_actions: vec![],
            citations: vec![],
            warnings: vec!["No relevant document chunks found. Build the RAG index first.".to_string()],
            elapsed_ms: start.elapsed().as_millis() as u64,
            created_at: epoch_ms(),
        });
    }

    // 2. Build citations.
    let citations: Vec<Citation> = all_results.iter().enumerate().map(|(i, r)| Citation {
        citation_id: format!("review-cite-{}", i + 1),
        page_index: r.page_index,
        chunk_id: r.chunk_id.clone(),
        snippet: r.text.chars().take(200).collect(),
        score: r.score,
        source: r.source.clone(),
    }).collect();

    // 3. Build context for LLM.
    let context: String = all_results.iter().take(8)
        .map(|r| format!("[Page {}] {}", r.page_index + 1, r.text.chars().take(400).collect::<String>()))
        .collect::<Vec<_>>().join("\n\n");

    // 4. Generate review with local LLM.
    let default_model = runtime.registry.default_model()
        .ok_or_else(|| "No default LLM model configured".to_string())?;

    let prompt = format!(
        "Document context:\n---\n{}\n---\n\n\
         Analyze this document and provide a JSON review with:\n\
         1. \"summary\": A 2-3 sentence overview.\n\
         2. \"findings\": Array of {{\"title\": \"...\", \"description\": \"...\", \"severity\": \"info|warning|major\", \"category\": \"general|technical|risk|missing_information\", \"page\": N, \"recommendation\": \"...\"}}\n\
         3. \"suggested_actions\": Array of {{\"action_type\": \"add_comment|add_highlight\", \"page\": N, \"text\": \"...\", \"reason\": \"...\"}}\n\n\
         Return ONLY valid JSON. No other text.",
        context
    );

    let gen_result = runtime.generate(LocalGenerateRequest {
        model_id: default_model.id,
        prompt,
        system_prompt: Some("You are a document reviewer. Output ONLY valid JSON. Be concise and factual. Only reference pages from the provided context.".to_string()),
        max_tokens: Some(1024),
        temperature: Some(0.3),
        top_p: Some(0.9),
        timeout_ms: Some(120_000),
    })?;

    // 5. Parse LLM output.
    let (summary, mut findings, suggested_actions) = parse_review_output(&gen_result.text, &citations, &mut warnings);

    // 6. Categorize findings.
    let mut risks: Vec<ReviewFinding> = Vec::new();
    let mut missing_info: Vec<ReviewFinding> = Vec::new();
    let mut general_findings: Vec<ReviewFinding> = Vec::new();

    for f in findings.drain(..) {
        match f.category.as_str() {
            "risk" => risks.push(f),
            "missing_information" => missing_info.push(f),
            _ => general_findings.push(f),
        }
    }

    // 7. Include engineering findings.
    let eng_findings: Vec<ReviewFinding> = engineering_findings.iter().map(|ef| ReviewFinding {
        finding_id: ef.finding_id.clone(),
        title: ef.title.clone(),
        description: ef.description.clone(),
        severity: match ef.severity { crate::engineering::types::FindingSeverity::Info => "info", crate::engineering::types::FindingSeverity::Warning => "warning", crate::engineering::types::FindingSeverity::Major => "major", crate::engineering::types::FindingSeverity::Critical => "critical" }.to_string(),
        category: "engineering".to_string(),
        page_refs: ef.page_refs.clone(),
        citations: vec![],
        recommendation: ef.recommendation.clone(),
        confidence: ef.confidence,
    }).collect();

    // 8. Filter suggested actions for safety.
    let safe_actions: Vec<ReviewSuggestedAction> = suggested_actions.into_iter()
        .filter(|a| SAFE_ACTION_TYPES.contains(&a.action_type.as_str()))
        .collect();

    let max = request.max_findings.unwrap_or(20);

    Ok(DocumentReviewResult {
        review_id: gen_id("review"),
        session_id: request.session_id.clone(),
        summary,
        findings: general_findings.into_iter().take(max).collect(),
        risks: risks.into_iter().take(max).collect(),
        missing_information: missing_info.into_iter().take(max).collect(),
        engineering_findings: eng_findings,
        suggested_actions: safe_actions,
        citations,
        warnings,
        elapsed_ms: start.elapsed().as_millis() as u64,
        created_at: epoch_ms(),
    })
}

fn parse_review_output(output: &str, citations: &[Citation], warnings: &mut Vec<String>) -> (String, Vec<ReviewFinding>, Vec<ReviewSuggestedAction>) {
    let json_str = extract_json(output);
    let Some(json_str) = json_str else {
        warnings.push("LLM output did not contain valid JSON. Using raw text as summary.".to_string());
        return (output.trim().chars().take(500).collect(), vec![], vec![]);
    };

    let parsed: serde_json::Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(format!("Failed to parse review JSON: {e}"));
            return (output.trim().chars().take(500).collect(), vec![], vec![]);
        }
    };

    let summary = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string();

    let mut findings = Vec::new();
    if let Some(arr) = parsed.get("findings").and_then(|v| v.as_array()) {
        for (i, item) in arr.iter().enumerate() {
            let page = item.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let page_index = page.saturating_sub(1);
            let page_citations: Vec<Citation> = citations.iter().filter(|c| c.page_index == page_index).cloned().collect();

            findings.push(ReviewFinding {
                finding_id: format!("rf-{}", i),
                title: item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                description: item.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                severity: item.get("severity").and_then(|v| v.as_str()).unwrap_or("info").to_string(),
                category: item.get("category").and_then(|v| v.as_str()).unwrap_or("general").to_string(),
                page_refs: vec![page_index],
                citations: page_citations,
                recommendation: item.get("recommendation").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                confidence: 0.7,
            });
        }
    }

    let mut actions = Vec::new();
    if let Some(arr) = parsed.get("suggested_actions").and_then(|v| v.as_array()) {
        for (i, item) in arr.iter().enumerate() {
            let page = item.get("page").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let page_index = page.saturating_sub(1);
            let page_citations: Vec<Citation> = citations.iter().filter(|c| c.page_index == page_index).cloned().collect();

            let action_type = item.get("action_type").and_then(|v| v.as_str()).unwrap_or("add_comment").to_string();
            if !SAFE_ACTION_TYPES.contains(&action_type.as_str()) { continue; }

            actions.push(ReviewSuggestedAction {
                action_id: format!("rsa-{}", i),
                action_type,
                page_index,
                text: item.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                reason: item.get("reason").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                citations: page_citations,
                confidence: 0.7,
            });
        }
    }

    (summary, findings, actions)
}

// ---------- Phase 24D: AI review of a compare result ----------

/// Build a prompt summarising a `CompareResult` for the local LLM.
/// Caps the number of changes per category and truncates long lines so
/// the prompt fits comfortably in a 4k–8k context window.
pub fn build_compare_review_prompt(
    base_label: &str,
    revised_label: &str,
    summary_line: &str,
    text_changes: &[(String, usize, Option<String>, Option<String>)],
    visual_change_count: usize,
    warnings: &[String],
) -> String {
    let mut prompt = String::new();
    prompt.push_str(&format!(
        "Base document: {base_label}\nRevised document: {revised_label}\n\nSummary: {summary_line}\n\n"
    ));

    if !warnings.is_empty() {
        prompt.push_str("Warnings emitted by the compare engine:\n");
        for w in warnings.iter().take(5) {
            prompt.push_str(&format!("- {w}\n"));
        }
        prompt.push('\n');
    }

    prompt.push_str("Text changes (max 40 shown):\n");
    for (ctype, page_one_based, old, new_) in text_changes.iter().take(40) {
        let o = old.as_deref().unwrap_or("").chars().take(160).collect::<String>();
        let n = new_.as_deref().unwrap_or("").chars().take(160).collect::<String>();
        prompt.push_str(&format!("- [{ctype}] page {page_one_based} OLD={o:?} NEW={n:?}\n"));
    }
    if text_changes.len() > 40 {
        prompt.push_str(&format!("...and {} more text changes\n", text_changes.len() - 40));
    }
    prompt.push_str(&format!("\nVisual modified regions: {visual_change_count}\n\n"));

    prompt.push_str(
        "Produce a structured JSON review with:\n\
         1. \"summary\": 2–3 sentences capturing the most important differences between base and revised.\n\
         2. \"findings\": array of {\"title\", \"description\", \"severity\": \"info|warning|major\", \"category\": \"risk|missing_information|general\", \"page\": N, \"recommendation\"}\n\
         3. \"suggested_actions\": array of {\"action_type\": \"add_comment\"|\"add_highlight\", \"page\": N, \"text\", \"reason\"}\n\n\
         Return ONLY valid JSON, no markdown fences."
    );
    prompt
}

/// Run an AI review of a compare result using the local LLM. Returns a
/// `DocumentReviewResult` shaped exactly like a regular document review,
/// so the existing frontend "apply suggested actions" pipeline can be
/// reused without changes.
pub fn run_compare_review(
    session_id: &str,
    base_label: &str,
    revised_label: &str,
    summary_line: &str,
    text_changes: &[(String, usize, Option<String>, Option<String>)],
    visual_change_count: usize,
    compare_warnings: &[String],
    runtime: &LocalRuntime,
) -> Result<DocumentReviewResult, String> {
    let start = Instant::now();
    let mut warnings: Vec<String> = Vec::new();

    let prompt = build_compare_review_prompt(
        base_label, revised_label, summary_line,
        text_changes, visual_change_count, compare_warnings,
    );

    let default_model = runtime.registry.default_model()
        .ok_or_else(|| "No default local AI model configured. Open Models tab to install one.".to_string())?;

    let gen_result = runtime.generate(LocalGenerateRequest {
        model_id: default_model.id,
        prompt,
        system_prompt: Some(
            "You are a careful reviewer comparing two revisions of the same PDF. \
             Only reference pages mentioned in the input. Output ONLY valid JSON."
                .to_string()
        ),
        max_tokens: Some(1024),
        temperature: Some(0.2),
        top_p: Some(0.9),
        timeout_ms: Some(120_000),
    })?;

    let (summary, mut findings, suggested_actions) =
        parse_review_output(&gen_result.text, &[], &mut warnings);

    let mut risks = Vec::new();
    let mut missing_info = Vec::new();
    let mut general_findings = Vec::new();
    for f in findings.drain(..) {
        match f.category.as_str() {
            "risk" => risks.push(f),
            "missing_information" => missing_info.push(f),
            _ => general_findings.push(f),
        }
    }

    let safe_actions: Vec<ReviewSuggestedAction> = suggested_actions
        .into_iter()
        .filter(|a| SAFE_ACTION_TYPES.contains(&a.action_type.as_str()))
        .collect();

    Ok(DocumentReviewResult {
        review_id: gen_id("compare-review"),
        session_id: session_id.to_string(),
        summary,
        findings: general_findings,
        risks,
        missing_information: missing_info,
        engineering_findings: vec![],
        suggested_actions: safe_actions,
        citations: vec![],
        warnings,
        elapsed_ms: start.elapsed().as_millis() as u64,
        created_at: epoch_ms(),
    })
}

fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start { Some(&text[start..=end]) } else { None }
}

fn gen_id(prefix: &str) -> String {
    format!("{}-{}", prefix, epoch_ms())
}

fn epoch_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_evidence_returns_clear_message() {
        // When no chunks exist, review should return insufficient evidence.
        let rag = RagEngine::new();
        let runtime = LocalRuntime::new();
        let req = DocumentReviewRequest {
            session_id: "s1".to_string(), include_engineering: false,
            include_ocr: false, max_findings: None, suggest_actions: true,
        };
        let result = run_review(&req, &rag, &runtime, None, vec![]);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert!(r.summary.contains("Not enough"));
        assert!(r.findings.is_empty());
    }

    #[test]
    fn parse_valid_review_json() {
        let json = r#"{"summary": "Test doc.", "findings": [{"title": "Risk", "description": "Desc", "severity": "warning", "category": "risk", "page": 1, "recommendation": "Fix it"}], "suggested_actions": [{"action_type": "add_comment", "page": 1, "text": "Note", "reason": "Important"}]}"#;
        let citations = vec![Citation { citation_id: "c1".into(), page_index: 0, chunk_id: "ch1".into(), snippet: "test".into(), score: 0.8, source: "native_text".into() }];
        let mut warnings = Vec::new();
        let (summary, findings, actions) = parse_review_output(json, &citations, &mut warnings);
        assert_eq!(summary, "Test doc.");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].category, "risk");
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, "add_comment");
    }

    #[test]
    fn rejects_unsafe_action_types() {
        let json = r#"{"summary": "X", "findings": [], "suggested_actions": [{"action_type": "delete_page", "page": 1, "text": "", "reason": ""}]}"#;
        let mut warnings = Vec::new();
        let (_, _, actions) = parse_review_output(json, &[], &mut warnings);
        assert!(actions.is_empty()); // delete_page is not safe
    }

    #[test]
    fn engineering_findings_included() {
        let eng_findings = vec![crate::engineering::types::EngineeringFinding {
            finding_id: "ef1".into(), finding_type: "total_connected_load".into(),
            severity: crate::engineering::types::FindingSeverity::Info,
            title: "Total: 50 kW".into(), description: "Sum".into(),
            page_refs: vec![0, 1], source_citations: vec![], calculation_trace_id: None,
            recommendation: "Verify".into(), confidence: 0.9,
        }];

        // Simulate the engineering findings integration.
        let eng_review: Vec<ReviewFinding> = eng_findings.iter().map(|ef| ReviewFinding {
            finding_id: ef.finding_id.clone(), title: ef.title.clone(),
            description: ef.description.clone(), severity: "info".into(),
            category: "engineering".into(), page_refs: ef.page_refs.clone(),
            citations: vec![], recommendation: ef.recommendation.clone(), confidence: ef.confidence,
        }).collect();

        assert_eq!(eng_review.len(), 1);
        assert_eq!(eng_review[0].category, "engineering");
    }

    #[test]
    fn invalid_json_returns_raw_summary() {
        let mut warnings = Vec::new();
        let (summary, findings, _) = parse_review_output("Not JSON at all", &[], &mut warnings);
        assert!(!summary.is_empty());
        assert!(findings.is_empty());
        assert!(!warnings.is_empty());
    }

    // -------- 24D: compare-review prompt builder --------

    #[test]
    fn compare_review_prompt_lists_base_and_revised_docs() {
        let prompt = build_compare_review_prompt(
            "base.pdf", "revised.pdf",
            "+3 / -1 / ~2",
            &[],
            0,
            &[],
        );
        assert!(prompt.contains("Base document: base.pdf"));
        assert!(prompt.contains("Revised document: revised.pdf"));
        assert!(prompt.contains("+3 / -1 / ~2"));
        assert!(prompt.contains("ONLY valid JSON"));
    }

    #[test]
    fn compare_review_prompt_caps_text_changes_at_40() {
        let changes: Vec<(String, usize, Option<String>, Option<String>)> = (0..60)
            .map(|i| ("added".into(), i, None, Some(format!("line {i}"))))
            .collect();
        let prompt = build_compare_review_prompt("a", "b", "many", &changes, 0, &[]);
        assert!(prompt.contains("...and 20 more text changes"));
        // We should also see at least one change in the prompt body.
        assert!(prompt.contains("line 0"));
    }

    #[test]
    fn compare_review_prompt_includes_warnings_section() {
        let prompt = build_compare_review_prompt(
            "a", "b", "x",
            &[],
            0,
            &["a-warning".into(), "b-warning".into()],
        );
        assert!(prompt.contains("a-warning"));
        assert!(prompt.contains("b-warning"));
    }
}
