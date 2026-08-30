//! Local LLM runtime: discovers llama.cpp executable and runs generation.
//! Uses process invocation (sidecar pattern) — no Rust FFI binding required.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use super::local_ai_paths::{resolve_local_ai_root, workspace_root_from_local_ai_root};
use super::local_types::{LocalGenerateRequest, LocalGenerateResult, LocalRuntimeStatus};
use super::model_registry::ModelRegistry;

const RUNTIME_DIR: &str = "local-ai/runtimes/llama-cpp";
const LLAMA_CLI_NAMES: &[&str] = &["llama-cli.exe", "llama-cli", "main.exe", "main"];

pub struct LocalRuntime {
    pub registry: ModelRegistry,
    runtime_path: Option<PathBuf>,
    workspace_root: PathBuf,
}

impl LocalRuntime {
    pub fn new() -> Self {
        let workspace_root = find_workspace_root();
        let runtime_path = discover_runtime(&workspace_root);
        Self {
            registry: ModelRegistry::new(),
            runtime_path,
            workspace_root,
        }
    }

    /// Get the current runtime status.
    pub fn status(&self) -> LocalRuntimeStatus {
        let mut errors = Vec::new();

        let available = self.runtime_path.is_some();
        if !available {
            errors.push(format!(
                "llama.cpp runtime not found in {}",
                self.workspace_root.join(RUNTIME_DIR).display()
            ));
        }

        let default_model = self.registry.default_model();
        if let Some(ref model) = default_model {
            if !model.exists {
                errors.push(format!("Default model file not found: {}", model.path));
            }
        } else {
            errors.push("No default model configured in models.json".to_string());
        }

        LocalRuntimeStatus {
            available,
            runtime_path: self
                .runtime_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            runtime_version: if available {
                "llama.cpp (local)".to_string()
            } else {
                String::new()
            },
            default_model_id: default_model.map(|m| m.id),
            errors,
        }
    }

    /// Run a local generation request using llama-cli.
    pub fn generate(&self, request: LocalGenerateRequest) -> Result<LocalGenerateResult, String> {
        let runtime_path = self.runtime_path.as_ref()
            .ok_or_else(|| "llama.cpp runtime not found. Place llama-cli executable in local-ai/runtimes/llama-cpp/".to_string())?;

        let model = self.registry.validate_model(&request.model_id)?;

        let max_tokens = request.max_tokens.unwrap_or(512);
        let temperature = request.temperature.unwrap_or(0.7);
        let timeout_ms = request.timeout_ms.unwrap_or(60_000);

        // Build the full prompt with optional system prompt.
        let full_prompt = match &request.system_prompt {
            Some(sys) => format!("<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", sys, request.prompt),
            None => format!("<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n", request.prompt),
        };

        // Build llama-cli command.
        let mut cmd = Command::new(runtime_path);
        cmd.arg("-m")
            .arg(&model.path)
            .arg("-p")
            .arg(&full_prompt)
            .arg("-n")
            .arg(max_tokens.to_string())
            .arg("--temp")
            .arg(format!("{:.2}", temperature))
            .arg("-c")
            .arg(model.context_window.to_string())
            .arg("--no-display-prompt")
            .arg("--log-disable")
            .arg("--offline")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(top_p) = request.top_p {
            cmd.arg("--top-p").arg(format!("{:.2}", top_p));
        }

        let start = Instant::now();

        // Spawn with timeout.
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn llama-cli: {e}"))?;

        // Wait with timeout.
        let timeout = std::time::Duration::from_millis(timeout_ms);
        let result = wait_with_timeout(&mut child, timeout);

        let elapsed_ms = start.elapsed().as_millis() as u64;

        match result {
            WaitResult::Completed(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if !output.status.success() {
                    let detail = summarize_runtime_output(&stderr);
                    let status = output
                        .status
                        .code()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    return Err(if detail.is_empty() {
                        format!("llama.cpp exited with status {status}")
                    } else {
                        format!("llama.cpp exited with status {status}: {detail}")
                    });
                }

                let text = stdout.trim().to_string();
                if text.is_empty() {
                    return Err("llama.cpp completed without generated text".to_string());
                }

                let mut warnings = Vec::new();
                if !stderr.is_empty() && !stderr.contains("llama_") {
                    // Filter out llama.cpp info logs, only keep real warnings.
                    let real_warnings: Vec<&str> = stderr
                        .lines()
                        .filter(|l| {
                            !l.starts_with("llama_")
                                && !l.starts_with("ggml_")
                                && !l.trim().is_empty()
                        })
                        .collect();
                    if !real_warnings.is_empty() {
                        warnings.push(real_warnings.join("\n"));
                    }
                }

                Ok(LocalGenerateResult {
                    model_id: request.model_id,
                    text,
                    tokens_generated: None, // llama-cli doesn't report this easily
                    elapsed_ms,
                    finish_reason: "stop".to_string(),
                    warnings,
                })
            }
            WaitResult::Timeout => {
                // Kill the process.
                let _ = child.kill();
                let _ = child.wait();
                Err(format!("Generation timed out after {}ms", timeout_ms))
            }
            WaitResult::Error(e) => Err(format!("Generation process error: {e}")),
        }
    }
}

