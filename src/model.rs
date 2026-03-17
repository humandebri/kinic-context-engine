// Where: src/model.rs
// What: Command-level data types for resolution, packs, and citations.
// Why: Separate CLI response envelopes from the shared IC wire types in the core crate.
pub use kinic_context_core::types::{SourceMetadata, SourceSnippet};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Intent {
    Code,
    Travel,
    General,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResolvedSource {
    pub source_id: String,
    pub title: String,
    pub score: f32,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ResolveOutput {
    pub query: String,
    pub intent: Intent,
    pub entities: Vec<String>,
    pub candidate_sources: Vec<ResolvedSource>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QueryOutput {
    pub query: String,
    pub source_id: String,
    pub snippets: Vec<SourceSnippet>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Warning {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvidencePack {
    pub query: String,
    pub resolved_sources: Vec<String>,
    pub evidence: Vec<SourceSnippet>,
    pub warnings: Vec<Warning>,
    pub pack_summary: String,
    pub token_budget: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CitationEntry {
    pub source_id: String,
    pub title: String,
    pub citation: String,
    pub trust: String,
    pub retrieved_at: String,
    pub version: Option<String>,
    pub stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CitationOutput {
    pub query: String,
    pub citations: Vec<CitationEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFilters {
    pub domain: Option<String>,
    pub trust: Option<String>,
    pub version: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcesOutput {
    pub sources: Vec<SourceMetadata>,
    pub count: usize,
    pub filters: Option<SourceFilters>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum CommandOutput {
    Resolve(ResolveOutput),
    Query(QueryOutput),
    Pack(EvidencePack),
    Cite(CitationOutput),
    ListSources(SourcesOutput),
    FilterSources(SourcesOutput),
}
