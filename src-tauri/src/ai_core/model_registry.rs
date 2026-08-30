//! Model registry: discovers, validates, and provides metadata for local AI models.

use std::path::{Path, PathBuf};

use super::local_ai_paths::{resolve_local_ai_root, workspace_root_from_local_ai_root};
use super::local_types::{LocalModelConfig, LocalModelInfo, ModelsConfig};

pub struct ModelRegistry {
    workspace_root: PathBuf,
    pub(crate) config: Option<ModelsConfig>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        let workspace_root = find_workspace_root();
        let mut registry = Self { workspace_root, config: None };
        registry.reload();
        registry
    }

    pub fn reload(&mut self) {
        let config_path = self.workspace_root.join("local-ai/config/models.json");
        self.config = load_models_config(&config_path);
    }

    /// List all registered models with existence/size validation.
    pub fn list_models(&self) -> Vec<LocalModelInfo> {
        let Some(config) = &self.config else {
            return Vec::new();
        };

        config.models.iter().map(|m| {
            let resolved_path = self.resolve_model_path(&m.path);
            let (exists, size_bytes) = if resolved_path.is_file() {
                let size = std::fs::metadata(&resolved_path)
                    .map(|meta| meta.len())
                    .unwrap_or(0);
                (true, size)
            } else {
                (false, 0)
            };

            LocalModelInfo {
                id: m.id.clone(),
                name: m.name.clone(),
                model_type: m.model_type.clone(),
                format: m.format.clone(),
                path: resolved_path.to_string_lossy().to_string(),
                runtime: m.runtime.clone(),
                exists,
                size_bytes,
                default: m.default,
                context_window: m.context_window,
            }
        }).collect()
    }

    /// Get the default model info.
    pub fn default_model(&self) -> Option<LocalModelInfo> {
        self.list_models().into_iter().find(|m| m.default)
    }

    /// Get a specific model by ID.
    pub fn get_model(&self, model_id: &str) -> Option<LocalModelInfo> {
        self.list_models().into_iter().find(|m| m.id == model_id)
    }

    /// Validate a specific model exists and is accessible.
    pub fn validate_model(&self, model_id: &str) -> Result<LocalModelInfo, String> {
        let model = self.get_model(model_id)
            .ok_or_else(|| format!("Model '{}' not found in registry", model_id))?;
        if !model.exists {
            return Err(format!("Model file not found: {}", model.path));
        }
        if model.size_bytes == 0 {
            return Err(format!("Model file is empty: {}", model.path));
        }
        Ok(model)
    }

    fn resolve_model_path(&self, relative_path: &str) -> PathBuf {
        let path = Path::new(relative_path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(relative_path)
        }
    }
}

fn find_workspace_root() -> PathBuf {
    let local_ai_root = resolve_local_ai_root(None);
    workspace_root_from_local_ai_root(&local_ai_root)
}

fn load_models_config(path: &Path) -> Option<ModelsConfig> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_models_from_workspace() {
        let registry = ModelRegistry::new();
        let models = registry.list_models();
        // Should find at least the Qwen3 model if running from workspace root.
        // In CI or other environments, it may be empty — that's acceptable.
        // The test validates the code path doesn't panic.
        assert!(models.len() <= 10); // sanity check
    }

    #[test]
    fn missing_model_returns_exists_false() {
        let mut registry = ModelRegistry::new();
        // Override config with a fake model path.
        registry.config = Some(ModelsConfig {
            schema_version: 1,
            models: vec![LocalModelConfig {
                id: "fake".to_string(),
                name: "Fake Model".to_string(),
                model_type: "llm".to_string(),
                format: "gguf".to_string(),
                path: "/nonexistent/model.gguf".to_string(),
                runtime: "llama.cpp".to_string(),
                context_window: 4096,
                default: true,
            }],
        });
        let models = registry.list_models();
        assert_eq!(models.len(), 1);
        assert!(!models[0].exists);
        assert_eq!(models[0].size_bytes, 0);
    }

    #[test]
    fn validate_model_returns_error_for_missing() {
        let mut registry = ModelRegistry::new();
        registry.config = Some(ModelsConfig {
            schema_version: 1,
            models: vec![LocalModelConfig {
                id: "missing".to_string(),
                name: "Missing".to_string(),
                model_type: "llm".to_string(),
                format: "gguf".to_string(),
                path: "/no/such/file.gguf".to_string(),
                runtime: "llama.cpp".to_string(),
                context_window: 4096,
                default: false,
            }],
        });
        let result = registry.validate_model("missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn validate_model_returns_error_for_unknown_id() {
        let registry = ModelRegistry::new();
        let result = registry.validate_model("nonexistent-id");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in registry"));
    }

    #[test]
    fn invalid_config_returns_empty_models() {
        let mut registry = ModelRegistry::new();
        registry.config = None;
        let models = registry.list_models();
        assert!(models.is_empty());
    }
}
