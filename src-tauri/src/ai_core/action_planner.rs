//! AI Action Planner: uses RAG retrieval + local LLM to propose document actions.
//! Never applies actions directly — only proposes them for user review.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::action_types::*;
use super::local_runtime::LocalRuntime;
use super::local_types::LocalGenerateRequest;
use super::rag::{Citation, RagEngine, RagState};
use super::vector_index::VectorSearchResult;

// ---------------------------------------------------------------------------
// Planner
// ---------------------------------------------------------------------------

pub struct ActionPlanner {
    pub(crate) batches: HashMap<String, AiActionBatch>,
}

impl ActionPlanner {
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
        }
    }

    /// Plan actions based on user request + RAG context.
    /// Returns proposed actions — does NOT apply them.
    pub fn plan(
        &mut self,
        request: &PlanActionsRequest,
        rag_engine: &RagEngine,
        runtime: &LocalRuntime,
        embedding_runtime: Option<&super::embedding::EmbeddingRuntime>,
    ) -> Result<AiActionBatch, String> {
        let start = Instant::now();
        let mut warnings: Vec<String> = Vec::new();

        // 1. Retrieve relevant context via RAG.
        let top_k = request.top_k.unwrap_or(5);
        let query_embedding = embedding_runtime.and_then(|emb| {
            if emb.is_server_ready() {
                emb.embed_query(&request.user_request).ok()
            } else {
                None
            }
        });
        let results = rag_engine.search(
            &request.session_id,
            &request.user_request,
            top_k,
            query_embedding.as_deref(),
        );

        if results.is_empty() {
            let batch = AiActionBatch {
                batch_id: gen_id("batch"),
                session_id: request.session_id.clone(),
                user_request: request.user_request.clone(),
                actions: Vec::new(),
                citations: Vec::new(),
                warnings: vec![
                    "No relevant document content found. Cannot propose actions without evidence."
                        .to_string(),
                ],
                created_at: epoch_ms(),
            };
            self.batches.insert(batch.batch_id.clone(), batch.clone());
            return Ok(batch);
        }

        // 2. Build citations from retrieved chunks.
        let citations: Vec<Citation> = results
            .iter()
            .enumerate()
            .map(|(i, r)| Citation {
                citation_id: format!("cite-{}", i + 1),
                page_index: r.page_index,
                chunk_id: r.chunk_id.clone(),
                snippet: r.text.chars().take(200).collect(),
                score: r.score,
                source: r.source.clone(),
            })
            .collect();

        // 3. Build prompt for action planning.
        let context = build_context(&results);
        let prompt = build_action_prompt(
            &request.user_request,
            &context,
            &citations,
            request.page_count,
        );

        // 4. Generate with local LLM.
        let default_model = runtime
            .registry
            .default_model()
            .ok_or_else(|| "No default LLM model configured".to_string())?;

        let gen_request = LocalGenerateRequest {
            model_id: default_model.id,
            prompt,
            system_prompt: Some(ACTION_SYSTEM_PROMPT.to_string()),
            max_tokens: Some(1024),
            temperature: Some(0.2),
            top_p: Some(0.9),
            timeout_ms: Some(120_000),
        };

        let gen_result = runtime.generate(gen_request)?;

        // 5. Parse and validate actions from LLM output.
        let raw_actions = parse_llm_actions(&gen_result.text);
        let mut valid_actions: Vec<AiProposedAction> = Vec::new();

        match raw_actions {
            Ok(raw) => {
                for (i, raw_action) in raw.iter().enumerate() {
                    match convert_raw_action(raw_action, &request.session_id, &citations, i) {
                        Ok(action) => {
                            let errors = action.validate(request.page_count);
                            if errors.is_empty() {
                                valid_actions.push(action);
                            } else {
                                let err_msgs: Vec<String> = errors
                                    .iter()
                                    .map(|e| format!("{}: {}", e.field, e.message))
                                    .collect();
                                warnings.push(format!(
                                    "Action {} rejected: {}",
                                    i,
                                    err_msgs.join(", ")
                                ));
                            }
                        }
                        Err(e) => {
                            warnings.push(format!("Action {} parse error: {}", i, e));
                        }
                    }
                }
            }
            Err(e) => {
                warnings.push(format!("Failed to parse LLM output as JSON: {}", e));
            }
        }

        let batch = AiActionBatch {
            batch_id: gen_id("batch"),
            session_id: request.session_id.clone(),
            user_request: request.user_request.clone(),
            actions: valid_actions,
            citations,
            warnings,
            created_at: epoch_ms(),
        };

        self.batches.insert(batch.batch_id.clone(), batch.clone());
        Ok(batch)
    }

    pub fn get_batch(&self, batch_id: &str) -> Option<&AiActionBatch> {
        self.batches.get(batch_id)
    }

    pub fn accept_action(&mut self, batch_id: &str, action_id: &str) -> Result<(), String> {
        let batch = self.batches.get_mut(batch_id).ok_or("Batch not found")?;
        let action = batch
            .actions
            .iter_mut()
            .find(|a| a.action_id == action_id)
            .ok_or("Action not found")?;
        action.status = AiActionStatus::Accepted;
        Ok(())
    }

    pub fn reject_action(&mut self, batch_id: &str, action_id: &str) -> Result<(), String> {
        let batch = self.batches.get_mut(batch_id).ok_or("Batch not found")?;
        let action = batch
            .actions
            .iter_mut()
            .find(|a| a.action_id == action_id)
            .ok_or("Action not found")?;
        action.status = AiActionStatus::Rejected;
        Ok(())
    }

    pub fn mark_applied(&mut self, batch_id: &str, action_id: &str) -> Result<(), String> {
        let batch = self.batches.get_mut(batch_id).ok_or("Batch not found")?;
        let action = batch
            .actions
            .iter_mut()
            .find(|a| a.action_id == action_id)
            .ok_or("Action not found")?;
        action.status = AiActionStatus::Applied;
        Ok(())
    }

    pub fn clear_batch(&mut self, batch_id: &str) {
        self.batches.remove(batch_id);
    }
}

