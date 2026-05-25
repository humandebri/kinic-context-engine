// Where: crates/kinic_context_core/src/types.rs
// What: Shared source metadata and retrieval snippet output types.
// Why: Use one canonical shape across CLI JSON output and source_ops tests.
use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct SourceMetadata {
    pub source_id: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub trust: String,
    pub domain: String,
    pub supported_versions: Vec<String>,
    pub retrieved_at: String,
    pub citations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct ResolvedCatalogSource {
    pub source_id: String,
    pub title: String,
    pub score: f32,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct FilterSourcesArgs {
    pub domain: Option<String>,
    pub trust: Option<String>,
    pub version: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct SourceUpsert {
    pub source_id: String,
    pub title: String,
    pub aliases: Vec<String>,
    pub trust: String,
    pub domain: String,
    pub supported_versions: Vec<String>,
    pub retrieved_at: String,
    pub citations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct SourceSnippet {
    pub source_id: String,
    pub title: String,
    pub snippet: String,
    pub citation: String,
    pub trust: String,
    pub retrieved_at: String,
    pub version: Option<String>,
    pub stale: bool,
    pub score: f32,
}
