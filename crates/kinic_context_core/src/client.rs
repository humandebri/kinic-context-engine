// Where: crates/kinic_context_core/src/client.rs
// What: Anonymous query-only IC agent wrapper.
// Why: Reuse safe query behavior without pulling in keyring, identity, or ledger code.
use anyhow::{Context, Result};
use candid::{CandidType, Decode, encode_args};
use ic_agent::{Agent, export::Principal};
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct QueryClient {
    agent: Agent,
}

impl QueryClient {
    pub async fn new(host: &str, fetch_root_key: bool) -> Result<Self> {
        let agent = Agent::builder().with_url(host).build()?;
        if fetch_root_key {
            agent.fetch_root_key().await?;
        }
        Ok(Self { agent })
    }

    pub async fn query<TArg, TRes>(
        &self,
        canister_id: &str,
        method: &str,
        args: TArg,
    ) -> Result<TRes>
    where
        TArg: CandidType,
        TRes: CandidType + DeserializeOwned,
    {
        let payload = candid::encode_one(args)?;
        self.query_raw(canister_id, method, payload).await
    }

    pub async fn query_args<TRes>(
        &self,
        canister_id: &str,
        method: &str,
        args: impl candid::utils::ArgumentEncoder,
    ) -> Result<TRes>
    where
        TRes: CandidType + DeserializeOwned,
    {
        let payload = encode_args(args)?;
        self.query_raw(canister_id, method, payload).await
    }

    pub async fn query_raw<TRes>(
        &self,
        canister_id: &str,
        method: &str,
        payload: Vec<u8>,
    ) -> Result<TRes>
    where
        TRes: CandidType + DeserializeOwned,
    {
        let canister = Principal::from_text(canister_id)
            .with_context(|| format!("failed to parse canister id `{canister_id}`"))?;
        let response = self
            .agent
            .query(&canister, method)
            .with_arg(payload)
            .call()
            .await
            .with_context(|| format!("failed to query method `{method}` on `{canister_id}`"))?;

        Decode!(&response, TRes).with_context(|| {
            format!("failed to decode response from `{method}` on `{canister_id}`")
        })
    }
}