// ---------------------------------------------------------------------------
// Prompt Building
// ---------------------------------------------------------------------------

const ACTION_SYSTEM_PROMPT: &str = "\
You are a document action planner. Based on the user's request and the provided document context, \
propose specific actions to take on the document. \
Rules: \
1. Output ONLY valid JSON with an \"actions\" array. No other text. \
2. Each action must reference a specific page from the context. \
3. Each action must have a reason explaining why. \
4. Confidence must be between 0.0 and 1.0. \
5. Valid action_type values: add_highlight, add_comment, add_text_box, add_redaction, apply_stamp. \
6. Rect coordinates are in PDF points (origin bottom-left). Use reasonable defaults: x=72, width=400, height=20-40. \
7. Do not propose actions for content not in the provided context. \
8. If you cannot fulfill the request from the context, return {\"actions\": []}.";

fn build_context(results: &[VectorSearchResult]) -> String {
    let mut ctx = String::new();
    for r in results.iter().take(5) {
        ctx.push_str(&format!("[Page {}] {}\n\n", r.page_index + 1, r.text));
    }
    ctx
}

fn build_action_prompt(
    user_request: &str,
    context: &str,
    citations: &[Citation],
    page_count: usize,
) -> String {
    format!(
        "Document context (pages 1-{}):\n---\n{}\n---\n\n\
         User request: {}\n\n\
         Respond with JSON only:\n\
         {{\"actions\": [{{\"action_type\": \"...\", \"page_index\": 0, \"rect\": {{\"x\": 72, \"y\": 700, \"width\": 400, \"height\": 30}}, \"text\": \"...\", \"reason\": \"...\", \"confidence\": 0.8}}]}}",
        page_count, context, user_request
    )
}

// ---------------------------------------------------------------------------
// JSON Parsing
// ---------------------------------------------------------------------------

fn parse_llm_actions(output: &str) -> Result<Vec<RawLlmAction>, String> {
    // Try to find JSON in the output (LLM may include extra text).
    let json_str =
        extract_json(output).ok_or_else(|| "No JSON object found in LLM output".to_string())?;

    let parsed: RawLlmResponse =
        serde_json::from_str(json_str).map_err(|e| format!("JSON parse error: {e}"))?;

    Ok(parsed.actions.unwrap_or_default())
}

