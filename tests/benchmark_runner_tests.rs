// Where: tests/benchmark_runner_tests.rs
// What: JSON and markdown benchmark report tests for deterministic hybrid retrieval comparisons.
// Why: Keep a machine-readable comparison artifact while rendering summaries as benchmark cases.
mod common_benchmark;

use kinic_context_cli::benchmark::{BenchmarkSuiteReport, markdown_summary};

use common_benchmark::deterministic_suite_report;

#[tokio::test]
async fn benchmark_report_serializes_as_json_and_markdown() {
    let report = deterministic_suite_report().await;

    let encoded = serde_json::to_string_pretty(&report).expect("benchmark report should encode");
    let decoded: BenchmarkSuiteReport =
        serde_json::from_str(&encoded).expect("benchmark report should decode");
    assert_eq!(decoded.scenarios.len(), 8);
    assert!(
        decoded
            .scenarios
            .iter()
            .all(|scenario| scenario.live_skipped)
    );
    assert!(
        decoded
            .scenarios
            .iter()
            .all(|scenario| scenario.quality_guard_passed)
    );
    assert!(
        decoded
            .scenarios
            .iter()
            .filter(|scenario| scenario.improved_sources || scenario.improved_tokens)
            .count()
            >= 2
    );

    let markdown = markdown_summary(&decoded);
    assert!(markdown.contains("| benchmark case |"));
    assert!(markdown.contains("two-source-auth"));
    assert!(markdown.contains("exact-middleware"));
    assert!(markdown.contains("migration-version"));
    assert!(markdown.contains("vector-natural-language"));
    assert!(markdown.contains("skipped"));
    assert!(markdown.contains("live sources"));
    assert!(markdown.contains("verdict"));
}
