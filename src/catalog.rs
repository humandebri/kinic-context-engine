// Where: src/catalog.rs
// What: CLI-facing source catalog abstraction backed by the catalog canister.
// Why: Keep catalog resolution separate from memory instance retrieval and make tests cheap to mock.
use anyhow::{Result, anyhow};
use kinic_context_core::{catalog, client::QueryClient, types::FilterSourcesArgs};

use crate::model::{ResolvedSource, SourceMetadata};

#[allow(async_fn_in_trait)]
pub trait SourceCatalog: Send + Sync {
    async fn get_source(&self, source_id: &str) -> Result<SourceMetadata>;
    async fn resolve_sources(&self, query: &str, limit: usize) -> Result<Vec<ResolvedSource>>;
    async fn list_sources(&self) -> Result<Vec<SourceMetadata>>;
    async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>>;
}

#[derive(Clone)]
pub struct IcSourceCatalog {
    client: QueryClient,
    catalog_canister_id: String,
}

impl IcSourceCatalog {
    pub fn new(client: QueryClient, catalog_canister_id: String) -> Self {
        Self {
            client,
            catalog_canister_id,
        }
    }
}

impl SourceCatalog for IcSourceCatalog {
    async fn get_source(&self, source_id: &str) -> Result<SourceMetadata> {
        catalog::get_source(&self.client, &self.catalog_canister_id, source_id)
            .await?
            .ok_or_else(|| anyhow!("unknown source_id: {source_id}"))
    }

    async fn resolve_sources(&self, query: &str, limit: usize) -> Result<Vec<ResolvedSource>> {
        let resolved = catalog::resolve_sources(
            &self.client,
            &self.catalog_canister_id,
            query,
            limit.max(1) as u32,
        )
        .await?;
        Ok(resolved
            .into_iter()
            .map(|item| ResolvedSource {
                source_id: item.source_id,
                title: item.title,
                score: item.score,
                reasons: item.reasons,
            })
            .collect())
    }

    async fn list_sources(&self) -> Result<Vec<SourceMetadata>> {
        catalog::list_sources(&self.client, &self.catalog_canister_id).await
    }

    async fn filter_sources(&self, args: FilterSourcesArgs) -> Result<Vec<SourceMetadata>> {
        catalog::filter_sources(&self.client, &self.catalog_canister_id, &args).await
    }
}
