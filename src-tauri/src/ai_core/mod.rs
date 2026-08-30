// Stub modules and early-development code still emit warnings;
// allow-list is kept intentional until each module is fully wired.
#![allow(dead_code, unused_imports, unused_variables)]

pub mod action_planner;
pub mod action_types;
pub mod chunking;
pub mod embedding;
pub mod engine;
pub mod errors;
pub mod ipc;
pub mod local_ai_paths;
pub mod local_runtime;
pub mod local_types;
pub mod model_registry;
pub mod rag;
pub mod review;
pub mod types;
pub mod vector_index;

pub use engine::AiEngine;
pub use ipc::*;
pub use local_runtime::LocalRuntime;
pub use rag::RagState;
pub use action_planner::ActionPlannerState;
pub use types::*;

pub type AiCoreState = tokio::sync::Mutex<AiEngine>;

/// State for the local AI runtime (model registry + llama.cpp).
pub struct LocalAiState {
    pub runtime: std::sync::Mutex<LocalRuntime>,
}

impl LocalAiState {
    pub fn new() -> Self {
        Self {
            runtime: std::sync::Mutex::new(LocalRuntime::new()),
        }
    }
}
