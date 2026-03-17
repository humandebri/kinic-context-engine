// Where: tests/engine_tests.rs
// What: Engine-level tests with deterministic mock catalog and providers.
// Why: Verify resolver, query, pack, and cite behavior without requiring live canisters.
use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use kinic_context_cli::{
    catalog::SourceCatalog,
    engine::ContextEngine,
    model::{CommandOutput, ResolvedSource, SourceMetadata, SourceSnippet},
    provider::SourceQueryProvider,
};
use kinic_context_core::types::FilterSourcesArgs;

#[derive(Clone)]
struct MockCatalog {
    sources: BTreeMap<String, SourceMetadata>,
    resolved: Vec<ResolvedSource>,
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

    async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>> {
        let mut sources: Vec<SourceMetadata> = self
            .sources
            .values()
            .filter(|source| {
                args.domain.as_ref().is_none_or(|domain| &source.domain == domain)
                    && args.trust.as_ref().is_none_or(|trust| &source.trust == trust)
                    && args.version.as_ref().is_none_or(|version| {
                        source.supported_versions.iter().any(|item| item == version)
                    })
            })
            .cloned()
            .collect();
        if let Some(limit) = args.limit {
            sources.truncate(limit as usize);
        }
        Ok(sources)
    }
}

#[derive(Clone)]
struct MockProvider {
    responses: BTreeMap<String, Vec<SourceSnippet>>,
    errors: BTreeMap<String, String>,
}

