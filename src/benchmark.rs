// Where: src/benchmark.rs
// What: Shared benchmark report shapes for deterministic and live wiki retrieval comparisons.
// Why: Keep benchmark JSON stable while rendering user-facing summaries as benchmark cases.
use serde::{Deserialize, Serialize};

use crate::model::PackMetrics;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkScenario {
    pub name: String,
    pub query: String,
    pub max_sources: usize,
    pub max_tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkMetricsSnapshot {
    pub resolved_sources_count: usize,
    pub queried_sources_count: usize,
    pub selected_evidence_count: usize,
    pub estimated_pack_tokens: usize,
    pub empty_source_count: usize,
    pub source_error_count: usize,
    pub resolve_ms: u64,
    pub query_ms_total: u64,
    pub pack_ms_total: u64,
    pub section_candidate_count: Option<usize>,
    pub document_candidate_count: Option<usize>,
    pub fallback_used: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkStrategyResult {
    pub strategy: String,
    pub metrics: BenchmarkMetricsSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkScenarioReport {
    pub scenario: BenchmarkScenario,
    pub baseline: BenchmarkStrategyResult,
    pub current_deterministic: BenchmarkStrategyResult,
    pub current_live: Option<BenchmarkStrategyResult>,
    pub live_skipped: bool,
    pub improved_sources: bool,
    pub improved_tokens: bool,
    pub quality_guard_passed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkSuiteReport {
    pub scenarios: Vec<BenchmarkScenarioReport>,
}

pub fn scenario_report(
    scenario: BenchmarkScenario,
    baseline: BenchmarkStrategyResult,
    current_deterministic: BenchmarkStrategyResult,
    current_live: Option<BenchmarkStrategyResult>,
    quality_guard_passed: bool,
) -> BenchmarkScenarioReport {
    let comparison = current_live
        .as_ref()
        .unwrap_or(&current_deterministic)
        .metrics
        .clone();
    BenchmarkScenarioReport {
        scenario,
        improved_sources: comparison.queried_sources_count
            <= baseline.metrics.queried_sources_count,
        improved_tokens: comparison.estimated_pack_tokens <= baseline.metrics.estimated_pack_tokens,
        quality_guard_passed,
        baseline,
        current_deterministic,
        live_skipped: current_live.is_none(),
        current_live,
    }
}

pub fn markdown_summary(report: &BenchmarkSuiteReport) -> String {
    let mut lines = vec![
        "| benchmark case | baseline sources | current sources | baseline tokens | current tokens | current docs | current fallback | live sources | live tokens | verdict |".to_string(),
        "| --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |".to_string(),
    ];

    for scenario in &report.scenarios {
        let live_sources = scenario
            .current_live
            .as_ref()
            .map(|result| result.metrics.queried_sources_count.to_string())
            .unwrap_or_else(|| "skipped".to_string());
        let live_tokens = scenario
            .current_live
            .as_ref()
            .map(|result| result.metrics.estimated_pack_tokens.to_string())
            .unwrap_or_else(|| "skipped".to_string());
        let current_docs = scenario
            .current_deterministic
            .metrics
            .document_candidate_count
            .map(|count| count.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let current_fallback = scenario
            .current_deterministic
            .metrics
            .fallback_used
            .map(|value| if value { "yes" } else { "no" }.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            scenario.scenario.name,
            scenario.baseline.metrics.queried_sources_count,
            scenario.current_deterministic.metrics.queried_sources_count,
            scenario.baseline.metrics.estimated_pack_tokens,
            scenario.current_deterministic.metrics.estimated_pack_tokens,
            current_docs,
            current_fallback,
            live_sources,
            live_tokens,
            verdict_label(scenario),
        ));
    }

    lines.join("\n")
}

fn verdict_label(report: &BenchmarkScenarioReport) -> &'static str {
    if report.quality_guard_passed && (report.improved_sources || report.improved_tokens) {
        "pass"
    } else if report.quality_guard_passed {
        "guard-only"
    } else {
        "fail"
    }
}

pub fn strategy_result(strategy: &str, metrics: &PackMetrics) -> BenchmarkStrategyResult {
    BenchmarkStrategyResult {
        strategy: strategy.to_string(),
        metrics: metrics_snapshot(metrics),
    }
}

pub fn strategy_result_with_retrieval(
    strategy: &str,
    metrics: &PackMetrics,
    section_candidate_count: usize,
    document_candidate_count: usize,
    fallback_used: bool,
) -> BenchmarkStrategyResult {
    let mut snapshot = metrics_snapshot(metrics);
    snapshot.section_candidate_count = Some(section_candidate_count);
    snapshot.document_candidate_count = Some(document_candidate_count);
    snapshot.fallback_used = Some(fallback_used);
    BenchmarkStrategyResult {
        strategy: strategy.to_string(),
        metrics: snapshot,
    }
}

pub fn metrics_snapshot(metrics: &PackMetrics) -> BenchmarkMetricsSnapshot {
    BenchmarkMetricsSnapshot {
        resolved_sources_count: metrics.resolved_sources_count,
        queried_sources_count: metrics.queried_sources_count,
        selected_evidence_count: metrics.selected_evidence_count,
        estimated_pack_tokens: metrics.estimated_pack_tokens,
        empty_source_count: metrics.empty_source_count,
        source_error_count: metrics.source_error_count,
        resolve_ms: metrics.resolve_ms,
        query_ms_total: metrics.query_ms_total,
        pack_ms_total: metrics.pack_ms_total,
        section_candidate_count: None,
        document_candidate_count: None,
        fallback_used: None,
    }
}
