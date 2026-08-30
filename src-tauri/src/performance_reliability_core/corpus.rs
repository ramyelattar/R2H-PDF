use super::types::{RegressionTestCase, TestCorpusAsset, TestCorpusStrategyResponse};

pub fn strategy() -> TestCorpusStrategyResponse {
    let assets = vec![
        TestCorpusAsset {
            id: "corpus-large-300p".to_string(),
            class_name: "Large structured contract".to_string(),
            page_count: 300,
            characteristics: vec![
                "dense_text".to_string(),
                "cross_references".to_string(),
                "bookmarks".to_string(),
            ],
            sensitive: true,
        },
        TestCorpusAsset {
            id: "corpus-scan-100p".to_string(),
            class_name: "Scanned image-heavy dossier".to_string(),
            page_count: 100,
            characteristics: vec![
                "raster".to_string(),
                "skewed_pages".to_string(),
                "ocr_noise".to_string(),
            ],
            sensitive: true,
        },
        TestCorpusAsset {
            id: "corpus-mixed-graphics".to_string(),
            class_name: "Mixed vector + bitmap engineering package".to_string(),
            page_count: 180,
            characteristics: vec![
                "vector_layers".to_string(),
                "large_images".to_string(),
                "overprint".to_string(),
            ],
            sensitive: false,
        },
    ];

    let regression_suite = vec![
        RegressionTestCase {
            id: "reg-open-latency".to_string(),
            name: "Open latency for 300-page corpus".to_string(),
            file_profile: "corpus-large-300p".to_string(),
            critical_metrics: vec!["open_ms".to_string()],
            max_regression_percent: 10.0,
        },
        RegressionTestCase {
            id: "reg-nav-jank".to_string(),
            name: "Navigation jank under rapid paging".to_string(),
            file_profile: "corpus-large-300p".to_string(),
            critical_metrics: vec!["nav_frame_budget_ms".to_string()],
            max_regression_percent: 8.0,
        },
        RegressionTestCase {
            id: "reg-ocr-throughput".to_string(),
            name: "OCR throughput for 100-page scan".to_string(),
            file_profile: "corpus-scan-100p".to_string(),
            critical_metrics: vec!["ocr_total_ms".to_string()],
            max_regression_percent: 15.0,
        },
    ];

    let governance_rules = vec![
        "Use synthetic or rights-cleared documents only for distribution.".to_string(),
        "Keep sensitive corpora encrypted at rest and excluded from telemetry payloads.".to_string(),
        "Track checksum/version for every benchmark corpus artifact.".to_string(),
        "Run full regression suite before release candidate promotion.".to_string(),
    ];

    TestCorpusStrategyResponse {
        assets,
        governance_rules,
        regression_suite,
    }
}