impl SourceQueryProvider for MockProvider {
    async fn query(
        &self,
        source: SourceMetadata,
        _query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SourceSnippet>> {
        if let Some(message) = self.errors.get(&source.source_id) {
            return Err(anyhow!(message.clone()));
        }
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

fn catalog() -> MockCatalog {
    let sources = BTreeMap::from([
        (
            "/vercel/next.js".to_string(),
            SourceMetadata {
                source_id: "/vercel/next.js".to_string(),
                title: "Next.js Docs".to_string(),
                aliases: vec!["next".to_string(), "middleware".to_string()],
                trust: "official".to_string(),
                domain: "code_docs".to_string(),
                skill_kind: None,
                targets: Vec::new(),
                capabilities: Vec::new(),
                canister_ids: vec!["aaaaa-aa".to_string(), "bbbbb-bb".to_string()],
                supported_versions: vec!["14".to_string(), "15".to_string()],
                retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                citations: vec!["https://nextjs.org/docs".to_string()],
            },
        ),
        (
            "/supabase/docs".to_string(),
            SourceMetadata {
                source_id: "/supabase/docs".to_string(),
                title: "Supabase Docs".to_string(),
                aliases: vec!["supabase".to_string(), "auth".to_string()],
                trust: "official".to_string(),
                domain: "code_docs".to_string(),
                skill_kind: None,
                targets: Vec::new(),
                capabilities: Vec::new(),
                canister_ids: vec!["ccccc-cc".to_string()],
                supported_versions: vec!["2026".to_string()],
                retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                citations: vec!["https://supabase.com/docs".to_string()],
            },
        ),
        (
            "/react/docs".to_string(),
            SourceMetadata {
                source_id: "/react/docs".to_string(),
                title: "React Docs".to_string(),
                aliases: vec!["react".to_string(), "hooks".to_string()],
                trust: "official".to_string(),
                domain: "code_docs".to_string(),
                skill_kind: None,
                targets: Vec::new(),
                capabilities: Vec::new(),
                canister_ids: vec!["ddddd-dd".to_string()],
                supported_versions: vec!["19".to_string()],
                retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                citations: vec!["https://react.dev".to_string()],
            },
        ),
        (
            "/skills/nextjs/migration".to_string(),
            SourceMetadata {
                source_id: "/skills/nextjs/migration".to_string(),
                title: "Next.js Migration Skill".to_string(),
                aliases: vec![
                    "next migration".to_string(),
                    "nextjs migration".to_string(),
                    "upgrade".to_string(),
                ],
                trust: "curated".to_string(),
                domain: "skill_knowledge".to_string(),
                skill_kind: Some("migration".to_string()),
                targets: vec!["nextjs".to_string()],
                capabilities: vec![
                    "auth".to_string(),
                    "middleware".to_string(),
                    "routing".to_string(),
                ],
                canister_ids: vec!["skill-aa".to_string()],
                supported_versions: Vec::new(),
                retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                citations: vec![
                    "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md"
                        .to_string(),
                ],
            },
        ),
    ]);
    let resolved = vec![
        ResolvedSource {
            source_id: "/vercel/next.js".to_string(),
            title: "Next.js Docs".to_string(),
            score: 1.2,
            reasons: vec!["matched alias `next`".to_string()],
        },
        ResolvedSource {
            source_id: "/supabase/docs".to_string(),
            title: "Supabase Docs".to_string(),
            score: 0.9,
            reasons: vec!["matched alias `supabase`".to_string()],
        },
        ResolvedSource {
            source_id: "/react/docs".to_string(),
            title: "React Docs".to_string(),
            score: 0.2,
            reasons: vec!["matched code intent".to_string()],
        },
        ResolvedSource {
            source_id: "/skills/nextjs/migration".to_string(),
            title: "Next.js Migration Skill".to_string(),
            score: 0.8,
            reasons: vec!["matched alias `next migration`".to_string()],
        },
    ];
    MockCatalog { sources, resolved }
}

fn provider() -> MockProvider {
    MockProvider {
        responses: BTreeMap::from([
            (
                "/vercel/next.js".to_string(),
                vec![SourceSnippet {
                    source_id: "/vercel/next.js".to_string(),
                    title: "Next.js Middleware".to_string(),
                    snippet: "Use middleware to read cookies and redirect.".to_string(),
                    citation:
                        "https://nextjs.org/docs/app/building-your-application/routing/middleware"
                            .to_string(),
                    trust: "official".to_string(),
                    retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                    version: Some("15".to_string()),
                    stale: false,
                    score: 2.0,
                }],
            ),
            (
                "/supabase/docs".to_string(),
                vec![SourceSnippet {
                    source_id: "/supabase/docs".to_string(),
                    title: "Supabase Next.js Auth".to_string(),
                    snippet: "Refresh auth state on the server before rendering protected pages."
                        .to_string(),
                    citation: "https://supabase.com/docs/guides/auth/auth-helpers/nextjs"
                        .to_string(),
                    trust: "official".to_string(),
                    retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                    version: Some("2026".to_string()),
                    stale: false,
                    score: 1.5,
                }],
            ),
            (
                "/skills/nextjs/migration".to_string(),
                vec![SourceSnippet {
                    source_id: "/skills/nextjs/migration".to_string(),
                    title: "Next.js Migration Skill".to_string(),
                    snippet: "Prefer official migration guides and verify breaking changes before upgrades."
                        .to_string(),
                    citation: "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md"
                        .to_string(),
                    trust: "curated".to_string(),
                    retrieved_at: "2026-03-17T00:00:00Z".to_string(),
                    version: None,
                    stale: false,
                    score: 1.1,
                }],
            ),
            ("/react/docs".to_string(), Vec::new()),
        ]),
        errors: BTreeMap::new(),
    }
}

#[tokio::test]
async fn resolve_prefers_nextjs_for_middleware_query() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Resolve(output) = engine
        .resolve("next.js middleware auth cookies", 5, false)
        .await
        .expect("resolve should succeed")
    else {
        panic!("expected resolve output");
    };

    assert_eq!(output.candidate_sources[0].source_id, "/vercel/next.js");
    assert!(
        output
            .candidate_sources
            .iter()
            .all(|item| item.source_id != "/skills/nextjs/migration")
    );
}

#[tokio::test]
async fn query_respects_version_filter() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Query(output) = engine
        .query(
            "/vercel/next.js",
            "middleware cookies redirect",
            Some("15"),
            5,
        )
        .await
        .expect("query should succeed")
    else {
        panic!("expected query output");
    };

    assert_eq!(output.snippets.len(), 1);
    assert_eq!(output.snippets[0].version.as_deref(), Some("15"));
}

#[tokio::test]
async fn pack_merges_multiple_sources_and_warns_on_empty_source() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Pack(output) = engine
        .pack("protect route in next.js with supabase auth", 3, 3000, false)
        .await
        .expect("pack should succeed")
    else {
        panic!("expected pack output");
    };

