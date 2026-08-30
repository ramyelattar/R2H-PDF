use super::types::PerformanceEngineeringStrategyResponse;

pub fn performance_strategy() -> PerformanceEngineeringStrategyResponse {
    PerformanceEngineeringStrategyResponse {
        open_pipeline: vec![
            "Fast metadata pass first, lazy deep parsing after first view".to_string(),
            "Parallelize xref scan and outline extraction".to_string(),
            "Prime first visible page render immediately".to_string(),
            "Defer non-blocking tasks (indexing, compare prep) to background queue".to_string(),
        ],
        render_pipeline: vec![
            "Tile-based rendering with viewport-first prioritization".to_string(),
            "Predictive prefetch for next/previous pages".to_string(),
            "Multi-level cache (tile, page, decode) under memory policy".to_string(),
            "Frame budget target for interaction path; drop detail before missing budget"
                .to_string(),
        ],
        ocr_pipeline: vec![
            "Batched OCR with bounded worker pool".to_string(),
            "Progressive page-level commits for resumability".to_string(),
            "Background scheduling with user-interaction priority override".to_string(),
            "Optional language pack caching by document profile".to_string(),
        ],
        reliability_guards: vec![
            "Adaptive memory pressure control and graceful degradation".to_string(),
            "Safe autosave checkpoints with integrity verification".to_string(),
            "Crash-safe session restore workflow".to_string(),
            "Regression gates for large-file performance before release".to_string(),
        ],
    }
}
