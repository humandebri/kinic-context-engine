// Where: tests/common_benchmark.rs
// What: Shared deterministic benchmark fixtures and baseline calculations.
// Why: Keep benchmark tests and report tests aligned on the same scenarios and expected comparisons.
use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use kinic_context_cli::{
    benchmark::{
        BenchmarkScenario, BenchmarkScenarioReport, BenchmarkStrategyResult, BenchmarkSuiteReport,
        scenario_report, strategy_result,
    },
    catalog::SourceCatalog,
    engine::ContextEngine,
    model::{CommandOutput, PackMetrics, ResolvedSource, SourceMetadata, SourceSnippet},
    pack::estimate_pack_tokens,
    provider::SourceQueryProvider,
};
use kinic_context_core::types::FilterSourcesArgs;

#[derive(Clone)]
pub struct MockCatalog {
    pub sources: BTreeMap<String, SourceMetadata>,
    pub resolved: Vec<ResolvedSource>,
}

impl SourceCatalog for MockCatalog {
    async fn get_source(&self, source_id: &str) -> Result<SourceMetadata> {
        self.sources
            .get(source_id)
            .cloned()
            .ok_or_else(|| anyhow!("unknown source_id: {source_id}"))
    }

    async fn resolve_sources(&self, _query: &str, limit: usize) -> Result<Vec<ResolvedSource>> {
        Ok(self.resolved.iter().take(limit).cloned().collect())
    }

    async fn list_sources(&self) -> Result<Vec<SourceMetadata>> {
        Ok(self.sources.values().cloned().collect())
    }

