use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub model_type: String,
    pub format: String,
    pub path: String,
    pub runtime: String,
    #[serde(rename = "contextWindow")]
    pub context_window: usize,
    pub default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsConfig {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    pub models: Vec<LocalModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalModelInfo {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub format: String,
    pub path: String,
    pub runtime: String,
    pub exists: bool,
    pub size_bytes: u64,
    pub default: bool,
    pub context_window: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRuntimeStatus {
    pub available: bool,
    pub runtime_path: String,
    pub runtime_version: String,
    pub default_model_id: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalGenerateRequest {
    pub model_id: String,
    pub prompt: String,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalGenerateResult {
    pub model_id: String,
    pub text: String,
    pub tokens_generated: Option<u32>,
    pub elapsed_ms: u64,
    pub finish_reason: String,
    pub warnings: Vec<String>,
}
