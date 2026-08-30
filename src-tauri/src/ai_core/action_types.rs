//! AI action types: proposed actions, batches, and validation.

use serde::{Deserialize, Serialize};

use super::rag::Citation;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiActionType {
    AddHighlight,
    AddComment,
    AddTextBox,
    AddRedaction,
    ApplyStamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiActionStatus {
    Proposed,
    Accepted,
    Rejected,
    Applied,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiActionRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiProposedAction {
    pub action_id: String,
    pub action_type: AiActionType,
    pub session_id: String,
    pub page_index: usize,
    pub rect: AiActionRect,
    pub text: String,
    pub reason: String,
    pub confidence: f32,
    pub source_citations: Vec<Citation>,
    pub status: AiActionStatus,
    pub created_at: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiActionBatch {
    pub batch_id: String,
    pub session_id: String,
    pub user_request: String,
    pub actions: Vec<AiProposedAction>,
    pub citations: Vec<Citation>,
    pub warnings: Vec<String>,
    pub created_at: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanActionsRequest {
    pub session_id: String,
    pub user_request: String,
    pub page_count: usize,
    pub top_k: Option<usize>,
}

/// Raw action from LLM JSON output (before validation).
#[derive(Debug, Clone, Deserialize)]
pub struct RawLlmAction {
    pub action_type: Option<String>,
    pub page_index: Option<usize>,
    pub rect: Option<RawRect>,
    pub text: Option<String>,
    pub reason: Option<String>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawRect {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawLlmResponse {
    pub actions: Option<Vec<RawLlmAction>>,
}

/// Validation error for a proposed action.
#[derive(Debug, Clone)]
pub struct ActionValidationError {
    pub field: String,
    pub message: String,
}

impl AiProposedAction {
    /// Validate an action against document constraints.
    pub fn validate(&self, page_count: usize) -> Vec<ActionValidationError> {
        let mut errors = Vec::new();

        if self.page_index >= page_count {
            errors.push(ActionValidationError {
                field: "page_index".to_string(),
                message: format!(
                    "Page {} is out of bounds (document has {} pages)",
                    self.page_index, page_count
                ),
            });
        }

        if self.rect.width <= 0.0 || self.rect.height <= 0.0 {
            errors.push(ActionValidationError {
                field: "rect".to_string(),
                message: "Rect must have positive width and height".to_string(),
            });
        }

        if self.rect.x < 0.0 || self.rect.y < 0.0 {
            errors.push(ActionValidationError {
                field: "rect".to_string(),
                message: "Rect coordinates must be non-negative".to_string(),
            });
        }

        if self.confidence < 0.0 || self.confidence > 1.0 {
            errors.push(ActionValidationError {
                field: "confidence".to_string(),
                message: "Confidence must be between 0.0 and 1.0".to_string(),
            });
        }

        if self.source_citations.is_empty() {
            errors.push(ActionValidationError {
                field: "source_citations".to_string(),
                message: "Document-specific actions must have at least one citation".to_string(),
            });
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_action() -> AiProposedAction {
        AiProposedAction {
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
            text: "Risk clause identified".to_string(),
            reason: "This clause contains termination risk".to_string(),
            confidence: 0.85,
            source_citations: vec![Citation {
                citation_id: "c1".to_string(),
                page_index: 0,
                chunk_id: "chunk-1".to_string(),
                snippet: "termination for convenience".to_string(),
                score: 0.9,
                source: "native_text".to_string(),
            }],
            status: AiActionStatus::Proposed,
            created_at: 0,
        }
    }

    #[test]
    fn valid_action_passes_validation() {
        let action = make_valid_action();
        let errors = action.validate(10);
        assert!(errors.is_empty());
    }

    #[test]
    fn rejects_out_of_bounds_page() {
        let mut action = make_valid_action();
        action.page_index = 99;
        let errors = action.validate(10);
        assert!(errors.iter().any(|e| e.field == "page_index"));
    }

    #[test]
    fn rejects_invalid_rect() {
        let mut action = make_valid_action();
        action.rect.width = -10.0;
        let errors = action.validate(10);
        assert!(errors.iter().any(|e| e.field == "rect"));
    }

    #[test]
    fn rejects_missing_citations() {
        let mut action = make_valid_action();
        action.source_citations.clear();
        let errors = action.validate(10);
        assert!(errors.iter().any(|e| e.field == "source_citations"));
    }

    #[test]
    fn rejects_invalid_confidence() {
        let mut action = make_valid_action();
        action.confidence = 1.5;
        let errors = action.validate(10);
        assert!(errors.iter().any(|e| e.field == "confidence"));
    }
}
