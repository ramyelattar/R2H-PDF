use serde::{Deserialize, Serialize};

use crate::document_core::types::PageRange;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskType {
    Summarize,
    ExtractKeyPoints,
    TranslateText,
    AnswerQuestion,
    ClassifyDocument,
    QuestionAnswer,
    SuggestAnnotations,
    ExtractEntities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiModelBackend {
    Unavailable,
    LocalOllama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskRequest {
    pub task_id: String,
    pub session_id: String,
    pub task_type: AiTaskType,
    pub model: String,
    pub question: Option<String>,
    pub page_range: Option<PageRange>,
    pub input_text: String,
    pub parameters_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiAnnotationSuggestion {
    pub page_index: usize,
    pub text_snippet: String,
    pub suggested_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiEntity {
    pub entity_type: String, // "Person"|"Organisation"|"Date"|"MonetaryAmount"|"DefinedTerm"
    pub value: String,
    pub page_index: usize,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiTaskResult {
    pub task_id: String,
    pub success: bool,
    pub output_text: Option<String>,
    pub suggestions: Option<Vec<AiAnnotationSuggestion>>,
    pub entities: Option<Vec<AiEntity>>,
    pub model_used: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStatusResponse {
    pub available: bool,
    pub backend: AiModelBackend,
    pub models: Vec<String>,
    pub active_tasks: usize,
    pub error: Option<String>,
}
