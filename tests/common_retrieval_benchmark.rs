// Where: tests/common_retrieval_benchmark.rs
// What: Direct fake canister retrieval benchmark fixtures and report helpers.
// Why: Measure retrieval heuristics directly instead of only pack-layer orchestration behavior.
use fake_memory_instance::{BenchmarkPolicyMode, RetrievalBenchmarkResult, evaluate_query_for_benchmark};
use kinic_context_cli::{
    benchmark::{
        BenchmarkScenario, BenchmarkScenarioReport, BenchmarkSuiteReport, scenario_report,
        strategy_result_with_retrieval,
    },
    model::{PackMetrics, SourceSnippet},
    pack::estimate_pack_tokens,
};
use kinic_context_core::types::{HybridQueryFilters, HybridQueryRequest, IndexedDocument};

#[derive(Clone)]
pub struct RetrievalFixture {
    pub scenario: BenchmarkScenario,
    pub request: HybridQueryRequest,
    pub documents: Vec<IndexedDocument>,
    pub expected_top_title: &'static str,
}

pub fn retrieval_fixtures() -> Vec<RetrievalFixture> {
    vec![
        exact_middleware_fixture(),
        migration_fixture(),
        ambiguous_hooks_fixture(),
        vector_natural_language_fixture(),
        fallback_noise_fixture(),
    ]
}

#[allow(dead_code)]
pub fn retrieval_suite_report() -> BenchmarkSuiteReport {
    BenchmarkSuiteReport {
        scenarios: retrieval_fixtures()
            .iter()
            .map(retrieval_scenario_report)
            .collect(),
    }
}

pub fn retrieval_scenario_report(fixture: &RetrievalFixture) -> BenchmarkScenarioReport {
    let baseline = evaluate_query_for_benchmark(
        &fixture.documents,
        &fixture.request,
        BenchmarkPolicyMode::Baseline,
    )
    .expect("baseline retrieval benchmark should succeed");
    let current = evaluate_query_for_benchmark(
        &fixture.documents,
        &fixture.request,
        BenchmarkPolicyMode::Current,
    )
    .expect("current retrieval benchmark should succeed");
    let quality_guard_passed = current
        .results
        .first()
        .is_some_and(|item| item.title == fixture.expected_top_title);
    scenario_report(
        fixture.scenario.clone(),
        strategy_result_with_retrieval(
            "baseline-retrieval",
            &metrics_from_result(&baseline),
            baseline.section_candidate_count,
            baseline.document_candidate_count,
            baseline.fallback_used,
        ),
        strategy_result_with_retrieval(
            "current-retrieval",
            &metrics_from_result(&current),
            current.section_candidate_count,
            current.document_candidate_count,
            current.fallback_used,
        ),
        None,
        quality_guard_passed,
    )
}

pub fn metrics_from_result(result: &RetrievalBenchmarkResult) -> PackMetrics {
    let evidence = result
        .results
        .iter()
        .map(|item| SourceSnippet {
            source_id: "benchmark".to_string(),
            title: item.title.clone(),
            snippet: item.snippet.clone(),
            citation: item.citation.clone(),
            trust: "official".to_string(),
            retrieved_at: "2026-03-25T00:00:00Z".to_string(),
            version: item.version.clone(),
            stale: false,
            score: item.score,
        })
        .collect::<Vec<_>>();
    PackMetrics {
        resolved_sources_count: 1,
        queried_canisters_count: 1,
        returned_snippets_count: evidence.len(),
        selected_evidence_count: evidence.len(),
        estimated_pack_tokens: estimate_pack_tokens(&evidence),
        empty_source_count: usize::from(evidence.is_empty()),
        source_error_count: 0,
        resolve_ms: 0,
        query_ms_total: 0,
        pack_ms_total: 0,
    }
}

