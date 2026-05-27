// Where: crates/kinic_context_core/src/config.rs
// What: Runtime settings for Kinic Wiki CLI-backed reads.
// Why: Centralize environment-derived parameters so the CLI has one source of truth.
use anyhow::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadConfig {
    pub database_id: String,
    pub wiki_cli_bin: String,
}

impl ReadConfig {
    pub fn from_env() -> Result<Self> {
        let database_id = std::env::var("KINIC_CONTEXT_DATABASE_ID")
            .or_else(|_| std::env::var("VFS_DATABASE_ID"))
            .unwrap_or_default();
        let wiki_cli_bin = std::env::var("KINIC_CONTEXT_WIKI_CLI_BIN")
            .unwrap_or_else(|_| "kinic-vfs-cli".to_string());
        Ok(Self {
            database_id,
            wiki_cli_bin,
        })
    }
}
