// Where: tools/pocket_ic_tests/tests/catalog_e2e.rs
// What: PocketIC deploy and engine-level end-to-end tests for the catalog and fake memory canisters.
// Why: Verify the read path end-to-end without relying on external embedding APIs or live ICP canisters.
mod common;

use anyhow::Result;
use fake_memory_instance::{
    BenchmarkPolicyMode, RetrievalBenchmarkResult, evaluate_query_for_benchmark,
};
use kinic_context_cli::{
    benchmark::{
        BenchmarkScenario, BenchmarkSuiteReport, markdown_summary, scenario_report,
        strategy_result, strategy_result_with_retrieval,
    },
    catalog::IcSourceCatalog,
    engine::ContextEngine,
    model::{PackMetrics, QueryOutput},
    pack::estimate_pack_tokens,
    provider::IcSourceQueryProvider,
};
use kinic_context_core::types::{HybridQueryFilters, HybridQueryRequest};
use kinic_context_core::{client::QueryClient, types::FilterSourcesArgs};

use common::{
    fixtures::{
        launch_agent_results, missing_canister_id, nextjs_migration_results, nextjs_results,
        source, supabase_results,
    },
    pocketic::{
        TestCanisters, ensure_pocket_ic_server, filter_sources, get_source, hybrid_query_memory,
        install_catalog_canister, install_fake_memory_instance, pocket_ic, replace_catalog,
        resolve_sources, search_memory, upgrade_catalog_canister, upgrade_fake_memory_instance,
    },
};

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn catalog_canister_deploys_and_resolves_fixture_sources() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let next_migration_memory =
        install_fake_memory_instance(&mut pic, test_canisters, nextjs_migration_results())?;
    let supabase_memory =
        install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source(
                "/vercel/next.js",
                vec![next_memory.to_text(), next_migration_memory.to_text()],
            ),
            source("/supabase/docs", vec![supabase_memory.to_text()]),
            source("/react/docs", vec![react_memory.to_text()]),
        ],
    )?;

    let nextjs = get_source(&pic, catalog_id, "/vercel/next.js")?
        .expect("next.js source should exist after replace_catalog");
    assert_eq!(nextjs.canister_ids[0], next_memory.to_text());

    let resolved = resolve_sources(&pic, catalog_id, "next middleware", 3)?;
    assert_eq!(resolved[0].source_id, "/vercel/next.js");

    let filtered = filter_sources(
        &pic,
        catalog_id,
        FilterSourcesArgs {
            domain: Some("code_docs".to_string()),
            trust: Some("official".to_string()),
            version: Some("15".to_string()),
            limit: Some(3),
        },
    )?;
    assert_eq!(filtered[0].source_id, "/vercel/next.js");

    Ok(())
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn catalog_upgrade_preserves_replaced_sources() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![source("/vercel/next.js", vec![next_memory.to_text()])],
    )?;

    upgrade_catalog_canister(&pic, test_canisters, catalog_id)?;

    let nextjs = get_source(&pic, catalog_id, "/vercel/next.js")?
        .expect("next.js source should remain after upgrade");
    assert_eq!(nextjs.canister_ids, vec![next_memory.to_text()]);

    let resolved = resolve_sources(&pic, catalog_id, "next middleware", 3)?;
    assert_eq!(resolved[0].source_id, "/vercel/next.js");

    Ok(())
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn fake_memory_upgrade_preserves_documents_sections_and_search() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let memory_id = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;

    upgrade_fake_memory_instance(&pic, test_canisters, memory_id)?;

    let vector_results = search_memory(&pic, memory_id, vec![0.9_f32, 0.1, 0.0, 0.0])?;
    assert!(vector_results[0].1.contains("Next.js Middleware"));

    let hybrid_results = hybrid_query_memory(
        &pic,
        memory_id,
        HybridQueryRequest {
            query_text: "middleware cookies".to_string(),
            query_embedding: vec![0.9_f32, 0.1, 0.0, 0.0],
            version: Some("15".to_string()),
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        },
    )?;
    assert_eq!(hybrid_results[0].title, "Next.js Middleware");
    assert_eq!(hybrid_results[0].section.as_deref(), Some("middleware"));

    Ok(())
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn engine_query_and_pack_work_against_pocket_ic() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let next_migration_memory =
        install_fake_memory_instance(&mut pic, test_canisters, nextjs_migration_results())?;
    let supabase_memory =
        install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source(
                "/vercel/next.js",
                vec![next_memory.to_text(), next_migration_memory.to_text()],
            ),
            source("/supabase/docs", vec![supabase_memory.to_text()]),
            source("/react/docs", vec![react_memory.to_text()]),
        ],
    )?;

    tokio::runtime::Runtime::new()?.block_on(async {
        let client = QueryClient::new(gateway.as_ref(), true).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider =
            IcSourceQueryProvider::with_fixed_embedding(client, vec![0.9_f32, 0.1, 0.0, 0.0]);
        let engine = ContextEngine::new(catalog, provider);

        let query = engine
            .query("/vercel/next.js", "middleware cookies", Some("15"), 5)
            .await?;
        let query_json = serde_json::to_value(query)?;
        assert_eq!(query_json["snippets"][0]["title"], "Next.js Middleware");
        assert_eq!(
            query_json["snippets"][0]["citation"],
            "https://nextjs.org/docs/app/building-your-application/routing/middleware"
        );

        let migration_query = engine
            .query("/vercel/next.js", "migration breaking changes", None, 5)
            .await?;
        let migration_query_json = serde_json::to_value(migration_query)?;
        assert_eq!(
            migration_query_json["snippets"][0]["title"],
            "Next.js Upgrade Guide"
        );

        let pack = engine
            .pack("protect route in next.js with supabase auth", 3, 3000)
            .await?;
        let pack_json = serde_json::to_value(pack)?;
        assert!(
            pack_json["resolved_sources"]
                .as_array()
                .expect("resolved_sources should be an array")
                .len()
                >= 2
        );
        assert!(
            pack_json["evidence"]
                .as_array()
                .expect("evidence should be an array")
                .len()
                >= 2
        );
        assert_eq!(pack_json["metrics"]["queried_canisters_count"], 3);
        assert!(
            pack_json["metrics"]["selected_evidence_count"]
                .as_u64()
                .expect("selected_evidence_count should be numeric")
                >= 2
        );

        let selective_pack = engine.pack("next react hooks", 3, 3000).await?;
        let selective_json = serde_json::to_value(selective_pack)?;
        assert!(
            selective_json["warnings"]
                .as_array()
                .expect("warnings should be an array")
                .iter()
                .any(|warning| warning["kind"] == "empty_source")
        );

        let migration_pack = engine.pack("next migration", 5, 3000).await?;
        let skill_pack_json = serde_json::to_value(migration_pack)?;
        assert!(
            skill_pack_json["evidence"]
                .as_array()
                .expect("evidence should be an array")
                .iter()
                .any(|item| item["source_id"] == "/vercel/next.js")
        );
        Ok(())
    })
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn engine_query_and_pack_error_contracts_stay_stable() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;
    let next_migration_memory =
        install_fake_memory_instance(&mut pic, test_canisters, nextjs_migration_results())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source(
                "/vercel/next.js",
                vec![
                    next_memory.to_text(),
                    next_migration_memory.to_text(),
                    missing_canister_id(),
                ],
            ),
            source("/supabase/docs", Vec::new()),
            source("/react/docs", vec![react_memory.to_text()]),
        ],
    )?;

    tokio::runtime::Runtime::new()?.block_on(async {
        let client = QueryClient::new(gateway.as_ref(), true).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider =
            IcSourceQueryProvider::with_fixed_embedding(client, vec![0.8_f32, 0.2, 0.0, 0.0]);
        let engine = ContextEngine::new(catalog, provider);

        let missing_source = engine.query("/unknown/source", "middleware", None, 5).await;
        assert!(missing_source.is_err());

        let empty_canisters = engine.query("/supabase/docs", "auth", None, 5).await;
        assert!(empty_canisters.is_err());

        let partial = engine
            .query("/vercel/next.js", "middleware cookies", None, 5)
            .await;
        match partial {
            Ok(partial) => {
                let partial_json = serde_json::to_value(partial)?;
                assert_eq!(partial_json["snippets"][0]["title"], "Next.js Middleware");
            }
            Err(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("memory search failed for source")
                );
            }
        }

        let version_filtered = engine
            .query("/vercel/next.js", "middleware cookies", Some("999"), 5)
            .await;
        match version_filtered {
            Ok(version_filtered) => {
                let version_json = serde_json::to_value(version_filtered)?;
                assert_eq!(
                    version_json["snippets"]
                        .as_array()
                        .expect("snippets should be an array")
                        .len(),
                    0
                );
            }
            Err(error) => {
                assert!(
                    error
                        .to_string()
                        .contains("memory search failed for source")
                );
            }
        }

        let pack = engine.pack("react hooks", 3, 3000).await?;
        let pack_json = serde_json::to_value(pack)?;
        assert!(
            pack_json["warnings"]
                .as_array()
                .expect("warnings should be an array")
                .iter()
                .any(|warning| {
                    warning["kind"] == "source_error" || warning["kind"] == "empty_source"
                })
        );
        assert!(
            pack_json["metrics"]["source_error_count"]
                .as_u64()
                .is_some(),
            "source_error_count should be numeric"
        );
        Ok(())
    })
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn hybrid_query_returns_trigram_match_for_launch_agent() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let launch_agent_memory =
        install_fake_memory_instance(&mut pic, test_canisters, launch_agent_results())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![source("/react/docs", vec![launch_agent_memory.to_text()])],
    )?;

    tokio::runtime::Runtime::new()?.block_on(async {
        let client = QueryClient::new(gateway.as_ref(), true).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider =
            IcSourceQueryProvider::with_fixed_embedding(client, vec![0.1_f32, 0.9, 0.0, 0.0]);
        let engine = ContextEngine::new(catalog, provider);

        let query = engine
            .query("/react/docs", "LaunchAgent", Some("1"), 5)
            .await?;
        let query_json = serde_json::to_value(query)?;
        assert_eq!(query_json["snippets"][0]["title"], "Tailscale LaunchAgent");
        Ok(())
    })
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn pocket_ic_benchmark_report_serializes() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let supabase_memory =
        install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source("/vercel/next.js", vec![next_memory.to_text()]),
            source("/supabase/docs", vec![supabase_memory.to_text()]),
            source("/react/docs", vec![react_memory.to_text()]),
        ],
    )?;

    tokio::runtime::Runtime::new()?.block_on(async {
        let client = QueryClient::new(gateway.as_ref(), true).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider =
            IcSourceQueryProvider::with_fixed_embedding(client, vec![0.9_f32, 0.1, 0.0, 0.0]);
        let engine = ContextEngine::new(catalog, provider);

        let pack = engine
            .pack("protect route in next.js with supabase auth", 3, 3000)
            .await?;
        let pack_json = serde_json::to_value(&pack)?;
        let metrics = match pack {
            kinic_context_cli::model::CommandOutput::Pack(output) => {
                output.metrics.expect("pack metrics should exist")
            }
            _ => panic!("expected pack output"),
        };
        let exact_query = engine
            .query("/vercel/next.js", "middleware cookies", Some("15"), 5)
            .await?;
        let migration_query = engine
            .query("/vercel/next.js", "migration breaking changes", None, 5)
            .await?;
        let kinic_context_cli::model::CommandOutput::Query(exact_query) = exact_query else {
            panic!("expected query output");
        };
        let kinic_context_cli::model::CommandOutput::Query(migration_query) = migration_query
        else {
            panic!("expected query output");
        };
        let exact_metrics = query_metrics(&exact_query, 1);
        let migration_metrics = query_metrics(&migration_query, 1);
        let exact_request = HybridQueryRequest {
            query_text: "middleware cookies".to_string(),
            query_embedding: vec![0.9_f32, 0.1, 0.0, 0.0],
            version: Some("15".to_string()),
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        };
        let migration_request = HybridQueryRequest {
            query_text: "migration breaking changes".to_string(),
            query_embedding: vec![0.9_f32, 0.1, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        };
        let two_source_request = HybridQueryRequest {
            query_text: "protect route in next.js with supabase auth".to_string(),
            query_embedding: vec![0.9_f32, 0.1, 0.0, 0.0],
            version: None,
            top_k: 5,
            candidate_limit: None,
            keyword_weight: None,
            vector_weight: None,
            filters: Some(HybridQueryFilters::default()),
        };
        let exact_documents = nextjs_results();
        let migration_documents = nextjs_migration_results();
        let mut two_source_documents = nextjs_results();
        two_source_documents.extend(supabase_results());
        let exact_baseline_result = evaluate_retrieval(
            &exact_documents,
            &exact_request,
            BenchmarkPolicyMode::Baseline,
        )?;
        let exact_current_result = evaluate_retrieval(
            &exact_documents,
            &exact_request,
            BenchmarkPolicyMode::Current,
        )?;
        let migration_baseline_result = evaluate_retrieval(
            &migration_documents,
            &migration_request,
            BenchmarkPolicyMode::Baseline,
        )?;
        let migration_current_result = evaluate_retrieval(
            &migration_documents,
            &migration_request,
            BenchmarkPolicyMode::Current,
        )?;
        let two_source_baseline_result = evaluate_retrieval(
            &two_source_documents,
            &two_source_request,
            BenchmarkPolicyMode::Baseline,
        )?;
        let two_source_current_result = evaluate_retrieval(
            &two_source_documents,
            &two_source_request,
            BenchmarkPolicyMode::Current,
        )?;
        let exact_baseline = retrieval_metrics(&exact_baseline_result);
        let exact_current = retrieval_metrics(&exact_current_result);
        let migration_baseline = retrieval_metrics(&migration_baseline_result);
        let migration_current = retrieval_metrics(&migration_current_result);
        let two_source_baseline = retrieval_metrics(&two_source_baseline_result);
        let two_source_current = retrieval_metrics(&two_source_current_result);

        let report = BenchmarkSuiteReport {
            scenarios: vec![
                scenario_report(
                    BenchmarkScenario {
                        name: "two-source-auth".to_string(),
                        query: "protect route in next.js with supabase auth".to_string(),
                        max_sources: 3,
                        max_tokens: 3000,
                    },
                    strategy_result_with_retrieval(
                        "baseline-retrieval",
                        &two_source_baseline,
                        two_source_baseline_result.section_candidate_count,
                        two_source_baseline_result.document_candidate_count,
                        two_source_baseline_result.fallback_used,
                    ),
                    strategy_result_with_retrieval(
                        "current-retrieval",
                        &two_source_current,
                        two_source_current_result.section_candidate_count,
                        two_source_current_result.document_candidate_count,
                        two_source_current_result.fallback_used,
                    ),
                    Some(strategy_result("pocket-ic", &metrics)),
                    pack_json["evidence"]
                        .as_array()
                        .is_some_and(|items| items.len() >= 2),
                ),
                scenario_report(
                    BenchmarkScenario {
                        name: "exact-middleware".to_string(),
                        query: "middleware cookies".to_string(),
                        max_sources: 1,
                        max_tokens: 1200,
                    },
                    strategy_result_with_retrieval(
                        "baseline-retrieval",
                        &exact_baseline,
                        exact_baseline_result.section_candidate_count,
                        exact_baseline_result.document_candidate_count,
                        exact_baseline_result.fallback_used,
                    ),
                    strategy_result_with_retrieval(
                        "current-retrieval",
                        &exact_current,
                        exact_current_result.section_candidate_count,
                        exact_current_result.document_candidate_count,
                        exact_current_result.fallback_used,
                    ),
                    Some(strategy_result("pocket-ic", &exact_metrics)),
                    exact_query
                        .snippets
                        .first()
                        .is_some_and(|item| item.title == "Next.js Middleware"),
                ),
                scenario_report(
                    BenchmarkScenario {
                        name: "migration-version".to_string(),
                        query: "migration breaking changes".to_string(),
                        max_sources: 1,
                        max_tokens: 1800,
                    },
                    strategy_result_with_retrieval(
                        "baseline-retrieval",
                        &migration_baseline,
                        migration_baseline_result.section_candidate_count,
                        migration_baseline_result.document_candidate_count,
                        migration_baseline_result.fallback_used,
                    ),
                    strategy_result_with_retrieval(
                        "current-retrieval",
                        &migration_current,
                        migration_current_result.section_candidate_count,
                        migration_current_result.document_candidate_count,
                        migration_current_result.fallback_used,
                    ),
                    Some(strategy_result("pocket-ic", &migration_metrics)),
                    migration_query
                        .snippets
                        .first()
                        .is_some_and(|item| item.title == "Next.js Upgrade Guide"),
                ),
            ],
        };

        let report_json =
            serde_json::to_string_pretty(&report).expect("benchmark report should encode");
        let report_markdown = markdown_summary(&report);
        assert!(report_json.contains("\"pocket_ic_skipped\": false"));
        assert!(report_markdown.contains("two-source-auth"));
        assert!(report_markdown.contains("exact-middleware"));
        assert!(report_markdown.contains("migration-version"));
        assert!(report_markdown.contains("| benchmark case |"));
        assert!(report_markdown.contains("verdict"));
        assert_eq!(pack_json["metrics"]["queried_canisters_count"], 2);
        Ok(())
    })
}

