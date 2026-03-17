// Where: src/provider.rs
// What: Read-only source querying backed by the existing KINIC memory instance search API.
// Why: Existing production canisters expose `search`, not a dedicated `query_source` endpoint.
use anyhow::{Context, Result, anyhow};
use kinic_context_core::{client::QueryClient, memory};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

use crate::model::{SourceMetadata, SourceSnippet};

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
    embedding_base_url: String,
    fixed_embedding: Option<Vec<f32>>,
}

impl IcSourceQueryProvider {
    pub fn new(client: QueryClient) -> Self {
        Self {
            client,
            embedding_base_url: env::var("EMBEDDING_API_ENDPOINT")
                .unwrap_or_else(|_| "https://api.kinic.io".to_string()),
            fixed_embedding: None,
        }
    }

    pub fn with_fixed_embedding(client: QueryClient, embedding: Vec<f32>) -> Self {
        Self {
            client,
            embedding_base_url: env::var("EMBEDDING_API_ENDPOINT")
                .unwrap_or_else(|_| "https://api.kinic.io".to_string()),
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
            None => fetch_embedding(&self.embedding_base_url, query).await?,
        };
        let mut snippets = Vec::new();
        let mut errors = Vec::new();

        for canister_id in &source.canister_ids {
            match memory::search(&self.client, canister_id, embedding.clone()).await {
                Ok(search_results) => {
                    for (score, payload) in search_results {
                        let parsed = ParsedPayload::from_raw(&payload, &source.source_id);
                        if version.is_some() && parsed.version.as_deref() != version {
                            continue;
                        }

                        snippets.push(SourceSnippet {
                            source_id: source.source_id.clone(),
                            title: parsed.title,
                            snippet: parsed.snippet,
                            citation: parsed.citation,
                            trust: source.trust.clone(),
                            retrieved_at: source.retrieved_at.clone(),
                            version: parsed.version,
                            stale: false,
                            score,
                        });
                    }
                }
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

async fn fetch_embedding(base_url: &str, query: &str) -> Result<Vec<f32>> {
    let response = Client::new()
        .post(format!("{base_url}/embedding"))
        .json(&EmbeddingRequest { content: query })
        .send()
        .await
        .context("failed to call embedding endpoint")?;

    let response = response
        .error_for_status()
        .context("embedding endpoint returned an error")?;
    let payload = response
        .json::<EmbeddingResponse>()
        .await
        .context("failed to decode embedding response")?;
    Ok(payload.embedding)
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    content: &'a str,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Default)]
struct ParsedPayload {
    title: String,
    snippet: String,
    citation: String,
    version: Option<String>,
}

impl ParsedPayload {
    fn from_raw(payload: &str, source_id: &str) -> Self {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
            let snippet = json
                .get("snippet")
                .and_then(|value| value.as_str())
                .or_else(|| json.get("text").and_then(|value| value.as_str()))
                .or_else(|| json.get("content").and_then(|value| value.as_str()))
                .unwrap_or(payload)
                .to_string();
            let title = json
                .get("title")
                .and_then(|value| value.as_str())
                .map(ToString::to_string)
                .unwrap_or_else(|| summarize_title(&snippet));
            let citation = json
                .get("citation")
                .and_then(|value| value.as_str())
                .or_else(|| json.get("url").and_then(|value| value.as_str()))
                .or_else(|| json.get("source_url").and_then(|value| value.as_str()))
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("memory://{source_id}"));
            let version = json
                .get("version")
                .and_then(|value| value.as_str())
                .map(ToString::to_string);

            return Self {
                title,
                snippet,
                citation,
                version,
            };
        }

        Self {
            title: summarize_title(payload),
            snippet: payload.to_string(),
            citation: format!("memory://{source_id}"),
            version: None,
        }
    }
}

fn summarize_title(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Untitled memory result".to_string();
    }

    let summary: String = trimmed.chars().take(80).collect();
    if trimmed.chars().count() > 80 {
        format!("{summary}...")
    } else {
        summary
    }
}