    async fn filter_sources(&self, _args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>> {
        Ok(self.sources.values().cloned().collect())
    }
}

#[derive(Clone)]
pub struct MockProvider {
    pub responses: BTreeMap<String, Vec<SourceSnippet>>,
}

impl SourceQueryProvider for MockProvider {
    async fn query(
        &self,
        source: SourceMetadata,
        _query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SourceSnippet>> {
        let mut snippets = self
            .responses
            .get(&source.source_id)
            .cloned()
            .unwrap_or_default();
        snippets.retain(|item| version.is_none() || item.version.as_deref() == version);
        snippets.truncate(top_k);
        Ok(snippets)
    }
}

#[derive(Clone)]
pub struct DeterministicFixture {
    pub scenario: BenchmarkScenario,
    pub catalog: MockCatalog,
    pub provider: MockProvider,
    pub expected_top_title: Option<String>,
}

pub fn source(source_id: &str, title: &str, canister_ids: Vec<&str>) -> SourceMetadata {
    SourceMetadata {
        source_id: source_id.to_string(),
        title: title.to_string(),
        aliases: Vec::new(),
        trust: "official".to_string(),
        domain: "code_docs".to_string(),
        skill_kind: None,
        targets: Vec::new(),
        capabilities: Vec::new(),
        canister_ids: canister_ids.into_iter().map(ToString::to_string).collect(),
        supported_versions: vec!["1".to_string()],
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        citations: vec![format!("https://example.com{source_id}")],
    }
}

pub fn snippet(source_id: &str, title: &str, score: f32) -> SourceSnippet {
    SourceSnippet {
        source_id: source_id.to_string(),
        title: title.to_string(),
        snippet: format!("snippet for {title}"),
        citation: format!("https://example.com/{title}"),
        trust: "official".to_string(),
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        version: Some("1".to_string()),
        stale: false,
        score,
    }
}

pub fn deterministic_fixtures() -> Vec<DeterministicFixture> {
    vec![
        two_source_auth_fixture(),
        ambiguous_hooks_fixture(),
        budget_fixture(),
        exact_lookup_fixture(),
        migration_fixture(),
        versioned_exact_fixture(),
        vector_natural_language_fixture(),
        fallback_noise_fixture(),
    ]
}

#[allow(dead_code)]
pub async fn current_report(fixture: &DeterministicFixture) -> BenchmarkStrategyResult {
    let output = current_pack_output(fixture).await;
    strategy_result(
        "candidate-narrowed",
        &output.metrics.expect("pack metrics should exist"),
    )
}

pub async fn current_pack_output(
    fixture: &DeterministicFixture,
) -> kinic_context_cli::model::EvidencePack {
    let engine = ContextEngine::new(fixture.catalog.clone(), fixture.provider.clone());
    let CommandOutput::Pack(output) = engine
        .pack(
            &fixture.scenario.query,
            fixture.scenario.max_sources,
            fixture.scenario.max_tokens,
        )
        .await
        .expect("deterministic pack should succeed")
    else {
        panic!("expected pack output");
    };
    output
}

pub fn baseline_report(fixture: &DeterministicFixture) -> BenchmarkStrategyResult {
    let resolved = fixture
        .catalog
        .resolved
        .iter()
        .take(fixture.scenario.max_sources)
        .cloned()
        .collect::<Vec<_>>();
    let mut evidence = Vec::new();
    let mut empty_source_count = 0_usize;
    let mut queried_canisters_count = 0_usize;

    for resolved_source in &resolved {
        let source = fixture
            .catalog
            .sources
            .get(&resolved_source.source_id)
            .expect("source should exist");
        queried_canisters_count += source.canister_ids.len();
        let mut snippets = fixture
            .provider
            .responses
            .get(&resolved_source.source_id)
            .cloned()
            .unwrap_or_default();
        snippets.truncate(3);
        if snippets.is_empty() {
            empty_source_count += 1;
        }
        evidence.extend(snippets);
    }

    evidence.sort_by(|left, right| right.score.total_cmp(&left.score));
    let selected_evidence = trim_to_budget(evidence, fixture.scenario.max_tokens);
    BenchmarkStrategyResult {
        strategy: "baseline".to_string(),
        metrics: kinic_context_cli::benchmark::BenchmarkMetricsSnapshot {
            resolved_sources_count: resolved.len(),
            queried_canisters_count,
            selected_evidence_count: selected_evidence.len(),
            estimated_pack_tokens: estimate_pack_tokens(&selected_evidence),
            empty_source_count,
            source_error_count: 0,
            resolve_ms: 0,
            query_ms_total: 0,
            pack_ms_total: 0,
            section_candidate_count: None,
            document_candidate_count: None,
            fallback_used: None,
        },
    }
}

#[allow(dead_code)]
pub async fn deterministic_scenario_report(
    fixture: &DeterministicFixture,
) -> BenchmarkScenarioReport {
    let output = current_pack_output(fixture).await;
    let quality_guard_passed = quality_guard_passed(fixture, &output);
    scenario_report(
        fixture.scenario.clone(),
        baseline_report(fixture),
        strategy_result(
            "candidate-narrowed",
            &output.metrics.clone().expect("pack metrics should exist"),
        ),
        None,
        quality_guard_passed,
    )
}

#[allow(dead_code)]
pub async fn deterministic_suite_report() -> BenchmarkSuiteReport {
    let mut scenarios = Vec::new();
    for fixture in deterministic_fixtures() {
        scenarios.push(deterministic_scenario_report(&fixture).await);
    }
    BenchmarkSuiteReport { scenarios }
}

#[allow(dead_code)]
pub fn metrics_from_report(report: &BenchmarkScenarioReport) -> PackMetrics {
    let metrics = &report.current_deterministic.metrics;
    PackMetrics {
        resolved_sources_count: metrics.resolved_sources_count,
        queried_canisters_count: metrics.queried_canisters_count,
        returned_snippets_count: 0,
        selected_evidence_count: metrics.selected_evidence_count,
        estimated_pack_tokens: metrics.estimated_pack_tokens,
        empty_source_count: metrics.empty_source_count,
        source_error_count: metrics.source_error_count,
        resolve_ms: metrics.resolve_ms,
        query_ms_total: metrics.query_ms_total,
        pack_ms_total: metrics.pack_ms_total,
    }
}

pub fn quality_guard_passed(
    fixture: &DeterministicFixture,
    output: &kinic_context_cli::model::EvidencePack,
) -> bool {
    fixture.expected_top_title.as_ref().is_none_or(|expected| {
        output
            .evidence
            .first()
            .is_some_and(|item| &item.title == expected)
    })
}

fn trim_to_budget(evidence: Vec<SourceSnippet>, max_tokens: usize) -> Vec<SourceSnippet> {
    if max_tokens == 0 {
        return Vec::new();
    }
    let mut selected = Vec::new();
    let mut used_tokens = 0_usize;
    for snippet in evidence {
        let snippet_tokens = estimate_pack_tokens(std::slice::from_ref(&snippet));
        if used_tokens.saturating_add(snippet_tokens) > max_tokens {
            continue;
        }
        used_tokens = used_tokens.saturating_add(snippet_tokens);
        selected.push(snippet);
    }
    selected
}

fn two_source_auth_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "two-source-auth".to_string(),
            query: "protect route in next.js with supabase auth".to_string(),
            max_sources: 3,
            max_tokens: 3000,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a", "b"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["c"]),
                ),
                (
                    "/react/docs".to_string(),
                    source("/react/docs", "React Docs", vec!["d"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.2,
                    reasons: vec!["matched next".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.9,
                    reasons: vec!["matched auth".to_string()],
                },
                ResolvedSource {
                    source_id: "/react/docs".to_string(),
                    title: "React Docs".to_string(),
                    score: 0.2,
                    reasons: vec!["weak code fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Middleware", 2.0)],
                ),
                (
                    "/supabase/docs".to_string(),
                    vec![snippet("/supabase/docs", "Supabase Auth", 1.5)],
                ),
                ("/react/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Next.js Middleware".to_string()),
    }
}

fn ambiguous_hooks_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "ambiguous-hooks".to_string(),
            query: "next react hooks".to_string(),
            max_sources: 3,
            max_tokens: 3000,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a"]),
                ),
                (
                    "/react/docs".to_string(),
                    source("/react/docs", "React Docs", vec!["b"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["c"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.0,
                    reasons: vec!["matched next".to_string()],
                },
                ResolvedSource {
                    source_id: "/react/docs".to_string(),
                    title: "React Docs".to_string(),
                    score: 0.55,
                    reasons: vec!["matched hooks".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.2,
                    reasons: vec!["fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Hook Usage", 1.1)],
                ),
                ("/react/docs".to_string(), Vec::new()),
                ("/supabase/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Next.js Hook Usage".to_string()),
    }
}

fn budget_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "budget-routing".to_string(),
            query: "protect route".to_string(),
            max_sources: 2,
            max_tokens: 500,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["b"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.0,
                    reasons: vec!["matched next".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.8,
                    reasons: vec!["matched auth".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![
                        snippet("/vercel/next.js", "Next A", 3.0),
                        snippet("/vercel/next.js", "Next B", 2.5),
                        snippet("/vercel/next.js", "Next C", 2.0),
                    ],
                ),
                (
                    "/supabase/docs".to_string(),
                    vec![
                        snippet("/supabase/docs", "Supabase A", 2.8),
                        snippet("/supabase/docs", "Supabase B", 2.4),
                        snippet("/supabase/docs", "Supabase C", 2.1),
                    ],
                ),
            ]),
        },
        expected_top_title: Some("Next A".to_string()),
    }
}

