use super::benchmark::{assess, default_scenarios};
use super::corpus::strategy;
use super::memory::MemoryController;
use super::types::{BenchmarkResultInput, MemorySnapshotInput};

#[test]
fn benchmark_plan_contains_large_file_open_target() {
    let scenarios = default_scenarios();
    let open = scenarios.iter().find(|scenario| scenario.id == "open_300_cold");
    assert!(open.is_some());

    let open = open.unwrap();
    let metric = open.targets.iter().find(|target| target.key == "open_ms");
    assert!(metric.is_some());
    assert_eq!(metric.unwrap().target_ms, 2000);
}

#[test]
fn benchmark_assessment_flags_regression_when_above_target() {
    let scenarios = default_scenarios();
    let input = BenchmarkResultInput {
        scenario_id: "open_300_cold".to_string(),
        metric_key: "open_ms".to_string(),
        samples_ms: vec![2100, 2200, 2300, 2400, 2500],
        machine_profile: "test-machine".to_string(),
    };

    let result = assess(input, &scenarios).expect("assessment should work");
    assert!(!result.passed);
    assert!(matches!(result.regression_risk.as_str(), "medium" | "high"));
}

#[test]
fn memory_controller_escalates_under_hard_limit() {
    let memory = MemoryController::new();
    let action = memory.evaluate(MemorySnapshotInput {
        resident_set_mb: 5000,
        page_cache_mb: 1500,
        job_queue_mb: 900,
        open_documents: 4,
        in_flight_renders: 6,
    });

    assert_eq!(action.state, "critical");
    assert!(!action.actions.is_empty());
}

#[test]
fn corpus_strategy_includes_regression_suite_for_large_files() {
    let corpus = strategy();
    assert!(corpus
        .regression_suite
        .iter()
        .any(|test_case| test_case.file_profile == "corpus-large-300p"));
}
