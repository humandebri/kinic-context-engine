// Where: src/bin/kinic-embed.rs
// What: Small stdin/stdout wrapper around the Rust multilingual-e5-large runtime.
// Why: Let tools/source_ops reuse the same local embedding implementation as the CLI.
use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use kinic_context_cli::{
    e5,
    embed_protocol::{EmbedKind, EmbedRequest, EmbedResponse, MODEL_NAME},
};

fn main() -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin()
        .read_to_string(&mut stdin)
        .context("failed to read stdin")?;
    if stdin.trim().is_empty() {
        bail!("stdin JSON payload is required");
    }
    let request: EmbedRequest =
        serde_json::from_str(&stdin).context("failed to decode embed request JSON")?;
    match request.kind {
        EmbedKind::Query | EmbedKind::Document | EmbedKind::Section => {}
    }
    let response = EmbedResponse {
        embedding: e5::embed_text(&request.text)?,
        model: MODEL_NAME.to_string(),
    };
    std::io::stdout()
        .write_all(
            serde_json::to_string(&response)
                .context("failed to encode embed response JSON")?
                .as_bytes(),
        )
        .context("failed to write stdout")?;
    Ok(())
}