fn summarize_runtime_output(output: &str) -> String {
    const MAX_ERROR_BYTES: usize = 1024;
    let normalized = output
        .lines()
        .filter(|line| {
            !line.starts_with("llama_") && !line.starts_with("ggml_") && !line.trim().is_empty()
        })
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.len() <= MAX_ERROR_BYTES {
        return normalized;
    }
    let truncated = normalized.chars().take(MAX_ERROR_BYTES).collect::<String>();
    format!("{truncated}…")
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_workspace_root() -> PathBuf {
    let local_ai_root = resolve_local_ai_root(None);
    workspace_root_from_local_ai_root(&local_ai_root)
}

fn discover_runtime(workspace_root: &Path) -> Option<PathBuf> {
    let runtime_dir = workspace_root.join(RUNTIME_DIR);
    if !runtime_dir.is_dir() {
        return None;
    }

    for name in LLAMA_CLI_NAMES {
        let path = runtime_dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

enum WaitResult {
    Completed(std::process::Output),
    Timeout,
    Error(String),
}

fn wait_with_timeout(child: &mut std::process::Child, timeout: std::time::Duration) -> WaitResult {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited — collect output.
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(mut out) = child.stdout.take() {
                    use std::io::Read;
                    let _ = out.read_to_end(&mut stdout);
                }
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_end(&mut stderr);
                }
                return WaitResult::Completed(std::process::Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) => {
                // Still running — check timeout.
                if start.elapsed() > timeout {
                    return WaitResult::Timeout;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                return WaitResult::Error(e.to_string());
            }
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
    fn runtime_status_reports_missing_runtime() {
        let mut runtime = LocalRuntime::new();
        runtime.runtime_path = None;
        let status = runtime.status();
        assert!(!status.available);
        assert!(status.errors.iter().any(|e| e.contains("not found")));
    }

    #[test]
    fn generate_returns_error_when_runtime_missing() {
        let mut runtime = LocalRuntime::new();
        runtime.runtime_path = None;
        let req = LocalGenerateRequest {
            model_id: "qwen3-4b-q4-k-m".to_string(),
            prompt: "Hello".to_string(),
            system_prompt: None,
            max_tokens: Some(10),
            temperature: None,
            top_p: None,
            timeout_ms: Some(5000),
        };
        let result = runtime.generate(req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("runtime not found"));
    }

    #[test]
    fn generate_returns_error_when_model_missing() {
        let mut runtime = LocalRuntime::new();
        // Set a fake runtime path so it passes the runtime check.
        runtime.runtime_path = Some(PathBuf::from("/fake/llama-cli"));
        // Override registry with a missing model.
        runtime.registry.config = Some(super::super::local_types::ModelsConfig {
            schema_version: 1,
            models: vec![],
        });
        let req = LocalGenerateRequest {
            model_id: "nonexistent".to_string(),
            prompt: "Hello".to_string(),
            system_prompt: None,
            max_tokens: Some(10),
            temperature: None,
            top_p: None,
            timeout_ms: Some(5000),
        };
        let result = runtime.generate(req);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found in registry"));
    }
}
