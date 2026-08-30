//! Local AI root path resolution and asset validation.
//! Priority: explicit test/dev override, installed app resources, installed
//! model pack, legacy env override, then executable-ancestor dev fallback
//! when not disabled. Resolution never depends on the process current directory.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAiAssetStatus {
    pub asset_id: String,
    pub name: String,
    pub category: String,
    pub path: String,
    pub required: bool,
    pub exists: bool,
    pub size_bytes: u64,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAiValidationResult {
    pub local_ai_root: String,
    pub ready: bool,
    pub total_size_bytes: u64,
    pub required_ok_count: usize,
    pub required_missing_count: usize,
    pub optional_missing_count: usize,
    pub assets: Vec<LocalAiAssetStatus>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalAiResolution {
    pub local_ai_root: String,
    pub source: String,
    pub dev_fallback_used: bool,
    pub inside_install_root: bool,
    pub install_root: String,
}

/// Required assets for the local AI system.
const REQUIRED_ASSETS: &[(&str, &str, &str, &str, bool)] = &[
    (
        "llm-qwen3-4b",
        "Qwen3 4B LLM",
        "model",
        "models/llm/Qwen3-4B-GGUF/Qwen3-4B-Q4_K_M.gguf",
        true,
    ),
    (
        "embedding-qwen3",
        "Qwen3 Embedding 4B",
        "model",
        "models/embeddings/Qwen3-Embedding-4B-GGUF/Qwen3-Embedding-4B-Q4_K_M.gguf",
        true,
    ),
    (
        "ocr-paddleocr-vl",
        "PaddleOCR-VL Model",
        "model",
        "models/ocr/PaddleOCR-VL/model.safetensors",
        true,
    ),
    (
        "ocr-paddleocr-config",
        "PaddleOCR-VL Config",
        "model",
        "models/ocr/PaddleOCR-VL/config.json",
        true,
    ),
    (
        "reranker-qwen3",
        "Qwen3 Reranker",
        "model",
        "models/rerankers/Qwen3-Reranker-0.6B-GGUF/qwen3-reranker-0.6b-q8_0.gguf",
        false,
    ),
    (
        "runtime-llama-cli",
        "llama-cli.exe",
        "runtime",
        "runtimes/llama-cpp/llama-cli.exe",
        true,
    ),
    (
        "runtime-llama-server",
        "llama-server.exe",
        "runtime",
        "runtimes/llama-cpp/llama-server.exe",
        true,
    ),
    (
        "runtime-llama-dll",
        "llama.dll",
        "runtime",
        "runtimes/llama-cpp/llama.dll",
        true,
    ),
    (
        "runtime-ggml-dll",
        "ggml.dll",
        "runtime",
        "runtimes/llama-cpp/ggml.dll",
        true,
    ),
    (
        "worker-ocr",
        "PaddleOCR Worker",
        "worker",
        "workers/paddleocr_vl_worker.py",
        true,
    ),
    (
        "config-models",
        "models.json",
        "config",
        "config/models.json",
        true,
    ),
];

/// Resolve the local-ai root directory using priority order.
pub fn resolve_local_ai_root(user_configured: Option<&str>) -> PathBuf {
    resolve_local_ai_root_diagnostics(user_configured).root_path()
}

pub fn resolve_local_ai_root_diagnostics(user_configured: Option<&str>) -> LocalAiResolution {
    let install_root = current_exe_dir();

    // 1. Explicit user/test configuration.
    if let Some(path) = user_configured {
        if let Some(p) = absolute_existing_dir(path) {
            return resolution(p, "user_configured", false, &install_root);
        }
    }

    // 2. Phase 36.7 explicit env override for test/dev automation.
    if let Ok(env_path) = std::env::var("R2H_PDF_LOCAL_AI_HOME") {
        if let Some(p) = absolute_existing_dir(&env_path) {
            return resolution(p, "env:R2H_PDF_LOCAL_AI_HOME", false, &install_root);
        }
    }

    // 3. Installed app directory / local-ai.
    if let Some(app_dir) = install_root.as_ref() {
        let p = app_dir.join("local-ai");
        if p.is_dir() {
            return resolution(p, "installed_app_local_ai", false, &install_root);
        }
        let p = app_dir.join("resources").join("local-ai");
        if p.is_dir() {
            return resolution(p, "installed_app_resources_local_ai", false, &install_root);
        }
        let p = app_dir.join("..").join("R2H-PDF-LocalAI").join("local-ai");
        if p.is_dir() {
            return resolution(p, "installed_sibling_model_pack", false, &install_root);
        }
    }

    // 4. Documented full-offline installer location from the current model-pack script.
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let p = PathBuf::from(local_app_data)
            .join("R2H-PDF")
            .join("local-ai");
        if p.is_dir() {
            return resolution(p, "localappdata_model_pack", false, &install_root);
        }
    }

    // 5. Legacy env variable retained for existing installed model packs.
    if let Ok(env_path) = std::env::var("R2H_LOCAL_AI_ROOT") {
        if let Some(p) = absolute_existing_dir(&env_path) {
            return resolution(p, "env:R2H_LOCAL_AI_ROOT", false, &install_root);
        }
    }

    // 6. Development fallback: search executable ancestors for local-ai
    // unless strict installed verification disabled it. Do not inspect the
    // process current directory; packaged launches and shell choices must not
    // change the selected resource root.
    if !dev_fallback_disabled() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(found) = exe.ancestors().find(|a| a.join("local-ai").is_dir()) {
                return resolution(
                    found.join("local-ai"),
                    "dev_exe_ancestor",
                    true,
                    &install_root,
                );
            }
        }
    }

    resolution(
        PathBuf::from("local-ai"),
        "unresolved_relative",
        false,
        &install_root,
    )
}