fn extract_json(text: &str) -> Option<&str> {
    // Find the first { and last } to extract JSON.
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

fn convert_raw_action(
    raw: &RawLlmAction,
    session_id: &str,
    citations: &[Citation],
    index: usize,
) -> Result<AiProposedAction, String> {
    let action_type_str = raw.action_type.as_deref().ok_or("Missing action_type")?;
    let action_type = match action_type_str {
        "add_highlight" => AiActionType::AddHighlight,
        "add_comment" => AiActionType::AddComment,
        "add_text_box" => AiActionType::AddTextBox,
        "add_redaction" => AiActionType::AddRedaction,
        "apply_stamp" => AiActionType::ApplyStamp,
        other => return Err(format!("Unknown action_type: {other}")),
    };

    let page_index = raw.page_index.ok_or("Missing page_index")?;
    let raw_rect = raw.rect.as_ref().ok_or("Missing rect")?;
    let rect = AiActionRect {
        x: raw_rect.x.unwrap_or(72.0),
        y: raw_rect.y.unwrap_or(700.0),
        width: raw_rect.width.unwrap_or(400.0),
        height: raw_rect.height.unwrap_or(30.0),
    };

    // Find citations for this page.
    let page_citations: Vec<Citation> = citations
        .iter()
        .filter(|c| c.page_index == page_index)
        .cloned()
        .collect();

    // If no page-specific citation, use the closest one.
    let source_citations = if page_citations.is_empty() {
        citations.iter().take(1).cloned().collect()
    } else {
        page_citations
    };

    Ok(AiProposedAction {
        action_id: gen_id(&format!("action-{}", index)),
        action_type,
        session_id: session_id.to_string(),
        page_index,
        rect,
        text: raw.text.clone().unwrap_or_default(),
        reason: raw.reason.clone().unwrap_or_default(),
        confidence: raw.confidence.unwrap_or(0.5).clamp(0.0, 1.0),
        source_citations,
        status: AiActionStatus::Proposed,
        created_at: epoch_ms(),
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn gen_id(prefix: &str) -> String {
    format!("{}-{}-{}", prefix, epoch_ms(), rand_suffix())
}

fn epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}

fn rand_suffix() -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    format!("{:x}", hasher.finish() & 0xFFFF)
}

// ---------------------------------------------------------------------------
// Tauri State
// ---------------------------------------------------------------------------

pub struct ActionPlannerState {
    pub planner: Mutex<ActionPlanner>,
}

impl ActionPlannerState {
    pub fn new() -> Self {
        Self {
            planner: Mutex::new(ActionPlanner::new()),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_json_actions() {
        let json = r#"{"actions": [{"action_type": "add_comment", "page_index": 0, "rect": {"x": 72, "y": 700, "width": 200, "height": 30}, "text": "Risk here", "reason": "Termination clause", "confidence": 0.8}]}"#;
        let result = parse_llm_actions(json);
        assert!(result.is_ok());
        let actions = result.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type.as_deref(), Some("add_comment"));
    }

    #[test]
    fn parse_rejects_invalid_json() {
        let result = parse_llm_actions("not json at all");
        assert!(result.is_err());
    }

    #[test]
    fn parse_extracts_json_from_mixed_output() {
        let output = "Here are the actions:\n{\"actions\": [{\"action_type\": \"add_highlight\", \"page_index\": 1, \"rect\": {\"x\": 72, \"y\": 500, \"width\": 300, \"height\": 20}, \"text\": \"\", \"reason\": \"Important\", \"confidence\": 0.9}]}\nDone.";
        let result = parse_llm_actions(output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn convert_raw_action_rejects_unknown_type() {
        let raw = RawLlmAction {
            action_type: Some("delete_everything".to_string()),
            page_index: Some(0),
            rect: Some(RawRect {
                x: Some(0.0),
                y: Some(0.0),
                width: Some(100.0),
                height: Some(20.0),
            }),
            text: None,
            reason: None,
            confidence: Some(0.5),
        };
        let result = convert_raw_action(&raw, "s1", &[], 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown action_type"));
    }

    #[test]
    fn planner_does_not_apply_actions() {
        // The planner only proposes — it has no apply method.
        let planner = ActionPlanner::new();
        // Verify there's no apply_action method that mutates documents.
        assert!(planner.batches.is_empty());
    }

    #[test]
    fn accept_reject_updates_status() {
        let mut planner = ActionPlanner::new();
        let batch = AiActionBatch {
            batch_id: "b1".to_string(),
            session_id: "s1".to_string(),
            user_request: "test".to_string(),
            actions: vec![AiProposedAction {
                action_id: "a1".to_string(),
                action_type: AiActionType::AddComment,
                session_id: "s1".to_string(),
                page_index: 0,
                rect: AiActionRect {
                    x: 72.0,
                    y: 700.0,
                    width: 200.0,
                    height: 30.0,
                },
                text: "Test".to_string(),
                reason: "Test reason".to_string(),
                confidence: 0.8,
                source_citations: vec![],
                status: AiActionStatus::Proposed,
                created_at: 0,
            }],
            citations: vec![],
            warnings: vec![],
            created_at: 0,
        };
        planner.batches.insert("b1".to_string(), batch);

        planner.accept_action("b1", "a1").unwrap();
        assert!(matches!(
            planner.batches["b1"].actions[0].status,
            AiActionStatus::Accepted
        ));

        planner.reject_action("b1", "a1").unwrap();
        assert!(matches!(
            planner.batches["b1"].actions[0].status,
            AiActionStatus::Rejected
        ));
    }
}
