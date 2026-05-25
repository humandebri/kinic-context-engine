// Where: src/wiki_cli.rs
// What: Small JSON command runner for kinic-vfs-cli adapters.
// Why: Keep wiki CLI subprocess handling shared and isolated from catalog/query logic.
use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::process::Command;

pub(crate) fn run_json<T: for<'de> Deserialize<'de>>(
    cli_bin: &str,
    database_id: &str,
    args: Vec<String>,
    error_label: &str,
) -> Result<T> {
    let mut parts = cli_bin.split_whitespace();
    let Some(program) = parts.next() else {
        return Err(anyhow!("KINIC_CONTEXT_WIKI_CLI_BIN must not be empty"));
    };
    let mut command = Command::new(program);
    command.args(parts);
    if !database_id.is_empty() {
        command.args(["--database-id", database_id]);
    }
    command.args(args);
    let output = command.output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{}: {}",
            error_label,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
