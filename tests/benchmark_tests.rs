// Where: tests/benchmark_tests.rs
// What: Deterministic benchmark-style tests for pack fan-out and retrieval budgeting.
// Why: Prove efficiency changes with stable fixtures before relying on PocketIC or live canisters.
mod common_benchmark;

use common_benchmark::{
    baseline_report, current_pack_output, current_report, deterministic_fixtures,
    quality_guard_passed,
};

#[tokio::test]
async fn pack_benchmark_shows_tighter_selection_than_baseline() {
    let fixture = deterministic_fixtures()
        .into_iter()
        .find(|fixture| fixture.scenario.name == "two-source-auth")
        .expect("two-source fixture should exist");

    let baseline = baseline_report(&fixture);
    let current = current_report(&fixture).await;
    assert!(
        current.metrics.queried_canisters_count < baseline.metrics.queried_canisters_count
    );
}

#[tokio::test]
async fn pack_benchmark_scales_retrieval_depth_with_token_budget() {
    let fixture = deterministic_fixtures()
        .into_iter()
        .find(|fixture| fixture.scenario.name == "budget-routing")
        .expect("budget fixture should exist");

    let baseline = baseline_report(&fixture);
    let current = current_report(&fixture).await;
    assert!(
        baseline.metrics.selected_evidence_count >= current.metrics.selected_evidence_count
    );
    assert!(current.metrics.estimated_pack_tokens <= baseline.metrics.estimated_pack_tokens);
}

#[tokio::test]
async fn pack_benchmark_keeps_exact_lookup_tight() {
    let fixture = deterministic_fixtures()
        .into_iter()
        .find(|fixture| fixture.scenario.name == "exact-middleware")
        .expect("exact fixture should exist");

    let baseline = baseline_report(&fixture);
    let current = current_report(&fixture).await;
    assert!(current.metrics.estimated_pack_tokens <= baseline.metrics.estimated_pack_tokens);
}

#[tokio::test]
async fn pack_benchmark_preserves_migration_case() {
    let fixture = deterministic_fixtures()
        .into_iter()
        .find(|fixture| fixture.scenario.name == "migration-version")
        .expect("migration fixture should exist");

    let baseline = baseline_report(&fixture);
    let current = current_report(&fixture).await;
    assert!(current.metrics.selected_evidence_count >= 1);
    assert!(current.metrics.estimated_pack_tokens <= baseline.metrics.estimated_pack_tokens);
}

#[tokio::test]
async fn pack_benchmark_improves_multiple_cases() {
    let mut improved_cases = 0_usize;
    for fixture in deterministic_fixtures() {
        let baseline = baseline_report(&fixture);
        let current = current_report(&fixture).await;
        if current.metrics.queried_canisters_count < baseline.metrics.queried_canisters_count
            || current.metrics.estimated_pack_tokens < baseline.metrics.estimated_pack_tokens
        {
            improved_cases += 1;
        }
    }
    assert!(improved_cases >= 2);
}

#[tokio::test]
async fn pack_benchmark_quality_guards_hold_for_exact_and_migration() {
    for scenario_name in ["exact-middleware", "migration-version", "versioned-exact"] {
        let fixture = deterministic_fixtures()
            .into_iter()
            .find(|fixture| fixture.scenario.name == scenario_name)
            .expect("guard fixture should exist");
        let output = current_pack_output(&fixture).await;
        assert!(quality_guard_passed(&fixture, &output));
    }
}
