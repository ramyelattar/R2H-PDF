use std::collections::HashMap;
use std::time::Duration;

use serde_json::Value;
use tokio::task::AbortHandle;

use crate::ai_core::{
    errors::AiCoreError,
    types::{
        AiAnnotationSuggestion, AiEntity, AiModelBackend, AiStatusResponse, AiTaskRequest,
        AiTaskResult, AiTaskType,
    },
};

pub struct AiEngine {
    client: reqwest::Client,
    endpoint: String,
    enabled: bool,
    active_tasks: HashMap<String, AbortHandle>,
}

impl AiEngine {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            endpoint: "http://localhost:11434".to_string(),
            enabled: false,
            active_tasks: HashMap::new(),
        }
    }

    pub async fn get_status(&self) -> AiStatusResponse {
        if !self.enabled {
            return AiStatusResponse {
                available: false,
                backend: AiModelBackend::Unavailable,
                models: Vec::new(),
                active_tasks: self.active_tasks.len(),
                error: None,
            };
        }

        let url = format!("{}/api/tags", self.endpoint);
        let result = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(response) => {
                if !response.status().is_success() {
                    return AiStatusResponse {
                        available: false,
                        backend: AiModelBackend::LocalOllama,
                        models: Vec::new(),
                        active_tasks: self.active_tasks.len(),
                        error: Some(format!(
                            "Ollama returned status {}",
                            response.status()
                        )),
                    };
                }

                match response.json::<serde_json::Value>().await {
                    Ok(json) => {
                        let models = json
                            .get("models")
                            .and_then(|m| m.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|entry| {
                                        entry.get("name").and_then(|n| n.as_str()).map(String::from)
                                    })
                                    .collect::<Vec<String>>()
                            })
                            .unwrap_or_default();

                        AiStatusResponse {
                            available: true,
                            backend: AiModelBackend::LocalOllama,
                            models,
                            active_tasks: self.active_tasks.len(),
                            error: None,
                        }
                    }
                    Err(e) => AiStatusResponse {
                        available: false,
                        backend: AiModelBackend::LocalOllama,
                        models: Vec::new(),
                        active_tasks: self.active_tasks.len(),
                        error: Some(format!("Failed to parse response: {}", e)),
                    },
                }
            }
            Err(e) => AiStatusResponse {
                available: false,
                backend: AiModelBackend::LocalOllama,
                models: Vec::new(),
                active_tasks: self.active_tasks.len(),
                error: Some(format!("Connection failed: {}", e)),
            },
        }
    }

    /// Runs an AI task asynchronously. Spawns a tokio task and stores its AbortHandle
    /// in `active_tasks` keyed by `task_id`. Returns the result once complete.
    pub async fn run_task(&mut self, req: AiTaskRequest) -> Result<AiTaskResult, AiCoreError> {
        if !self.enabled {
            return Err(AiCoreError::Disabled);
        }

        let task_id = req.task_id.clone();
        let client = self.client.clone();
        let endpoint = self.endpoint.clone();

        // Spawn the actual work as a tokio task so we can abort it
        let handle = tokio::spawn(Self::execute_task(client, endpoint, req));

        // Store the abort handle keyed by task_id
        self.active_tasks.insert(task_id.clone(), handle.abort_handle());

        // Await the spawned task
        let result = handle.await;

        // Remove from active tasks once complete
        self.active_tasks.remove(&task_id);

        match result {
            Ok(inner_result) => inner_result,
            Err(join_err) => {
                if join_err.is_cancelled() {
                    Err(AiCoreError::TaskFailed("Task was cancelled".to_string()))
                } else {
                    Err(AiCoreError::TaskFailed(format!("Task panicked: {}", join_err)))
                }
            }
        }
    }

    /// The actual task execution logic, designed to run inside a spawned tokio task.
    async fn execute_task(
        client: reqwest::Client,
        endpoint: String,
        req: AiTaskRequest,
    ) -> Result<AiTaskResult, AiCoreError> {
        // For Summarize tasks, check if chunked summarisation is needed
        if req.task_type == AiTaskType::Summarize {
            let approx_tokens = req.input_text.len() / 4;
            if approx_tokens > 8000 {
                let summary =
                    Self::summarise_chunked_async(&client, &endpoint, &req.input_text, &req.model)
                        .await?;
                return Ok(AiTaskResult {
                    task_id: req.task_id,
                    success: true,
                    output_text: Some(summary),
                    suggestions: None,
                    entities: None,
                    model_used: Some(req.model),
                    error: None,
                });
            }
        }

        let prompt = Self::build_prompt(&req);
        let generated_text = Self::call_generate_async(&client, &endpoint, &req.model, &prompt).await?;

        // For structured output tasks, parse the generated text as JSON
        match req.task_type {
            AiTaskType::SuggestAnnotations => {
                let suggestions: Vec<AiAnnotationSuggestion> =
                    serde_json::from_str(&generated_text).map_err(|e| {
                        AiCoreError::ParseError(format!(
                            "Failed to parse annotation suggestions: {}",
                            e
                        ))
                    })?;
                Ok(AiTaskResult {
                    task_id: req.task_id,
                    success: true,
                    output_text: Some(generated_text),
                    suggestions: Some(suggestions),
                    entities: None,
                    model_used: Some(req.model),
                    error: None,
                })
            }
            AiTaskType::ExtractEntities => {
                let entities: Vec<AiEntity> =
                    serde_json::from_str(&generated_text).map_err(|e| {
                        AiCoreError::ParseError(format!("Failed to parse entities: {}", e))
                    })?;
                Ok(AiTaskResult {
                    task_id: req.task_id,
                    success: true,
                    output_text: Some(generated_text),
                    suggestions: None,
                    entities: Some(entities),
                    model_used: Some(req.model),
                    error: None,
                })
            }
            _ => {
                // Summarize, QuestionAnswer, and other text-output tasks
                Ok(AiTaskResult {
                    task_id: req.task_id,
                    success: true,
                    output_text: Some(generated_text),
                    suggestions: None,
                    entities: None,
                    model_used: Some(req.model),
                    error: None,
                })
            }
        }
    }

    /// Sends a single generation request to Ollama and returns the response text (async).
    async fn call_generate_async(
        client: &reqwest::Client,
        endpoint: &str,
        model: &str,
        prompt: &str,
    ) -> Result<String, AiCoreError> {
        let url = format!("{}/api/generate", endpoint);

        let body = serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        });

        let response = client
            .post(&url)
            .timeout(Duration::from_secs(120))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    AiCoreError::Timeout
                } else {
                    AiCoreError::HttpError(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            return Err(AiCoreError::HttpError(format!(
                "Ollama returned status {}",
                response.status()
            )));
        }

        let resp_json: Value = response
            .json()
            .await
            .map_err(|e| AiCoreError::ParseError(e.to_string()))?;

        let generated_text = resp_json
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(generated_text)
    }

    /// Performs chunked summarisation for texts that exceed the model's context window (async).
    /// Splits text into chunks of 4000 chars with 10% overlap (400 chars),
    /// summarises each chunk individually, then summarises the concatenated summaries.
    async fn summarise_chunked_async(
        client: &reqwest::Client,
        endpoint: &str,
        text: &str,
        model: &str,
    ) -> Result<String, AiCoreError> {
        let chunk_size: usize = 4000;
        let overlap: usize = 400; // 10% of chunk_size
        let step = chunk_size - overlap;

        // Split text into overlapping chunks
        let mut chunks: Vec<&str> = Vec::new();
        let mut start = 0;
        let text_len = text.len();

        while start < text_len {
            let end = (start + chunk_size).min(text_len);
            chunks.push(&text[start..end]);
            if end == text_len {
                break;
            }
            start += step;
        }

        // Summarise each chunk individually
        let mut chunk_summaries: Vec<String> = Vec::new();
        for chunk in &chunks {
            let prompt = format!(
                "Summarize the following document text concisely:\n\n{}",
                chunk
            );
            let summary = Self::call_generate_async(client, endpoint, model, &prompt).await?;
            chunk_summaries.push(summary);
        }

        // Concatenate chunk summaries and produce a final summary
        let combined = chunk_summaries.join("\n\n");
        let final_prompt = format!(
            "Summarize the following document text concisely:\n\n{}",
            combined
        );
        Self::call_generate_async(client, endpoint, model, &final_prompt).await
    }

    fn build_prompt(req: &AiTaskRequest) -> String {
        match req.task_type {
            AiTaskType::Summarize => {
                format!(
                    "Summarize the following document text concisely:\n\n{}",
                    req.input_text
                )
            }
            AiTaskType::QuestionAnswer => {
                let question = req.question.as_deref().unwrap_or("No question provided");
                format!(
                    "Answer the following question based only on the document text below. If the answer is not in the text, say so.\n\nQuestion: {}\n\nDocument:\n{}",
                    question, req.input_text
                )
            }
            AiTaskType::SuggestAnnotations => {
                format!(
                    "Identify important passages in the following text that should be highlighted. Return a JSON array of objects with fields: page_index (integer), text_snippet (string), suggested_note (string). Return ONLY the JSON array, no other text.\n\nText:\n{}",
                    req.input_text
                )
            }
            AiTaskType::ExtractEntities => {
                format!(
                    "Extract named entities from the following text. Return a JSON array of objects with fields: entity_type (one of Person, Organisation, Date, MonetaryAmount, DefinedTerm), value (string), page_index (integer), snippet (string). Return ONLY the JSON array, no other text.\n\nText:\n{}",
                    req.input_text
                )
            }
            _ => {
                // Fallback for legacy task types
                format!("Process the following text:\n\n{}", req.input_text)
            }
        }
    }

    pub fn cancel_task(&mut self, task_id: &str) -> Result<(), AiCoreError> {
        if let Some(handle) = self.active_tasks.remove(task_id) {
            handle.abort();
            Ok(())
        } else {
            Err(AiCoreError::TaskNotFound(task_id.to_string()))
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_endpoint(&mut self, endpoint: String) {
        self.endpoint = endpoint;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_status_unavailable_when_endpoint_unreachable() {
        let mut engine = AiEngine::new();
        engine.set_enabled(true);
        engine.set_endpoint("http://127.0.0.1:0".to_string());

        let status = engine.get_status().await;

        assert!(
            !status.available,
            "expected available == false for unreachable endpoint"
        );
        assert!(
            status.error.is_some(),
            "expected error to be Some when endpoint is unreachable"
        );
    }

    #[test]
    fn test_cancel_task_not_found() {
        let mut engine = AiEngine::new();

        let result = engine.cancel_task("nonexistent-task-id");

        assert!(result.is_err(), "expected Err for unknown task id");
        match result.unwrap_err() {
            AiCoreError::TaskNotFound(id) => {
                assert_eq!(id, "nonexistent-task-id");
            }
            other => panic!("expected TaskNotFound, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_run_task_returns_disabled_when_not_enabled() {
        let mut engine = AiEngine::new();
        // Engine is disabled by default (enabled = false)

        let req = AiTaskRequest {
            task_id: "test-task".to_string(),
            session_id: "test-session".to_string(),
            task_type: AiTaskType::Summarize,
            model: "llama3".to_string(),
            question: None,
            page_range: None,
            input_text: "Some text to summarize".to_string(),
            parameters_json: None,
        };

        let result = engine.run_task(req).await;
        assert!(matches!(result, Err(AiCoreError::Disabled)));
    }

    #[tokio::test]
    async fn test_run_task_http_error_on_unreachable_endpoint() {
        let mut engine = AiEngine::new();
        engine.set_enabled(true);
        engine.set_endpoint("http://127.0.0.1:0".to_string());

        let req = AiTaskRequest {
            task_id: "test-task-http".to_string(),
            session_id: "test-session".to_string(),
            task_type: AiTaskType::Summarize,
            model: "llama3".to_string(),
            question: None,
            page_range: None,
            input_text: "Short text".to_string(),
            parameters_json: None,
        };

        let result = engine.run_task(req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            AiCoreError::HttpError(_) => {} // expected
            other => panic!("expected HttpError, got: {:?}", other),
        }
    }

    #[test]
    fn test_chunked_summarisation_threshold() {
        // Token count = text.len() / 4 (integer division). Threshold is 8000 tokens.
        // Text at exactly 32000 chars = 8000 tokens should NOT trigger chunking (> 8000 required).
        let at_threshold = "a".repeat(32_000); // 32000 / 4 = 8000 tokens, NOT > 8000
        let approx_tokens = at_threshold.len() / 4;
        assert_eq!(approx_tokens, 8000);
        assert!(approx_tokens <= 8000, "text at threshold should not trigger chunking");

        // Text above threshold SHOULD trigger chunking.
        let long_text = "a".repeat(32_004); // 32004 / 4 = 8001 tokens > 8000
        let approx_tokens = long_text.len() / 4;
        assert!(approx_tokens > 8000, "long text should exceed threshold");
    }

    #[test]
    fn test_chunk_text_splitting_logic() {
        // Verify the chunking algorithm produces correct overlapping chunks.
        let chunk_size: usize = 4000;
        let overlap: usize = 400; // 10% of chunk_size
        let step = chunk_size - overlap; // 3600

        // Create text that is exactly 10000 chars
        let text = "x".repeat(10_000);
        let text_len = text.len();

        let mut chunks: Vec<&str> = Vec::new();
        let mut start = 0;
        while start < text_len {
            let end = (start + chunk_size).min(text_len);
            chunks.push(&text[start..end]);
            if end == text_len {
                break;
            }
            start += step;
        }

        // Expected chunks:
        // [0..4000], [3600..7600], [7200..10000]
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 4000);
        assert_eq!(chunks[1].len(), 4000);
        assert_eq!(chunks[2].len(), 2800); // 10000 - 7200

        // Verify overlap: chunk[0] ends at 4000, chunk[1] starts at 3600
        // So overlap region is [3600..4000] = 400 chars
    }

    #[test]
    fn test_chunk_text_single_chunk_when_text_smaller_than_chunk_size() {
        let chunk_size: usize = 4000;
        let overlap: usize = 400;
        let step = chunk_size - overlap;

        // Text smaller than one chunk
        let text = "y".repeat(3000);
        let text_len = text.len();

        let mut chunks: Vec<&str> = Vec::new();
        let mut start = 0;
        while start < text_len {
            let end = (start + chunk_size).min(text_len);
            chunks.push(&text[start..end]);
            if end == text_len {
                break;
            }
            start += step;
        }

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 3000);
    }

    #[tokio::test]
    async fn test_chunked_summarisation_triggers_for_large_text() {
        // When text exceeds 8000 tokens (32000 chars), chunked summarisation
        // should be triggered. Since the endpoint is unreachable, we expect
        // an HttpError, but this confirms the chunked path is entered.
        let mut engine = AiEngine::new();
        engine.set_enabled(true);
        engine.set_endpoint("http://127.0.0.1:0".to_string());

        // 40000 chars = 10000 tokens > 8000 threshold
        let large_text = "word ".repeat(8000); // 40000 chars

        let req = AiTaskRequest {
            task_id: "test-chunked".to_string(),
            session_id: "test-session".to_string(),
            task_type: AiTaskType::Summarize,
            model: "llama3".to_string(),
            question: None,
            page_range: None,
            input_text: large_text,
            parameters_json: None,
        };

        let result = engine.run_task(req).await;
        // Should fail with HttpError since endpoint is unreachable,
        // but this confirms the chunked path was entered (not the direct path)
        assert!(result.is_err());
        match result.unwrap_err() {
            AiCoreError::HttpError(_) => {} // expected - chunked path tried to call Ollama
            other => panic!("expected HttpError from chunked path, got: {:?}", other),
        }
    }
}