fn exact_middleware_fixture() -> RetrievalFixture {
    RetrievalFixture {
        scenario: BenchmarkScenario {
            name: "exact-middleware".to_string(),
            query: "middleware cookies".to_string(),
            max_sources: 1,
            max_tokens: 1200,
        },
        request: HybridQueryRequest {
            query_text: "middleware cookies".to_string(),
            query_embedding: vec![0.9, 0.1, 0.0, 0.0],
            version: Some("15".to_string()),
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
        documents: vec![document(
            serde_json::json!({
                "title": "Next.js Middleware",
                "snippet": "Use middleware to inspect cookies and redirect unauthenticated users.",
                "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
                "version": "15",
                "content": "Full Next.js middleware docs chunk",
                "section": "middleware",
                "tags": ["next.js", "auth", "cookies", "redirect"]
            }),
            vec![0.9, 0.1, 0.0, 0.0],
        )],
        expected_top_title: "Next.js Middleware",
    }
}

fn migration_fixture() -> RetrievalFixture {
    RetrievalFixture {
        scenario: BenchmarkScenario {
            name: "migration-version".to_string(),
            query: "migration breaking changes".to_string(),
            max_sources: 1,
            max_tokens: 1800,
        },
        request: HybridQueryRequest {
            query_text: "migration breaking changes".to_string(),
            query_embedding: vec![0.8, 0.2, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
        documents: vec![
            document(
                serde_json::json!({
                    "title": "Next.js Middleware",
                    "snippet": "Use middleware to inspect cookies and redirect unauthenticated users.",
                    "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
                    "version": "15",
                    "content": "Full Next.js middleware docs chunk",
                    "section": "middleware",
                    "tags": ["next.js", "auth", "cookies", "redirect"]
                }),
                vec![0.9, 0.1, 0.0, 0.0],
            ),
            document(
                serde_json::json!({
                    "title": "Next.js Upgrade Guide",
                    "snippet": "Check official migration guides and validate breaking changes before upgrading.",
                    "citation": "https://nextjs.org/docs/app/building-your-application/upgrading",
                    "content": "Prefer official migration notes, verify middleware behavior, and review auth integration changes.",
                    "section": "migration",
                    "tags": ["next.js", "migration", "upgrade"]
                }),
                vec![0.8, 0.2, 0.0, 0.0],
            ),
        ],
        expected_top_title: "Next.js Upgrade Guide",
    }
}

fn vector_natural_language_fixture() -> RetrievalFixture {
    RetrievalFixture {
        scenario: BenchmarkScenario {
            name: "vector-natural-language".to_string(),
            query: "how do i keep auth state fresh before rendering protected routes".to_string(),
            max_sources: 1,
            max_tokens: 1600,
        },
        request: HybridQueryRequest {
            query_text: "how do i keep auth state fresh before rendering protected routes".to_string(),
            query_embedding: vec![0.7, 0.3, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
        documents: vec![
            document(
                serde_json::json!({
                    "title": "Supabase Next.js Auth",
                    "snippet": "Refresh auth state on the server before rendering protected routes.",
                    "citation": "https://supabase.com/docs/guides/auth/server-side/nextjs",
                    "version": "2026",
                    "content": "Full Supabase auth docs chunk",
                    "section": "auth",
                    "tags": ["supabase", "auth", "next.js", "server"]
                }),
                vec![0.7, 0.3, 0.0, 0.0],
            ),
            document(
                serde_json::json!({
                    "title": "Next.js Middleware",
                    "snippet": "Use middleware to inspect cookies and redirect unauthenticated users.",
                    "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
                    "version": "15",
                    "content": "Full Next.js middleware docs chunk",
                    "section": "middleware",
                    "tags": ["next.js", "auth", "cookies", "redirect"]
                }),
                vec![0.9, 0.1, 0.0, 0.0],
            ),
        ],
        expected_top_title: "Supabase Next.js Auth",
    }
}

fn ambiguous_hooks_fixture() -> RetrievalFixture {
    RetrievalFixture {
        scenario: BenchmarkScenario {
            name: "ambiguous-hooks".to_string(),
            query: "next react hooks".to_string(),
            max_sources: 1,
            max_tokens: 1600,
        },
        request: HybridQueryRequest {
            query_text: "next react hooks".to_string(),
            query_embedding: vec![0.8, 0.2, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
        documents: vec![
            document(
                serde_json::json!({
                    "title": "Next.js Hook Usage",
                    "snippet": "Use Next.js router hooks to coordinate navigation and client state.",
                    "citation": "https://nextjs.org/docs/app/api-reference/functions/use-router",
                    "version": "15",
                    "content": "Next.js App Router hook patterns and navigation guidance.",
                    "section": "routing",
                    "tags": ["next.js", "hooks", "routing"]
                }),
                vec![0.8, 0.2, 0.0, 0.0],
            ),
            document(
                serde_json::json!({
                    "title": "React Hooks Reference",
                    "snippet": "React hooks like useEffect and useState let components synchronize with external systems.",
                    "citation": "https://react.dev/reference/react",
                    "version": "19",
                    "content": "React hook reference and lifecycle guidance.",
                    "section": "hooks",
                    "tags": ["react", "hooks", "reference"]
                }),
                vec![0.6, 0.4, 0.0, 0.0],
            ),
        ],
        expected_top_title: "Next.js Hook Usage",
    }
}

fn fallback_noise_fixture() -> RetrievalFixture {
    RetrievalFixture {
        scenario: BenchmarkScenario {
            name: "fallback-noise".to_string(),
            query: "next launchagent auth".to_string(),
            max_sources: 1,
            max_tokens: 1400,
        },
        request: HybridQueryRequest {
            query_text: "next launchagent auth".to_string(),
            query_embedding: vec![0.1, 0.9, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
        documents: vec![
            document(
                serde_json::json!({
                    "title": "Tailscale LaunchAgent",
                    "snippet": "Use the macOS LaunchAgent plist to keep Tailscale running after login.",
                    "citation": "https://tailscale.com/kb/launchagent",
                    "version": "1",
                    "content": "LaunchAgent setup details for Tailscale on macOS.",
                    "section": "launchd",
                    "tags": ["tailscale", "macos", "launchagent"]
                }),
                vec![0.1, 0.9, 0.0, 0.0],
            ),
            document(
                serde_json::json!({
                    "title": "Supabase Next.js Auth",
                    "snippet": "Refresh auth state on the server before rendering protected routes.",
                    "citation": "https://supabase.com/docs/guides/auth/server-side/nextjs",
                    "version": "2026",
                    "content": "Full Supabase auth docs chunk",
                    "section": "auth",
                    "tags": ["supabase", "auth", "next.js", "server"]
                }),
                vec![0.7, 0.3, 0.0, 0.0],
            ),
        ],
        expected_top_title: "Tailscale LaunchAgent",
    }
}

fn document(payload: serde_json::Value, embedding: Vec<f32>) -> IndexedDocument {
    IndexedDocument {
        title: payload["title"].as_str().unwrap_or_default().to_string(),
        snippet: payload["snippet"].as_str().unwrap_or_default().to_string(),
        citation: payload["citation"].as_str().unwrap_or_default().to_string(),
        version: payload["version"].as_str().map(ToString::to_string),
        content: payload["content"].as_str().unwrap_or_default().to_string(),
        section: payload["section"].as_str().map(ToString::to_string),
        tags: payload["tags"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(ToString::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        embedding,
    }
}
