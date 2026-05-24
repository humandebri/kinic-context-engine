// Where: src/provider.rs
// What: Read-only source querying backed by the hybrid canister query API.
// Why: Keep query embedding generation in the CLI while source canisters own trigram/vector fusion.
use anyhow::{Result, anyhow};
use kinic_context_core::{client::QueryClient, memory, types::{HybridQueryRequest, HybridSearchResult}};

use crate::{
    embedding::EmbeddingClient,
    model::{SourceMetadata, SourceSnippet},
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
pub struct IcSourceQueryProvider {
    client: QueryClient,
    embedding_client: EmbeddingClient,
    fixed_embedding: Option<Vec<f32>>,
}

impl IcSourceQueryProvider {
    pub fn new(client: QueryClient) -> Self {
        Self {
            client,
            embedding_client: EmbeddingClient::from_env(),
            fixed_embedding: None,
        }
    }

    pub fn with_fixed_embedding(client: QueryClient, embedding: Vec<f32>) -> Self {
        Self {
            client,
            embedding_client: EmbeddingClient::from_env(),
            fixed_embedding: Some(embedding),
        }
    }
}

impl SourceQueryProvider for IcSourceQueryProvider {
    async fn query(
        &self,
        source: SourceMetadata,
        query: &str,
        version: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SourceSnippet>> {
        if source.canister_ids.is_empty() {
            return Err(anyhow!(
                "source `{}` is missing canister_ids",
                source.source_id
            ));
        }

        let embedding = match &self.fixed_embedding {
            Some(embedding) => embedding.clone(),
            None => self.embedding_client.embed_query(query).await?,
        };

        let mut snippets = Vec::new();
        let mut errors = Vec::new();
        for canister_id in &source.canister_ids {
            let request = HybridQueryRequest {
                query_text: query.to_string(),
                query_embedding: embedding.clone(),
                version: version.map(ToString::to_string),
                top_k: top_k.max(1) as u32,
                candidate_limit: None,
                keyword_weight: None,
                vector_weight: None,
                filters: None,
            };
            match memory::hybrid_query(&self.client, canister_id, request).await {
                Ok(results) => snippets.extend(
                    results
                        .into_iter()
                        .map(|item| to_source_snippet(&source, item)),
                ),
                Err(error) => errors.push(format!("{canister_id}: {error}")),
            }
        }

        if snippets.is_empty() && !errors.is_empty() {
            return Err(anyhow!(
                "memory search failed for source `{}`: {}",
                source.source_id,
                errors.join("; ")
            ));
        }

        snippets.sort_by(|left, right| right.score.total_cmp(&left.score));
        snippets.truncate(top_k.max(1));
        Ok(snippets)
    }
}

fn to_source_snippet(source: &SourceMetadata, item: HybridSearchResult) -> SourceSnippet {
    SourceSnippet {
        source_id: source.source_id.clone(),
        title: item.title,
        snippet: item.snippet,
        citation: item.citation,
        trust: source.trust.clone(),
        retrieved_at: source.retrieved_at.clone(),
        version: item.version,
        stale: false,
        score: item.score,
    }
}