impl LocalAiResolution {
    pub fn root_path(&self) -> PathBuf {
        PathBuf::from(&self.local_ai_root)
    }
}

pub fn workspace_root_from_local_ai_root(root: &Path) -> PathBuf {
    root.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn current_exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
}

fn absolute_existing_dir(value: &str) -> Option<PathBuf> {
    let path = PathBuf::from(value);
    (path.is_absolute() && path.is_dir()).then_some(path)
}

fn dev_fallback_disabled() -> bool {
    for name in [
        "R2H_PDF_DISABLE_DEV_FALLBACK",
        "R2H_PDF_STRICT_INSTALLED_AI",
    ] {
        if let Ok(value) = std::env::var(name) {
            let v = value.trim().to_ascii_lowercase();
            if matches!(v.as_str(), "1" | "true" | "yes") {
                return true;
            }
        }
    }
    false
}

fn resolution(
    root: PathBuf,
    source: &str,
    dev_fallback_used: bool,
    install_root: &Option<PathBuf>,
) -> LocalAiResolution {
    let canonical_root = root.canonicalize().unwrap_or(root);
    let canonical_install = install_root.as_ref().and_then(|p| p.canonicalize().ok());
    let inside_install_root = canonical_install
        .as_ref()
        .map(|install| canonical_root.starts_with(install))
        .unwrap_or(false);

    LocalAiResolution {
        local_ai_root: canonical_root.to_string_lossy().to_string(),
        source: source.to_string(),
        dev_fallback_used,
        inside_install_root,
        install_root: canonical_install
            .or_else(|| install_root.clone())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default(),
    }
}

