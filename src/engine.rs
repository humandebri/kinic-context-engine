// Where: src/engine.rs
// What: Orchestration layer for resolve, query, pack, and cite flows.
// Why: Keep command behavior consistent and centralize the read-only business logic.
use std::{collections::BTreeSet, path::Path};

use anyhow::{Result, anyhow};
use futures::future::join_all;
use kinic_context_core::types::FilterSourcesArgs;

use crate::{
    catalog::SourceCatalog,
    model::{
        CitationEntry, CitationOutput, CommandOutput, EvidencePack, Intent, QueryOutput,
        ResolveOutput, SourceFilters, SourceMetadata, SourceSnippet, SourcesOutput, Warning,
    },
    provider::SourceQueryProvider,
};

const SKILL_DOMAIN: &str = "skill_knowledge";
const SKILL_HINTS: [&str; 6] = [
    "migration",
    "upgrade",
    "debug",
    "checklist",
    "playbook",
    "workflow",
];
const TOPIC_HINTS: [&str; 7] = [
    "auth",
    "middleware",
    "routing",
    "cookies",
    "server",
    "hooks",
    "deploy",
];

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

    pub async fn resolve(
        &self,
        query: &str,
        max_sources: usize,
        include_skills: bool,
    ) -> Result<CommandOutput> {
        let candidate_sources = self
            .catalog()
            .resolve_sources(query, max_sources.saturating_mul(2).max(1))
            .await?;
        let candidate_sources = if include_skills {
            self.rerank_resolved(query, candidate_sources).await?
        } else {
            self.exclude_skill_resolved(candidate_sources).await?
        };
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
        include_skills: bool,
    ) -> Result<CommandOutput> {
        let resolved_sources = self
            .catalog()
            .resolve_sources(query, max_sources.saturating_mul(2).max(1))
            .await?;
        let resolved_sources = if include_skills {
            self.rerank_resolved(query, resolved_sources).await?
        } else {
            self.exclude_skill_resolved(resolved_sources).await?
        };
        let resolved_sources: Vec<_> = resolved_sources.into_iter().take(max_sources).collect();
        let source_ids: Vec<String> = resolved_sources
            .iter()
            .map(|candidate| candidate.source_id.clone())
            .collect();
        let mut warnings = Vec::new();
        let mut evidence = Vec::new();
        let mut seen = BTreeSet::new();
        let mut successful_sources = 0_usize;

        for outcome in self
            .fetch_pack_outcomes(query.to_string(), source_ids.clone())
            .await?
        {
            match outcome {
                PackSourceOutcome::QueryFailed { source_id, stage } => warnings.push(Warning {
                    kind: "source_error".to_string(),
                    message: format!("Failed to {stage} for {source_id}"),
                }),
                PackSourceOutcome::QuerySucceeded { source_id, snippets } => {
                    successful_sources += 1;
                    if snippets.is_empty() {
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

        Ok(CommandOutput::Pack(EvidencePack {
            query: query.to_string(),
            resolved_sources: source_ids,
            evidence: evidence.clone(),
            warnings,
            pack_summary: summarize(&evidence),
            token_budget: max_tokens,
        }))
    }

    pub async fn list_sources(&self, include_skills: bool) -> Result<CommandOutput> {
        let sources = self.catalog().list_sources().await?;
        let sources = filter_skill_sources(sources, include_skills, false);
        let count = sources.len();
        Ok(CommandOutput::ListSources(SourcesOutput {
            sources,
            count,
            filters: None,
        }))
    }

    pub async fn filter_sources(
        &self,
        args: FilterSourcesArgs,
        include_skills: bool,
    ) -> Result<CommandOutput> {
        let allow_skill_domain = args.domain.as_deref() == Some(SKILL_DOMAIN);
        let sources = self.catalog().filter_sources(args.clone()).await?;
        let sources = filter_skill_sources(sources, include_skills, allow_skill_domain);
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

    async fn exclude_skill_resolved(
        &self,
        candidates: Vec<crate::model::ResolvedSource>,
    ) -> Result<Vec<crate::model::ResolvedSource>> {
        let mut filtered = Vec::new();
        for candidate in candidates {
            let source = self.catalog().get_source(&candidate.source_id).await?;
            if !is_skill_source(&source) {
                filtered.push(candidate);
            }
        }
        Ok(filtered)
    }

    async fn rerank_resolved(
        &self,
        query: &str,
        candidates: Vec<crate::model::ResolvedSource>,
    ) -> Result<Vec<crate::model::ResolvedSource>> {
        let query_normalized = normalize(query);
        let task_kind = extract_task_kind(&query_normalized);
        let entities = extract_skill_entities(&query_normalized);
        let topics = extract_skill_topics(&query_normalized);
        let mut reranked = Vec::with_capacity(candidates.len());

        for mut candidate in candidates {
            let source = self.catalog().get_source(&candidate.source_id).await?;
            if is_skill_source(&source) {
                let task_kind_match =
                    task_kind.is_some() && source.skill_kind.as_deref() == task_kind;
                if task_kind_match {
                    candidate.score += 0.7;
                    candidate
                        .reasons
                        .push("skill kind matched query".to_string());
                    if source.targets.iter().any(|target| {
                        let target_normalized = normalize(target);
                        entities.iter().any(|entity| entity == &target_normalized)
                    }) {
                        candidate.score += 0.8;
                        candidate
                            .reasons
                            .push("skill target matched query entity".to_string());
                    }
                    if source.capabilities.iter().any(|capability| {
                        let capability_normalized = normalize(capability);
                        topics.iter().any(|topic| topic == &capability_normalized)
                    }) {
                        candidate.score += 0.45;
                        candidate
                            .reasons
                            .push("skill capability matched query topic".to_string());
                    }
                }
                if source
                    .aliases
                    .iter()
                    .any(|alias| normalize(alias) == query_normalized)
                {
                    candidate.score += 0.45;
                    candidate
                        .reasons
                        .push("skill alias exactly matched query".to_string());
                }
            }
            reranked.push(candidate);
        }

        reranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(reranked)
    }

    async fn fetch_pack_outcomes(
        &self,
        query: String,
        source_ids: Vec<String>,
    ) -> Result<Vec<PackSourceOutcome>> {
        let futures = source_ids.into_iter().map(|source_id| {
            let catalog = self.catalog();
            let provider = self.provider();
            let query = query.clone();
            async move {
                match catalog.get_source(&source_id).await {
                    Ok(source) => match provider.query(source, &query, None, 3).await {
                        Ok(snippets) => PackSourceOutcome::QuerySucceeded { source_id, snippets },
                        Err(_) => PackSourceOutcome::QueryFailed {
                            source_id,
                            stage: "query source canisters",
                        },
                    },
                    Err(_) => PackSourceOutcome::QueryFailed {
                        source_id,
                        stage: "load source metadata",
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
    },
    QuerySucceeded {
        source_id: String,
        snippets: Vec<SourceSnippet>,
    },
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
        let snippet_tokens = approximate_tokens(&snippet);
        if used_tokens.saturating_add(snippet_tokens) > max_tokens {
            continue;
        }
        used_tokens = used_tokens.saturating_add(snippet_tokens);
        selected.push(snippet);
    }
    selected
}

fn approximate_tokens(snippet: &SourceSnippet) -> usize {
    let chars = snippet.title.chars().count()
        + snippet.snippet.chars().count()
        + snippet.citation.chars().count();
    chars.div_ceil(4)
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

fn filter_skill_sources(
    sources: Vec<SourceMetadata>,
    include_skills: bool,
    allow_skill_domain: bool,
) -> Vec<SourceMetadata> {
    if include_skills || allow_skill_domain {
        return sources;
    }

    sources
        .into_iter()
        .filter(|source| !is_skill_source(source))
        .collect()
}

fn is_skill_source(source: &SourceMetadata) -> bool {
    source.domain == SKILL_DOMAIN
}

fn normalize(text: &str) -> String {
    text.to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn extract_task_kind(query_normalized: &str) -> Option<&'static str> {
    SKILL_HINTS
        .iter()
        .copied()
        .find(|hint| query_normalized.contains(hint))
}

fn extract_skill_entities(query_normalized: &str) -> Vec<String> {
    let mut entities = Vec::new();
    if query_normalized.contains("nextjs") || query_normalized.contains("next") {
        entities.push("nextjs".to_string());
    }
    if query_normalized.contains("supabase") {
        entities.push("supabase".to_string());
    }
    if query_normalized.contains("react") {
        entities.push("react".to_string());
    }
    entities
}

fn extract_skill_topics(query_normalized: &str) -> Vec<String> {
    TOPIC_HINTS
        .iter()
        .filter(|topic| query_normalized.contains(**topic))
        .map(|topic| (*topic).to_string())
        .collect()
}
