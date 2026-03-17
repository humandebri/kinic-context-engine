// Where: src/output.rs
// What: JSON rendering helpers for CLI output.
// Why: Make structured output the default while still allowing human-friendly indentation.
use anyhow::Result;

use crate::model::CommandOutput;

pub fn render_json(output: &CommandOutput, pretty: bool) -> Result<String> {
    if pretty {
        Ok(serde_json::to_string_pretty(output)?)
    } else {
        Ok(serde_json::to_string(output)?)
    }
}
