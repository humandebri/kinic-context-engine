// Where: crates/kinic_context_core/src/memory.rs
// What: Read-only memory instance canister calls using the existing instance.did interface.
// Why: Reuse the current KINIC memory search backend rather than assuming new source canisters exist.
use anyhow::Result;

use crate::client::QueryClient;

const SEARCH_METHOD: &str = "search";

pub async fn search(
    client: &QueryClient,
    memory_canister_id: &str,
    embedding: Vec<f32>,
) -> Result<Vec<(f32, String)>> {
    client
        .query::<Vec<f32>, Vec<(f32, String)>>(memory_canister_id, SEARCH_METHOD, embedding)
        .await
}
