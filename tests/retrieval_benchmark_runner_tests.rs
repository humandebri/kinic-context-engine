// Where: tests/retrieval_benchmark_runner_tests.rs
// What: JSON and markdown report tests for direct retrieval benchmark comparisons.
// Why: Keep retrieval heuristic comparison on the same report schema as pack-level benchmarks.
mod common_retrieval_benchmark;

use kinic_context_cli::benchmark::{BenchmarkSuiteReport, markdown_summary};

use common_retrieval_benchmark::retrieval_suite_report;

#[test]
fn retrieval_report_serializes_as_json_and_markdown() {
    let report = retrieval_suite_report();
    let encoded = serde_json::to_string_pretty(&report).expect("retrieval report should encode");
    let decoded: BenchmarkSuiteReport =
        serde_json::from_str(&encoded).expect("retrieval report should decode");
    assert_eq!(decoded.scenarios.len(), 5);
    assert!(decoded.scenarios.iter().all(|scenario| scenario.pocket_ic_skipped));
    assert!(
        decoded
            .scenarios
            .iter()
            .filter(|scenario| scenario.quality_guard_passed)
            .count()
            == decoded.scenarios.len()
    );
    assert!(
        decoded
            .scenarios
            .iter()
            .filter(|scenario| scenario.improved_canisters || scenario.improved_tokens)
            .count()
            >= 2
    );
    assert!(decoded.scenarios.iter().all(|scenario| scenario
        .current_deterministic
        .metrics
        .document_candidate_count
        .is_some()));
    assert!(decoded.scenarios.iter().all(|scenario| scenario
        .current_deterministic
        .metrics
        .fallback_used
        .is_some()));

    let markdown = markdown_summary(&decoded);
    assert!(markdown.contains("exact-middleware"));
    assert!(markdown.contains("ambiguous-hooks"));
    assert!(markdown.contains("vector-natural-language"));
    assert!(markdown.contains("fallback-noise"));
    assert!(markdown.contains("current docs"));
    assert!(markdown.contains("current fallback"));
    assert!(markdown.contains("verdict"));
}
