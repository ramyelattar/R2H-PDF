use super::errors::PlatformLayerError;
use super::types::{BenchmarkAssessment, BenchmarkResultInput, BenchmarkScenario, BenchmarkTarget};

pub fn default_scenarios() -> Vec<BenchmarkScenario> {
    vec![
        BenchmarkScenario {
            id: "open_300_cold".to_string(),
            name: "Open 300-page file cold".to_string(),
            corpus_ref: "corpus-large-300p".to_string(),
            targets: vec![BenchmarkTarget {
                key: "open_ms".to_string(),
                description: "Time to open and produce first actionable view state".to_string(),
                target_ms: 2000,
                p95_target_ms: 2200,
            }],
            warmup_runs: 1,
            measured_runs: 9,
        },
        BenchmarkScenario {
            id: "nav_smoothness".to_string(),
            name: "Sequential page navigation smoothness".to_string(),
            corpus_ref: "corpus-large-300p".to_string(),
            targets: vec![BenchmarkTarget {
                key: "nav_frame_budget_ms".to_string(),
                description: "P95 latency per navigation event under prefetch".to_string(),
                target_ms: 16,
                p95_target_ms: 24,
            }],
            warmup_runs: 1,
            measured_runs: 200,
        },
        BenchmarkScenario {
            id: "zoom_render_latency".to_string(),
            name: "Zoom and render latency".to_string(),
            corpus_ref: "corpus-mixed-graphics".to_string(),
            targets: vec![BenchmarkTarget {
                key: "zoom_render_ms".to_string(),
                description: "P95 tile ready latency across 0.75x..4x".to_string(),
                target_ms: 75,
                p95_target_ms: 120,
            }],
            warmup_runs: 2,
            measured_runs: 60,
        },
        BenchmarkScenario {
            id: "ocr_100_pages".to_string(),
            name: "OCR 100-page batch throughput".to_string(),
            corpus_ref: "corpus-scan-100p".to_string(),
            targets: vec![BenchmarkTarget {
                key: "ocr_total_ms".to_string(),
                description: "Total OCR processing time for 100-page batch".to_string(),
                target_ms: 180000,
                p95_target_ms: 210000,
            }],
            warmup_runs: 0,
            measured_runs: 3,
        },
    ]
}

pub fn assess(
    input: BenchmarkResultInput,
    scenarios: &[BenchmarkScenario],
) -> Result<BenchmarkAssessment, PlatformLayerError> {
    let scenario = scenarios
        .iter()
        .find(|scenario| scenario.id == input.scenario_id)
        .ok_or_else(|| PlatformLayerError::NotFound(format!("scenario {}", input.scenario_id)))?;

    let target = scenario
        .targets
        .iter()
        .find(|target| target.key == input.metric_key)
        .ok_or_else(|| {
            PlatformLayerError::NotFound(format!(
                "metric {} in scenario {}",
                input.metric_key, input.scenario_id
            ))
        })?;

    if input.samples_ms.is_empty() {
        return Err(PlatformLayerError::InvalidInput(
            "benchmark samples cannot be empty".to_string(),
        ));
    }

    let mut samples = input.samples_ms;
    samples.sort_unstable();

    let p50_index = samples.len() / 2;
    let p95_index = ((samples.len() as f32) * 0.95).floor() as usize;
    let p95_index = p95_index.min(samples.len().saturating_sub(1));

    let p50_ms = samples[p50_index];
    let p95_ms = samples[p95_index];

    let passed = p50_ms <= target.target_ms && p95_ms <= target.p95_target_ms;
    let regression_risk = if passed {
        "low".to_string()
    } else if p95_ms <= target.p95_target_ms.saturating_mul(12) / 10 {
        "medium".to_string()
    } else {
        "high".to_string()
    };

    Ok(BenchmarkAssessment {
        scenario_id: scenario.id.clone(),
        metric_key: target.key.clone(),
        p50_ms,
        p95_ms,
        target_ms: target.target_ms,
        p95_target_ms: target.p95_target_ms,
        passed,
        regression_risk,
    })
}