fn exact_lookup_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "exact-middleware".to_string(),
            query: "middleware cookies".to_string(),
            max_sources: 2,
            max_tokens: 1200,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a"]),
                ),
                (
                    "/react/docs".to_string(),
                    source("/react/docs", "React Docs", vec!["b"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.4,
                    reasons: vec!["matched middleware".to_string()],
                },
                ResolvedSource {
                    source_id: "/react/docs".to_string(),
                    title: "React Docs".to_string(),
                    score: 0.1,
                    reasons: vec!["weak fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Middleware", 2.2)],
                ),
                ("/react/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Next.js Middleware".to_string()),
    }
}

fn migration_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "migration-version".to_string(),
            query: "migration breaking changes".to_string(),
            max_sources: 2,
            max_tokens: 1800,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a", "b"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["c"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.3,
                    reasons: vec!["matched migration".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.2,
                    reasons: vec!["weak fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Upgrade Guide", 2.1)],
                ),
                ("/supabase/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Next.js Upgrade Guide".to_string()),
    }
}

fn versioned_exact_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "versioned-exact".to_string(),
            query: "middleware cookies v15".to_string(),
            max_sources: 2,
            max_tokens: 1200,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["b"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 1.2,
                    reasons: vec!["matched version".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.1,
                    reasons: vec!["weak fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Middleware", 2.3)],
                ),
                ("/supabase/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Next.js Middleware".to_string()),
    }
}