    assert!(
        output
            .resolved_sources
            .contains(&"/vercel/next.js".to_string())
    );
    assert!(
        output
            .resolved_sources
            .contains(&"/supabase/docs".to_string())
    );
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.kind == "empty_source")
    );
    assert!(
        output
            .resolved_sources
            .iter()
            .all(|item| item != "/skills/nextjs/migration")
    );
}

#[tokio::test]
async fn pack_skips_failed_sources_and_records_source_error_warning() {
    let engine = ContextEngine::new(
        catalog(),
        MockProvider {
            responses: provider().responses,
            errors: BTreeMap::from([(
                "/supabase/docs".to_string(),
                "memory search failed".to_string(),
            )]),
        },
    );
    let CommandOutput::Pack(output) = engine
        .pack("protect route in next.js with supabase auth", 3, 3000, false)
        .await
        .expect("pack should succeed")
    else {
        panic!("expected pack output");
    };

    assert!(
        output
            .evidence
            .iter()
            .any(|item| item.source_id == "/vercel/next.js")
    );
    assert!(
        output
            .warnings
            .iter()
            .any(|warning| warning.kind == "source_error"
                && warning.message.contains("/supabase/docs"))
    );
}

#[tokio::test]
async fn pack_fails_when_all_resolved_sources_fail() {
    let engine = ContextEngine::new(
        catalog(),
        MockProvider {
            responses: provider().responses,
            errors: BTreeMap::from([
                (
                    "/vercel/next.js".to_string(),
                    "memory search failed".to_string(),
                ),
                (
                    "/supabase/docs".to_string(),
                    "memory search failed".to_string(),
                ),
                ("/react/docs".to_string(), "memory search failed".to_string()),
                (
                    "/skills/nextjs/migration".to_string(),
                    "memory search failed".to_string(),
                ),
            ]),
        },
    );

    let error = engine
        .pack("protect route in next.js with supabase auth", 3, 3000, false)
        .await
        .expect_err("pack should fail when every source fails");
    assert!(error
        .to_string()
        .contains("all resolved sources failed"));
}

#[tokio::test]
async fn pack_respects_token_budget() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Pack(output) = engine
        .pack("protect route in next.js with supabase auth", 3, 10, false)
        .await
        .expect("pack should succeed")
    else {
        panic!("expected pack output");
    };

    assert!(output.evidence.is_empty());
    assert_eq!(output.pack_summary, "No evidence found for the query.");
    assert_eq!(output.token_budget, 10);
}

#[tokio::test]
async fn cite_reads_inline_pack_json() {
    let engine = ContextEngine::citer();
    let pack = r#"{"query":"q","resolved_sources":["/vercel/next.js"],"evidence":[{"source_id":"/vercel/next.js","title":"Next.js Middleware","snippet":"s","citation":"https://nextjs.org/docs/app/building-your-application/routing/middleware","trust":"official","retrieved_at":"2026-03-17T00:00:00Z","version":"15","stale":false,"score":1.0}],"warnings":[],"pack_summary":"Top evidence came from: Next.js Middleware","token_budget":3000}"#;

    let CommandOutput::Cite(output) = engine.cite(pack).expect("cite should succeed") else {
        panic!("expected cite output");
    };

    assert_eq!(output.citations.len(), 1);
}

