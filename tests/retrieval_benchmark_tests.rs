// Where: tests/retrieval_benchmark_tests.rs
// What: Direct retrieval benchmark tests against the fake SQL-backed canister logic.
// Why: Compare baseline and current heuristics without going through the pack orchestration layer.
mod common_retrieval_benchmark;

use common_retrieval_benchmark::{retrieval_fixtures, retrieval_scenario_report};

#[test]
fn retrieval_benchmark_preserves_exact_and_migration_top_hits() {
    for fixture in retrieval_fixtures()
        .into_iter()
        .filter(|fixture| matches!(fixture.scenario.name.as_str(), "exact-middleware" | "migration-version"))
    {
        let report = retrieval_scenario_report(&fixture);
        assert!(report.quality_guard_passed);
    }
}

#[test]
fn retrieval_benchmark_improves_multiple_cases() {
    let improved_cases = retrieval_fixtures()
        .iter()
        .map(retrieval_scenario_report)
        .filter(|report| report.improved_canisters || report.improved_tokens)
        .count();
    assert!(improved_cases >= 2);
}

#[test]
fn retrieval_benchmark_reduces_visible_weak_cases() {
    let weak_cases = retrieval_fixtures()
        .iter()
        .map(retrieval_scenario_report)
        .filter(|report| !report.quality_guard_passed)
        .count();
    assert_eq!(weak_cases, 0);
}