fn vector_natural_language_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "vector-natural-language".to_string(),
            query: "how do i keep auth state fresh before rendering protected routes".to_string(),
            max_sources: 3,
            max_tokens: 1600,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["a"]),
                ),
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["b"]),
                ),
                (
                    "/react/docs".to_string(),
                    source("/react/docs", "React Docs", vec!["c"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 1.1,
                    reasons: vec!["matched auth".to_string()],
                },
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 0.7,
                    reasons: vec!["matched rendering".to_string()],
                },
                ResolvedSource {
                    source_id: "/react/docs".to_string(),
                    title: "React Docs".to_string(),
                    score: 0.1,
                    reasons: vec!["weak fallback".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                (
                    "/supabase/docs".to_string(),
                    vec![snippet("/supabase/docs", "Supabase Next.js Auth", 2.4)],
                ),
                (
                    "/vercel/next.js".to_string(),
                    vec![snippet("/vercel/next.js", "Next.js Middleware", 1.2)],
                ),
                ("/react/docs".to_string(), Vec::new()),
            ]),
        },
        expected_top_title: Some("Supabase Next.js Auth".to_string()),
    }
}

fn fallback_noise_fixture() -> DeterministicFixture {
    DeterministicFixture {
        scenario: BenchmarkScenario {
            name: "fallback-noise".to_string(),
            query: "next launchagent auth".to_string(),
            max_sources: 3,
            max_tokens: 1400,
        },
        catalog: MockCatalog {
            sources: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    source("/vercel/next.js", "Next.js Docs", vec!["a"]),
                ),
                (
                    "/react/docs".to_string(),
                    source("/react/docs", "React Docs", vec!["b"]),
                ),
                (
                    "/supabase/docs".to_string(),
                    source("/supabase/docs", "Supabase Docs", vec!["c"]),
                ),
            ]),
            resolved: vec![
                ResolvedSource {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Docs".to_string(),
                    score: 0.9,
                    reasons: vec!["matched next".to_string()],
                },
                ResolvedSource {
                    source_id: "/react/docs".to_string(),
                    title: "React Docs".to_string(),
                    score: 0.8,
                    reasons: vec!["matched launchagent".to_string()],
                },
                ResolvedSource {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Docs".to_string(),
                    score: 0.4,
                    reasons: vec!["matched auth".to_string()],
                },
            ],
        },
        provider: MockProvider {
            responses: BTreeMap::from([
                ("/vercel/next.js".to_string(), Vec::new()),
                (
                    "/react/docs".to_string(),
                    vec![snippet("/react/docs", "Tailscale LaunchAgent", 1.9)],
                ),
                (
                    "/supabase/docs".to_string(),
                    vec![snippet("/supabase/docs", "Supabase Next.js Auth", 1.2)],
                ),
            ]),
        },
        expected_top_title: Some("Tailscale LaunchAgent".to_string()),
    }
}
