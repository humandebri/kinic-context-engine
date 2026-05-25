// Where: src/catalog.rs
// What: CLI-facing source discovery backed by kinic-vfs-cli search.
// Why: Reuse the existing wiki canister API instead of a dedicated catalog canister.
use anyhow::Result;
use kinic_context_core::types::FilterSourcesArgs;
use std::collections::BTreeSet;

use crate::model::{ResolvedSource, SourceMetadata};
use crate::wiki_cli::run_json;
pub(crate) use crate::wiki_metadata::source_prefix;
use crate::wiki_metadata::{
    SOURCES_ROOT, WikiNodeEntry, WikiSearchHit, is_source_index_path, read_node_metadata,
    source_metadata_from_id, source_metadata_from_value,
};

#[allow(async_fn_in_trait)]
pub trait SourceCatalog: Send + Sync {
    async fn get_source(&self, source_id: &str) -> Result<SourceMetadata>;
    async fn resolve_sources(&self, query: &str, limit: usize) -> Result<Vec<ResolvedSource>>;
    async fn list_sources(&self) -> Result<Vec<SourceMetadata>>;
    async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>>;
}

#[derive(Clone)]
pub struct WikiCliSourceCatalog {
    cli_bin: String,
    database_id: String,
}

impl WikiCliSourceCatalog {
    pub fn new(cli_bin: String, database_id: String) -> Self {
        Self {
            cli_bin,
            database_id,
        }
    }
}

impl SourceCatalog for WikiCliSourceCatalog {
    async fn get_source(&self, source_id: &str) -> Result<SourceMetadata> {
        Ok(source_metadata_from_id(source_id))
    }

    async fn resolve_sources(&self, query: &str, limit: usize) -> Result<Vec<ResolvedSource>> {
        let hits = run_search(
            &self.cli_bin,
            &self.database_id,
            SOURCES_ROOT,
            query,
            limit.max(1),
        )?;
        let mut resolved = Vec::<ResolvedSource>::new();
        let mut seen = BTreeSet::<String>::new();
        for hit in hits {
            let Some(metadata) =
                read_node_metadata(&self.cli_bin, &self.database_id, &hit.path, "source")?
            else {
                continue;
            };
            let Some(source) = source_metadata_from_value(metadata, &hit.path) else {
                continue;
            };
            if !seen.insert(source.source_id.clone()) {
                continue;
            }
            let score = hit.score.unwrap_or(0.0);
            resolved.push(ResolvedSource {
                source_id: source.source_id,
                title: source.title,
                score,
                reasons: hit.match_reasons,
            });
            if resolved.len() >= limit.max(1) {
                break;
            }
        }
        Ok(resolved)
    }

    async fn list_sources(&self) -> Result<Vec<SourceMetadata>> {
        load_sources(&self.cli_bin, &self.database_id)
    }

    async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>> {
        let mut sources = load_sources(&self.cli_bin, &self.database_id)?;
        sources.retain(|source| {
            args.domain
                .as_ref()
                .is_none_or(|domain| &source.domain == domain)
                && args
                    .trust
                    .as_ref()
                    .is_none_or(|trust| &source.trust == trust)
                && args.version.as_ref().is_none_or(|version| {
                    source.supported_versions.iter().any(|item| item == version)
                })
        });
        if let Some(limit) = args.limit {
            sources.truncate(limit as usize);
        }
        Ok(sources)
    }
}

pub(crate) fn run_search(
    cli_bin: &str,
    database_id: &str,
    prefix: &str,
    query: &str,
    top_k: usize,
) -> Result<Vec<WikiSearchHit>> {
    let top_k = top_k.to_string();
    run_json(
        cli_bin,
        database_id,
        vec![
            "search-remote".to_string(),
            query.to_string(),
            "--prefix".to_string(),
            prefix.to_string(),
            "--top-k".to_string(),
            top_k,
            "--json".to_string(),
        ],
        "kinic-vfs-cli search failed",
    )
}

fn load_sources(cli_bin: &str, database_id: &str) -> Result<Vec<SourceMetadata>> {
    let entries: Vec<WikiNodeEntry> = run_json(
        cli_bin,
        database_id,
        vec![
            "list-nodes".to_string(),
            "--prefix".to_string(),
            SOURCES_ROOT.to_string(),
            "--recursive".to_string(),
            "--json".to_string(),
        ],
        "kinic-vfs-cli list-nodes failed",
    )?;
    let mut sources = Vec::new();
    for entry in entries {
        if !is_source_index_path(&entry.path) {
            continue;
        }
        if let Some(metadata) = read_node_metadata(cli_bin, database_id, &entry.path, "source")?
            && let Some(source) = source_metadata_from_value(metadata, &entry.path)
        {
            sources.push(source)
        }
    }
    sources.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    Ok(sources)
}