#[tokio::test]
async fn list_sources_returns_catalog_entries() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::ListSources(output) = engine
        .list_sources(false)
        .await
        .expect("list_sources should succeed")
    else {
        panic!("expected list sources output");
    };

    assert_eq!(output.count, 3);
    assert!(output.filters.is_none());
}

#[tokio::test]
async fn filter_sources_respects_domain_and_version() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::FilterSources(output) = engine
        .filter_sources(FilterSourcesArgs {
            domain: Some("code_docs".to_string()),
            trust: Some("official".to_string()),
            version: Some("15".to_string()),
            limit: Some(5),
        }, false)
        .await
        .expect("filter_sources should succeed")
    else {
        panic!("expected filter sources output");
    };

    assert_eq!(output.count, 1);
    assert_eq!(output.sources[0].source_id, "/vercel/next.js");
    assert_eq!(output.filters.expect("filters should exist").version.as_deref(), Some("15"));
}

#[tokio::test]
async fn resolve_can_include_skill_sources_when_opted_in() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Resolve(output) = engine
        .resolve("next migration", 5, true)
        .await
        .expect("resolve should succeed")
    else {
        panic!("expected resolve output");
    };

    assert_eq!(output.candidate_sources[0].source_id, "/skills/nextjs/migration");
}

#[tokio::test]
async fn query_can_read_skill_source_without_opt_in_flag() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Query(output) = engine
        .query("/skills/nextjs/migration", "upgrade plan", None, 5)
        .await
        .expect("query should succeed")
    else {
        panic!("expected query output");
    };

    assert_eq!(output.snippets[0].source_id, "/skills/nextjs/migration");
    assert!(output.snippets[0].citation.contains("github.com/ICME-Lab"));
}

#[tokio::test]
async fn pack_can_include_skill_evidence_when_opted_in() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Pack(output) = engine
        .pack("next migration", 5, 3000, true)
        .await
        .expect("pack should succeed")
    else {
        panic!("expected pack output");
    };

    assert!(
        output
            .evidence
            .iter()
            .any(|item| item.source_id == "/skills/nextjs/migration")
    );
}

#[tokio::test]
async fn list_sources_hides_skills_by_default_but_includes_them_with_opt_in() {
    let engine = ContextEngine::new(catalog(), provider());

    let CommandOutput::ListSources(default_output) = engine
        .list_sources(false)
        .await
        .expect("list_sources should succeed")
    else {
        panic!("expected list sources output");
    };
    assert!(
        default_output
            .sources
            .iter()
            .all(|item| item.domain != "skill_knowledge")
    );

    let CommandOutput::ListSources(opted_in_output) = engine
        .list_sources(true)
        .await
        .expect("list_sources should succeed")
    else {
        panic!("expected list sources output");
    };
    assert!(
        opted_in_output
            .sources
            .iter()
            .any(|item| item.source_id == "/skills/nextjs/migration")
    );
}

#[tokio::test]
async fn filter_sources_can_return_skill_domain_without_opt_in() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::FilterSources(output) = engine
        .filter_sources(
            FilterSourcesArgs {
                domain: Some("skill_knowledge".to_string()),
                trust: None,
                version: None,
                limit: Some(5),
            },
            false,
        )
        .await
        .expect("filter_sources should succeed")
    else {
        panic!("expected filter sources output");
    };

    assert_eq!(output.count, 1);
    assert_eq!(output.sources[0].source_id, "/skills/nextjs/migration");
}

#[tokio::test]
async fn skill_boost_does_not_leak_into_regular_queries() {
    let engine = ContextEngine::new(catalog(), provider());
    let CommandOutput::Resolve(output) = engine
        .resolve("next middleware", 5, true)
        .await
        .expect("resolve should succeed")
    else {
        panic!("expected resolve output");
    };

    assert_eq!(output.candidate_sources[0].source_id, "/vercel/next.js");
}