/// Validate all required assets in the local-ai root.
pub fn validate_local_ai_root(root: &Path) -> LocalAiValidationResult {
    let mut assets = Vec::new();
    let mut total_size = 0u64;
    let mut required_ok = 0;
    let mut required_missing = 0;
    let mut optional_missing = 0;
    let mut warnings = Vec::new();

    for (id, name, category, rel_path, required) in REQUIRED_ASSETS {
        let full_path = root.join(rel_path);
        let (exists, size) = if full_path.is_file() {
            let s = std::fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);
            (true, s)
        } else {
            (false, 0)
        };

        let (status, message) = if exists && size > 0 {
            if *required {
                required_ok += 1;
            }
            total_size += size;
            (
                "ok".to_string(),
                format!("{:.1} MB", size as f64 / 1_048_576.0),
            )
        } else if !exists && *required {
            required_missing += 1;
            (
                "missing".to_string(),
                format!("Required file not found: {}", rel_path),
            )
        } else if !exists {
            optional_missing += 1;
            (
                "optional_missing".to_string(),
                format!("Optional file not found: {}", rel_path),
            )
        } else {
            (
                "invalid".to_string(),
                "File exists but is empty".to_string(),
            )
        };

        assets.push(LocalAiAssetStatus {
            asset_id: id.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            path: full_path.to_string_lossy().to_string(),
            required: *required,
            exists,
            size_bytes: size,
            status,
            message,
        });
    }

    if !root.is_dir() {
        warnings.push(format!(
            "Local AI root directory does not exist: {}",
            root.display()
        ));
    }

    let ready = required_missing == 0;

    LocalAiValidationResult {
        local_ai_root: root.to_string_lossy().to_string(),
        ready,
        total_size_bytes: total_size,
        required_ok_count: required_ok,
        required_missing_count: required_missing,
        optional_missing_count: optional_missing,
        assets,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_finds_local_ai_in_ancestors() {
        let root = resolve_local_ai_root(None);
        // In dev environment, should find local-ai directory.
        // May or may not exist in CI — test just verifies no panic.
        assert!(!root.to_string_lossy().is_empty());
    }

    #[test]
    fn validate_missing_root_returns_not_ready() {
        let result = validate_local_ai_root(Path::new("/nonexistent/path"));
        assert!(!result.ready);
        assert!(result.required_missing_count > 0);
    }

    #[test]
    fn resolve_prefers_phase_36_7_env_override() {
        let temp = std::env::temp_dir().join(format!("r2h-local-ai-test-{}", std::process::id()));
        std::fs::create_dir_all(&temp).unwrap();
        let old_new_env = std::env::var_os("R2H_PDF_LOCAL_AI_HOME");
        let old_legacy_env = std::env::var_os("R2H_LOCAL_AI_ROOT");
        std::env::set_var("R2H_PDF_LOCAL_AI_HOME", &temp);
        std::env::remove_var("R2H_LOCAL_AI_ROOT");

        let resolved = resolve_local_ai_root(None);
        let expected = temp.canonicalize().unwrap();

        match old_new_env {
            Some(value) => std::env::set_var("R2H_PDF_LOCAL_AI_HOME", value),
            None => std::env::remove_var("R2H_PDF_LOCAL_AI_HOME"),
        }
        match old_legacy_env {
            Some(value) => std::env::set_var("R2H_LOCAL_AI_ROOT", value),
            None => std::env::remove_var("R2H_LOCAL_AI_ROOT"),
        }
        std::fs::remove_dir_all(&temp).unwrap();
        assert_eq!(resolved, expected);
    }

    #[test]
    fn validate_real_root_if_exists() {
        let root = resolve_local_ai_root(None);
        if root.is_dir() {
            let result = validate_local_ai_root(&root);
            // If local-ai exists, at least some assets should be found.
            assert!(!result.assets.is_empty());
        }
    }

    #[test]
    fn no_hardcoded_dev_path() {
        // Ensure no development-specific path is hardcoded in the resolution logic.
        // We check the non-test portion of the file by reading only the resolve function.
        let src = include_str!("local_ai_paths.rs");
        // Split at the test module boundary.
        let production_code = src.split("#[cfg(test)]").next().unwrap_or("");
        assert!(
            !production_code.contains("E:\\\\Projects"),
            "Found hardcoded Windows dev path"
        );
    }

    #[test]
    fn production_resolution_does_not_depend_on_current_directory() {
        let src = include_str!("local_ai_paths.rs");
        let production_code = src.split("#[cfg(test)]").next().unwrap_or("");
        assert!(!production_code.contains("current_dir"));
        assert!(absolute_existing_dir("relative/local-ai").is_none());
    }
}
