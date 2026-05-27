// Where: src/provider.rs
// What: Source querying backed by kinic-vfs-cli search.
// Why: Keep retrieval on the existing wiki API instead of a separate source backend.
use anyhow::Result;

use crate::{
    catalog::{run_search, source_prefix},
    model::{SourceMetadata, SourceSnippet},
    wiki_metadata::{is_docs_chunk_path, read_node_metadata},
};

#[allow(async_fn_in_trait)]
pub trait SourceQueryProvider: Send + Sync {
    async fn query(
        &self,
        source: SourceMetadata,
        query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SourceSnippet>>;
}

#[derive(Clone)]
pub struct WikiCliSourceQueryProvider {
    cli_bin: String,
    database_id: String,
}

impl WikiCliSourceQueryProvider {
    pub fn new(cli_bin: String, database_id: String) -> Self {
        Self {
            cli_bin,
            database_id,
        }
    }
}

impl SourceQueryProvider for WikiCliSourceQueryProvider {
    async fn query(
        &self,
        source: SourceMetadata,
        query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SourceSnippet>> {
        let prefix = match version {
            Some(version) if !version.is_empty() => {
                format!("{}/{}", source_prefix(&source.source_id), version)
            }
            _ => source_prefix(&source.source_id),
        };
        let hits = run_search(
            &self.cli_bin,
            &self.database_id,
            &prefix,
            query,
            top_k.max(1),
        )?;
        let mut snippets = Vec::new();
        for hit in hits {
            if !is_docs_chunk_path(&hit.path) {
                continue;
            }
            let Some(metadata) =
                read_node_metadata(&self.cli_bin, &self.database_id, &hit.path, "chunk")?
            else {
                continue;
            };
            if metadata.source_id != source.source_id || metadata.chunk_id.is_none() {
                eprintln!("skipped chunk metadata: {}", hit.path);
                continue;
            }
            snippets.push(SourceSnippet {
                source_id: source.source_id.clone(),
                title: if metadata.title.is_empty() {
                    source.title.clone()
                } else {
                    metadata.title
                },
                snippet: hit.snippet.unwrap_or_else(|| hit.path.clone()),
                citation: if metadata.citation.is_empty() {
                    hit.path
                } else {
                    metadata.citation
                },
                trust: if metadata.trust.is_empty() {
                    source.trust.clone()
                } else {
                    metadata.trust
                },
                retrieved_at: if metadata.retrieved_at.is_empty() {
                    source.retrieved_at.clone()
                } else {
                    metadata.retrieved_at
                },
                version: metadata
                    .version
                    .or_else(|| version.map(ToString::to_string)),
                stale: false,
                score: hit.score.unwrap_or(0.0),
            });
        }
        Ok(snippets)
    }
}