fn query_metrics(output: &QueryOutput, queried_canisters_count: usize) -> PackMetrics {
    PackMetrics {
        resolved_sources_count: 1,
        queried_canisters_count,
        returned_snippets_count: output.snippets.len(),
        selected_evidence_count: output.snippets.len(),
        estimated_pack_tokens: estimate_pack_tokens(&output.snippets),
        empty_source_count: usize::from(output.snippets.is_empty()),
        source_error_count: 0,
        resolve_ms: 0,
        query_ms_total: 0,
        pack_ms_total: 0,
    }
}

fn evaluate_retrieval(
    documents: &[kinic_context_core::types::IndexedDocument],
    request: &HybridQueryRequest,
    mode: BenchmarkPolicyMode,
) -> Result<RetrievalBenchmarkResult> {
    evaluate_query_for_benchmark(documents, request, mode).map_err(anyhow::Error::msg)
}

fn retrieval_metrics(output: &RetrievalBenchmarkResult) -> PackMetrics {
    let snippets = output
        .results
        .iter()
        .map(|item| kinic_context_cli::model::SourceSnippet {
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
        returned_snippets_count: snippets.len(),
        selected_evidence_count: snippets.len(),
        estimated_pack_tokens: estimate_pack_tokens(&snippets),
        empty_source_count: usize::from(snippets.is_empty()),
        source_error_count: 0,
        resolve_ms: 0,
        query_ms_total: 0,
        pack_ms_total: 0,
    }
}
