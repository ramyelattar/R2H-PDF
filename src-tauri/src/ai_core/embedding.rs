//! Dense embedding generation using llama-server.exe in embedding mode.
//! The server binds to 127.0.0.1 only (offline, no external network).
//! Falls back gracefully when the server binary or model is not present.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const SERVER_BINARY_NAMES: &[&str] = &["llama-server.exe", "llama-server"];
const EMBEDDING_HOST: &str = "127.0.0.1";
const EMBEDDING_PORT: u16 = 18080;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingVector {
    pub chunk_id: String,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetrievalMode {
    #[serde(rename = "bm25")]
    Bm25,
    #[serde(rename = "dense")]
    Dense,
    #[serde(rename = "hybrid")]
    Hybrid,
}

impl std::fmt::Display for RetrievalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RetrievalMode::Bm25 => write!(f, "bm25"),
            RetrievalMode::Dense => write!(f, "dense"),
            RetrievalMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

pub struct EmbeddingRuntime {
    server_binary: Option<PathBuf>,
    model_path: PathBuf,
    server_process: Option<Child>,
    server_ready: bool,
}

impl EmbeddingRuntime {
    pub fn new(workspace_root: &Path) -> Self {
        let runtime_dir = workspace_root.join("local-ai/runtimes/llama-cpp");
        let server_binary = discover_server_binary(&runtime_dir);
        let model_path = workspace_root.join(
            "local-ai/models/embeddings/Qwen3-Embedding-4B-GGUF/Qwen3-Embedding-4B-Q4_K_M.gguf",
        );

        Self {
            server_binary,
            model_path,
            server_process: None,
            server_ready: false,
        }
    }

    /// Check if dense embedding infrastructure exists (binary + model).
    pub fn is_available(&self) -> bool {
        self.server_binary.is_some() && self.model_path.is_file()
    }

    /// Check if the embedding server is currently running and ready.
    pub fn is_server_ready(&self) -> bool {
        self.server_ready
    }

    /// Get the current retrieval mode based on availability.
    pub fn retrieval_mode(&self) -> RetrievalMode {
        if self.server_ready {
            RetrievalMode::Hybrid
        } else if self.is_available() {
            // Available but server not started yet.
            RetrievalMode::Bm25
        } else {
            RetrievalMode::Bm25
        }
    }

    /// Get a diagnostic message about why dense embeddings are unavailable.
    pub fn fallback_reason(&self) -> Option<String> {
        if self.server_ready {
            return None;
        }
        let mut reasons = Vec::new();
        if self.server_binary.is_none() {
            reasons.push("llama-server.exe not found in local-ai/runtimes/llama-cpp/".to_string());
        }
        if !self.model_path.is_file() {
            reasons.push(format!(
                "Embedding model not found: {}",
                self.model_path.display()
            ));
        }
        if self.server_binary.is_some() && self.model_path.is_file() && !self.server_ready {
            reasons.push("Embedding server not started. Call start_server() first.".to_string());
        }
        if reasons.is_empty() {
            None
        } else {
            Some(reasons.join("; "))
        }
    }

    /// Start the embedding server process.
    pub fn start_server(&mut self) -> Result<(), String> {
        if self.server_ready {
            return Ok(());
        }

        let binary = self
            .server_binary
            .as_ref()
            .ok_or_else(|| "llama-server.exe not found".to_string())?;

        if !self.model_path.is_file() {
            return Err(format!(
                "Embedding model not found: {}",
                self.model_path.display()
            ));
        }

        let child = Command::new(binary)
            .arg("-m")
            .arg(self.model_path.to_string_lossy().as_ref())
            .arg("--embedding")
            .arg("--host")
            .arg(EMBEDDING_HOST)
            .arg("--port")
            .arg(EMBEDDING_PORT.to_string())
            .arg("--offline")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to start embedding server: {e}"))?;

        self.server_process = Some(child);

        // Wait for server to become ready (poll health endpoint).
        let start = Instant::now();
        let timeout = Duration::from_secs(60);
        loop {
            if start.elapsed() > timeout {
                self.stop_server();
                return Err("Embedding server startup timed out after 60s".to_string());
            }

            std::thread::sleep(Duration::from_millis(500));

            if let Ok(resp) = reqwest::blocking::Client::new()
                .get(format!(
                    "http://{}:{}/health",
                    EMBEDDING_HOST, EMBEDDING_PORT
                ))
                .timeout(Duration::from_secs(2))
                .send()
            {
                if resp.status().is_success() {
                    self.server_ready = true;
                    return Ok(());
                }
            }
        }
    }

    /// Stop the embedding server process.
    pub fn stop_server(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.server_ready = false;
    }

    /// Generate embeddings for a batch of texts via the local server.
    pub fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if !self.server_ready {
            return Err("Embedding server not ready. Call start_server() first.".to_string());
        }

        let client = reqwest::blocking::Client::new();
        let url = format!("http://{}:{}/v1/embeddings", EMBEDDING_HOST, EMBEDDING_PORT);

        let body = serde_json::json!({
            "input": texts,
            "model": "embedding"
        });

        let resp = client
            .post(&url)
            .timeout(Duration::from_secs(120))
            .json(&body)
            .send()
            .map_err(|e| format!("Embedding request failed: {e}"))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Embedding server returned status {}",
                resp.status()
            ));
        }

        let json: serde_json::Value = resp
            .json()
            .map_err(|e| format!("Failed to parse embedding response: {e}"))?;

        let data = json
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| "Embedding response missing 'data' array".to_string())?;

        let mut results = Vec::with_capacity(texts.len());
        for item in data {
            let embedding = item
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| "Embedding item missing 'embedding' array".to_string())?;

            let values: Vec<f32> = embedding
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            if values.is_empty() {
                return Err("Embedding vector is empty".to_string());
            }
            results.push(values);
        }

        Ok(results)
    }

    /// Generate embedding for a single query text.
    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>, String> {
        let results = self.embed_texts(&[query.to_string()])?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| "No embedding returned for query".to_string())
    }
}

impl Drop for EmbeddingRuntime {
    fn drop(&mut self) {
        self.stop_server();
    }
}

/// Compute cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

fn discover_server_binary(runtime_dir: &Path) -> Option<PathBuf> {
    if !runtime_dir.is_dir() {
        return None;
    }
    for name in SERVER_BINARY_NAMES {
        let path = runtime_dir.join(name);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &b).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }

    #[test]
    fn retrieval_mode_bm25_when_no_server_binary() {
        let runtime = EmbeddingRuntime {
            server_binary: None,
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            server_process: None,
            server_ready: false,
        };
        assert_eq!(runtime.retrieval_mode(), RetrievalMode::Bm25);
        assert!(!runtime.is_available());
    }

    #[test]
    fn fallback_reason_includes_missing_binary() {
        let runtime = EmbeddingRuntime {
            server_binary: None,
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            server_process: None,
            server_ready: false,
        };
        let reason = runtime.fallback_reason().unwrap();
        assert!(reason.contains("llama-server.exe not found"));
    }

    #[test]
    fn is_available_true_when_binary_and_model_exist() {
        // Use the test file itself as a stand-in for "file exists".
        let runtime = EmbeddingRuntime {
            server_binary: Some(PathBuf::from(file!())),
            model_path: PathBuf::from(file!()),
            server_process: None,
            server_ready: false,
        };
        assert!(runtime.is_available());
    }
}
