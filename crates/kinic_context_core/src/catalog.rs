// Where: crates/kinic_context_core/src/catalog.rs
// What: Read-only catalog canister client helpers.
// Why: Keep catalog Candid calls centralized so the CLI and tests share one wire contract.
use anyhow::Result;

use crate::{
    client::QueryClient,
    types::{FilterSourcesArgs, ResolvedCatalogSource, SourceMetadata},
};

pub async fn list_sources(
    client: &QueryClient,
    catalog_canister_id: &str,
) -> Result<Vec<SourceMetadata>> {
    client
        .query_args(catalog_canister_id, "list_sources", ())
        .await
}

pub async fn get_source(
    client: &QueryClient,
    catalog_canister_id: &str,
    source_id: &str,
) -> Result<Option<SourceMetadata>> {
    client
        .query_args(
            catalog_canister_id,
            "get_source",
            (source_id.to_string(),),
        )
        .await
}

pub async fn resolve_sources(
    client: &QueryClient,
    catalog_canister_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<ResolvedCatalogSource>> {
    client
        .query_args(
            catalog_canister_id,
            "resolve_sources",
            (query.to_string(), limit),
        )
        .await
}

pub async fn filter_sources(
    client: &QueryClient,
    catalog_canister_id: &str,
    args: &FilterSourcesArgs,
) -> Result<Vec<SourceMetadata>> {
    client
        .query(catalog_canister_id, "filter_sources", args)
        .await
}
