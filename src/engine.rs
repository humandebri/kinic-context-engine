// Where: src/engine.rs
// What: Orchestration layer for resolve, query, pack, and cite flows.
// Why: Keep command behavior consistent and centralize the read-only business logic.
use std::{collections::BTreeSet, path::Path, time::Instant};

use anyhow::{Result, anyhow};
use futures::future::join_all;
use kinic_context_core::types::FilterSourcesArgs;

use crate::{
    catalog::SourceCatalog,
    model::{
        CitationEntry, CitationOutput, CommandOutput, EvidencePack, Intent, PackMetrics,
        QueryOutput, ResolveOutput, ResolvedSource, SourceFilters, SourceMetadata, SourceSnippet,
        SourcesOutput, Warning,
    },
    pack::{
        build_resolved_source_details, estimate_pack_tokens, per_source_top_k,
        select_sources_for_pack,
    },
    provider::SourceQueryProvider,
};

pub struct ContextEngine<C, P> {
    catalog: Option<C>,
    provider: Option<P>,
}

impl<C, P> ContextEngine<C, P>
where
    C: SourceCatalog,
    P: SourceQueryProvider,
{
    pub fn new(catalog: C, provider: P) -> Self {
        Self {
            catalog: Some(catalog),
            provider: Some(provider),
        }
    }

    pub async fn resolve(&self, query: &str, max_sources: usize) -> Result<CommandOutput> {
        let candidate_sources = self
            .catalog()
            .resolve_sources(query, max_sources.saturating_mul(2).max(1))
            .await?;
        Ok(CommandOutput::Resolve(ResolveOutput {
            query: query.to_string(),
            intent: infer_intent(query),
            entities: extract_entities(query),
            candidate_sources: candidate_sources.into_iter().take(max_sources).collect(),
        }))
    }

    pub async fn query(
        &self,
        source_id: &str,
        query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<CommandOutput> {
        let source = self.catalog().get_source(source_id).await?;
        let snippets = self.provider().query(source, query, version, top_k).await?;
        Ok(CommandOutput::Query(QueryOutput {
            query: query.to_string(),
            source_id: source_id.to_string(),
            snippets,
        }))
    }

    pub async fn pack(
        &self,
        query: &str,
        max_sources: usize,
        max_tokens: usize,
    ) -> Result<CommandOutput> {
        let pack_started_at = Instant::now();
        let resolve_started_at = Instant::now();
        let resolved_candidates = self
            .catalog()
            .resolve_sources(query, max_sources.saturating_mul(2).max(1))
            .await?;
        let resolve_ms = elapsed_ms(resolve_started_at);
        let resolved_sources_count = resolved_candidates.len();
        let selected_sources =
            select_sources_for_pack(&resolved_candidates, max_sources, max_tokens);
        let resolved_source_details =
            build_resolved_source_details(&resolved_candidates, &selected_sources);
        let source_ids: Vec<String> = selected_sources
            .iter()
            .map(|candidate| candidate.source_id.clone())
            .collect();
        let mut warnings = Vec::new();
        let mut evidence = Vec::new();
        let mut seen = BTreeSet::new();
        let mut successful_sources = 0_usize;
        let mut queried_sources_count = 0_usize;
        let mut returned_snippets_count = 0_usize;
        let mut empty_source_count = 0_usize;
        let mut source_error_count = 0_usize;
        let mut query_ms_total = 0_u64;
        let source_top_k = per_source_top_k(selected_sources.len(), max_tokens);

        for outcome in self
            .fetch_pack_outcomes(query.to_string(), selected_sources, source_top_k)
            .await?
        {
            queried_sources_count += outcome.queried_sources_count();
            query_ms_total = query_ms_total.saturating_add(outcome.query_ms());
            match outcome {
                PackSourceOutcome::QueryFailed {
                    source_id, stage, ..
                } => {
                    source_error_count += 1;
                    warnings.push(Warning {
                        kind: "source_error".to_string(),
                        message: format!("Failed to {stage} for {source_id}"),
                    });
                }
                PackSourceOutcome::QuerySucceeded {
                    source_id,
                    snippets,
                    returned_snippets,
                    ..
                } => {
                    successful_sources += 1;
                    returned_snippets_count += returned_snippets;
                    if snippets.is_empty() {
                        empty_source_count += 1;
                        warnings.push(Warning {
                            kind: "empty_source".to_string(),
                            message: format!("No snippets matched for {source_id}"),
                        });
                    }

                    for snippet in snippets {
                        let dedup_key = format!(
                            "{}::{}::{}",
                            snippet.source_id, snippet.title, snippet.citation
                        );
                        if seen.insert(dedup_key) {
                            evidence.push(snippet);
                        }
                    }
                }
            }
        }

        evidence.sort_by(|left, right| right.score.total_cmp(&left.score));
        let evidence = trim_evidence_to_budget(evidence, max_tokens);
        if successful_sources == 0 && evidence.is_empty() {
            return Err(anyhow!(
                "failed to build evidence pack because all resolved sources failed"
            ));
        }

        let metrics = PackMetrics {
            resolved_sources_count,
            queried_sources_count,
            returned_snippets_count,
            selected_evidence_count: evidence.len(),
            estimated_pack_tokens: estimate_pack_tokens(&evidence),
            empty_source_count,
            source_error_count,
            resolve_ms,
            query_ms_total,
            pack_ms_total: elapsed_ms(pack_started_at),
        };

        Ok(CommandOutput::Pack(EvidencePack {
            query: query.to_string(),
            resolved_sources: source_ids,
            resolved_source_details,
            evidence: evidence.clone(),
            warnings,
            pack_summary: summarize(&evidence),
            token_budget: max_tokens,
            metrics: Some(metrics),
        }))
    }

    pub async fn list_sources(&self) -> Result<CommandOutput> {
        let sources = self.catalog().list_sources().await?;
        let count = sources.len();
        Ok(CommandOutput::ListSources(SourcesOutput {
            sources,
            count,
            filters: None,
        }))
    }

    pub async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<CommandOutput> {
        let sources = self.catalog().filter_sources(args.clone()).await?;
        let count = sources.len();
        Ok(CommandOutput::FilterSources(SourcesOutput {
            sources,
            count,
            filters: Some(SourceFilters {
                domain: args.domain,
                trust: args.trust,
                version: args.version,
                limit: args.limit,
            }),
        }))
    }

    fn catalog(&self) -> &C {
        self.catalog
            .as_ref()
            .expect("catalog is required for resolve/query/pack")
    }

    fn provider(&self) -> &P {
        self.provider
            .as_ref()
            .expect("provider is required for query/pack")
    }

    async fn fetch_pack_outcomes(
        &self,
        query: String,
        selected_sources: Vec<ResolvedSource>,
        source_top_k: usize,
    ) -> Result<Vec<PackSourceOutcome>> {
        let futures = selected_sources.into_iter().map(|selected_source| {
            let catalog = self.catalog();
            let provider = self.provider();
            let query = query.clone();
            async move {
                let source_id = selected_source.source_id;
                match catalog.get_source(&source_id).await {
                    Ok(source) => {
                        let started_at = Instant::now();
                        let queried_sources_count = 1;
                        match provider.query(source, &query, None, source_top_k).await {
                            Ok(snippets) => {
                                let returned_snippets = snippets.len();
                                PackSourceOutcome::QuerySucceeded {
                                    source_id,
                                    snippets,
                                    queried_sources_count,
                                    returned_snippets,
                                    query_ms: elapsed_ms(started_at),
                                }
                            }
                            Err(_) => PackSourceOutcome::QueryFailed {
                                source_id,
                                stage: "query wiki source",
                                queried_sources_count,
                                query_ms: elapsed_ms(started_at),
                            },
                        }
                    }
                    Err(_) => PackSourceOutcome::QueryFailed {
                        source_id,
                        stage: "load source metadata",
                        queried_sources_count: 0,
                        query_ms: 0,
                    },
                }
            }
        });

        Ok(join_all(futures).await)
    }
}

impl<C, P> ContextEngine<C, P> {
    pub fn cite(&self, pack: &str) -> Result<CommandOutput> {
        let pack_input = if Path::new(pack).is_file() {
            std::fs::read_to_string(pack)
                .map_err(|error| anyhow!("failed to read evidence pack file `{pack}`: {error}"))?
        } else {
            pack.to_string()
        };

        let parsed: EvidencePack = serde_json::from_str(&pack_input)
            .map_err(|error| anyhow!("failed to parse evidence pack JSON: {error}"))?;
        let citations = parsed
            .evidence
            .iter()
            .map(|item| CitationEntry {
                source_id: item.source_id.clone(),
                title: item.title.clone(),
                citation: item.citation.clone(),
                trust: item.trust.clone(),
                retrieved_at: item.retrieved_at.clone(),
                version: item.version.clone(),
                stale: item.stale,
            })
            .collect();

        Ok(CommandOutput::Cite(CitationOutput {
            query: parsed.query,
            citations,
        }))
    }
}

impl ContextEngine<(), NoopProvider> {
    pub fn citer() -> Self {
        Self {
            catalog: None,
            provider: None,
        }
    }
}

pub struct NoopProvider;

enum PackSourceOutcome {
    QueryFailed {
        source_id: String,
        stage: &'static str,
        queried_sources_count: usize,
        query_ms: u64,
    },
    QuerySucceeded {
        source_id: String,
        snippets: Vec<SourceSnippet>,
        queried_sources_count: usize,
        returned_snippets: usize,
        query_ms: u64,
    },
}

impl PackSourceOutcome {
    fn queried_sources_count(&self) -> usize {
        match self {
            Self::QueryFailed {
                queried_sources_count,
                ..
            }
            | Self::QuerySucceeded {
                queried_sources_count,
                ..
            } => *queried_sources_count,
        }
    }

    fn query_ms(&self) -> u64 {
        match self {
            Self::QueryFailed { query_ms, .. } | Self::QuerySucceeded { query_ms, .. } => *query_ms,
        }
    }
}

impl SourceQueryProvider for NoopProvider {
    async fn query(
        &self,
        _source: SourceMetadata,
        _query: &str,
        _version: Option<&str>,
        _top_k: usize,
    ) -> Result<Vec<SourceSnippet>> {
        Ok(Vec::new())
    }
}

fn summarize(evidence: &[SourceSnippet]) -> String {
    if evidence.is_empty() {
        return "No evidence found for the query.".to_string();
    }

    let titles: Vec<String> = evidence
        .iter()
        .take(3)
        .map(|item| item.title.clone())
        .collect();
    format!("Top evidence came from: {}", titles.join(", "))
}

fn trim_evidence_to_budget(evidence: Vec<SourceSnippet>, max_tokens: usize) -> Vec<SourceSnippet> {
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

fn infer_intent(query: &str) -> Intent {
    let normalized = query.to_ascii_lowercase();
    if ["next", "nextjs", "next.js", "react", "supabase", "hook"]
        .iter()
        .any(|token| normalized.contains(token))
    {
        Intent::Code
    } else if ["travel", "trip", "hotel"]
        .iter()
        .any(|token| normalized.contains(token))
    {
        Intent::Travel
    } else {
        Intent::General
    }
}

fn extract_entities(query: &str) -> Vec<String> {
    let normalized = query.to_ascii_lowercase();
    ["next.js", "supabase", "react", "middleware", "auth"]
        .iter()
        .filter(|candidate| {
            let compact = candidate.replace('.', "");
            normalized.contains(&compact) || normalized.contains(**candidate)
        })
        .map(|candidate| (*candidate).to_string())
        .collect()
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
