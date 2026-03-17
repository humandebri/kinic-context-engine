// Where: crates/kinic_context_core/src/config.rs
// What: Runtime settings for anonymous read-only IC access.
// Why: Centralize environment-derived parameters so the CLI has one source of truth.
use anyhow::{Result, anyhow};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadConfig {
    pub ic_host: String,
    pub catalog_canister_id: String,
    pub launcher_canister_id: Option<String>,
    pub fetch_root_key: bool,
}

impl ReadConfig {
    pub fn from_env() -> Result<Self> {
        let catalog_canister_id = std::env::var("KINIC_CONTEXT_CATALOG_CANISTER_ID")
            .map_err(|_| anyhow!("KINIC_CONTEXT_CATALOG_CANISTER_ID is required"))?;
        let ic_host = std::env::var("KINIC_CONTEXT_IC_HOST")
            .unwrap_or_else(|_| "https://ic0.app".to_string());
        let launcher_canister_id = std::env::var("KINIC_CONTEXT_LAUNCHER_CANISTER_ID").ok();
        let fetch_root_key = std::env::var("KINIC_CONTEXT_FETCH_ROOT_KEY")
            .ok()
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes"))
            .unwrap_or(false);

        Ok(Self {
            ic_host,
            catalog_canister_id,
            launcher_canister_id,
            fetch_root_key,
        })
    }
}
