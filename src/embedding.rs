// Where: src/embedding.rs
// What: Shared query/document embedding client and text normalization helpers.
// Why: Keep one backend selection path for CLI reads and source_ops writes.
use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{env, path::PathBuf};

use crate::{e5, embed_protocol::MODEL_NAME};

const DEFAULT_QUERY_PREFIX: &str = "query: ";
const DEFAULT_DOCUMENT_PREFIX: &str = "passage: ";

#[derive(Clone)]
pub struct EmbeddingClient {
    http: Client,
    backend: EmbeddingBackend,
    model: String,
    query_prefix: String,
    document_prefix: String,
}

#[derive(Clone)]
enum EmbeddingBackend {
    Local(LocalBackend),
    Remote(RemoteBackend),
}

#[derive(Clone)]
struct LocalBackend {
    model_dir: PathBuf,
}

#[derive(Clone)]
struct RemoteBackend {
    base_url: String,
}

impl EmbeddingClient {
    pub fn from_env() -> Self {
        Self {
            http: Client::new(),
            backend: backend_from_env(),
            model: env::var("KINIC_CONTEXT_EMBEDDING_MODEL")
                .unwrap_or_else(|_| MODEL_NAME.to_string()),
            query_prefix: env::var("KINIC_CONTEXT_EMBEDDING_QUERY_PREFIX")
                .unwrap_or_else(|_| DEFAULT_QUERY_PREFIX.to_string()),
            document_prefix: env::var("KINIC_CONTEXT_EMBEDDING_DOCUMENT_PREFIX")
                .unwrap_or_else(|_| DEFAULT_DOCUMENT_PREFIX.to_string()),
        }
    }

    pub async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embed_text(self.query_text(query)).await
    }

    pub fn query_text(&self, query: &str) -> String {
        prefixed(&self.query_prefix, query)
    }

    pub fn document_text(&self, title: &str, snippet: &str, content: &str) -> String {
        join_non_empty([
            prefixed(&self.document_prefix, title),
            snippet.trim().to_string(),
            content.trim().to_string(),
        ])
    }

    pub fn section_text(&self, section_id: &str, title: &str, summary: &str) -> String {
        let heading = if title.trim().is_empty() {
            section_id
        } else {
            title
        };
        join_non_empty([
            prefixed(&self.document_prefix, heading),
            summary.trim().to_string(),
            String::new(),
        ])
    }

    async fn embed_text(&self, text: String) -> Result<Vec<f32>> {
        match &self.backend {
            EmbeddingBackend::Remote(backend) => self.embed_remote(backend, text).await,
            EmbeddingBackend::Local(backend) => self.embed_local(backend, text).await,
        }
    }

    async fn embed_remote(&self, backend: &RemoteBackend, text: String) -> Result<Vec<f32>> {
        let response = self
            .http
            .post(format!("{}/embedding", backend.base_url))
            .json(&RemoteEmbeddingRequest {
                content: text,
                model: self.model.clone(),
            })
            .send()
            .await
            .context("failed to call embedding endpoint")?;
        let response = response
            .error_for_status()
            .context("embedding endpoint returned an error")?;
        let payload = response
            .json::<RemoteEmbeddingResponse>()
            .await
            .context("failed to decode embedding response")?;
        Ok(payload.embedding)
    }

    async fn embed_local(&self, backend: &LocalBackend, text: String) -> Result<Vec<f32>> {
        let model_dir = backend.model_dir.clone();
        tokio::task::spawn_blocking(move || e5::embed_text_with_model_dir(model_dir, &text))
        .await
        .context("local embedding task failed")?
    }

    #[cfg(test)]
    fn backend_kind(&self) -> &'static str {
        match self.backend {
            EmbeddingBackend::Local(_) => "local",
            EmbeddingBackend::Remote(_) => "remote",
        }
    }

    #[cfg(test)]
    fn local_model_dir(&self) -> Option<PathBuf> {
        match &self.backend {
            EmbeddingBackend::Local(backend) => Some(backend.model_dir.clone()),
            EmbeddingBackend::Remote(_) => None,
        }
    }
}

pub fn document_input_text(title: &str, snippet: &str, content: &str) -> String {
    EmbeddingClient::from_env().document_text(title, snippet, content)
}

pub fn section_input_text(section_id: &str, title: &str, summary: &str) -> String {
    EmbeddingClient::from_env().section_text(section_id, title, summary)
}

fn backend_from_env() -> EmbeddingBackend {
    match env::var("EMBEDDING_API_ENDPOINT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        Some(base_url) => EmbeddingBackend::Remote(RemoteBackend { base_url }),
        None => EmbeddingBackend::Local(LocalBackend {
            model_dir: e5::resolved_model_dir(),
        }),
    }
}

fn prefixed(prefix: &str, text: &str) -> String {
    format!("{}{}", prefix, text.trim())
}

fn join_non_empty(parts: [String; 3]) -> String {
    parts
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Serialize)]
struct RemoteEmbeddingRequest {
    content: String,
    model: String,
}

#[derive(Deserialize)]
struct RemoteEmbeddingResponse {
    embedding: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        sync::{Mutex, OnceLock},
    };

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn text_helpers_apply_expected_prefixes() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            env::remove_var("EMBEDDING_API_ENDPOINT");
            env::remove_var("KINIC_CONTEXT_EMBEDDING_QUERY_PREFIX");
            env::remove_var("KINIC_CONTEXT_EMBEDDING_DOCUMENT_PREFIX");
        }
        let client = EmbeddingClient::from_env();
        assert_eq!(client.query_text("middleware"), "query: middleware");
        assert_eq!(
            client.document_text("Next.js Middleware", "Inspect cookies.", "Full chunk"),
            "passage: Next.js Middleware\n\nInspect cookies.\n\nFull chunk"
        );
        assert_eq!(
            client.section_text("middleware", "Middleware", "Inspect requests."),
            "passage: Middleware\n\nInspect requests."
        );
    }

    #[test]
    fn backend_selection_prefers_remote_only_when_endpoint_is_set() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            env::remove_var("EMBEDDING_API_ENDPOINT");
        }
        let local = EmbeddingClient::from_env();
        assert_eq!(local.backend_kind(), "local");
        unsafe {
            env::set_var("EMBEDDING_API_ENDPOINT", "http://127.0.0.1:9999");
        }
        let remote = EmbeddingClient::from_env();
        assert_eq!(remote.backend_kind(), "remote");
        unsafe {
            env::remove_var("EMBEDDING_API_ENDPOINT");
        }
    }

    #[test]
    fn local_backend_uses_default_model_dir() {
        let _guard = env_lock().lock().expect("env lock");
        unsafe {
            env::remove_var("EMBEDDING_API_ENDPOINT");
            env::remove_var("KINIC_CONTEXT_EMBEDDING_MODEL_DIR");
        }
        let client = EmbeddingClient::from_env();
        assert_eq!(client.local_model_dir(), Some(e5::default_model_dir()));
    }
}
