// Where: crates/kinic_retrieval_types/src/lib.rs
// What: Canonical retrieval request and result types shared across engine and canister layers.
// Why: Eliminate duplicate retrieval type definitions and keep wire shapes aligned.
use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub struct HybridQueryFilters {
    pub section: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct HybridQueryRequest {
    pub query_text: String,
    pub query_embedding: Vec<f32>,
    pub version: Option<String>,
    pub top_k: u32,
    pub candidate_limit: Option<u32>,
    pub keyword_weight: Option<f32>,
    pub vector_weight: Option<f32>,
    pub filters: Option<HybridQueryFilters>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct HybridSearchResult {
    pub title: String,
    pub snippet: String,
    pub citation: String,
    pub version: Option<String>,
    pub score: f32,
    pub keyword_score: Option<f32>,
    pub vector_score: Option<f32>,
    pub section: Option<String>,
    pub tags: Option<Vec<String>>,
    pub match_reasons: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct IndexedDocument {
    pub title: String,
    pub snippet: String,
    pub citation: String,
    pub version: Option<String>,
    pub content: String,
    pub section: Option<String>,
    pub tags: Vec<String>,
    pub embedding: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct VectorSearchResult {
    pub score: f32,
    pub title: String,
    pub snippet: String,
    pub citation: String,
    pub version: Option<String>,
    pub section: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, CandidType, PartialEq)]
pub struct SectionIndexRecord {
    pub section_id: String,
    pub title: String,
    pub summary: String,
    pub version: Option<String>,
    pub embedding: Vec<f32>,
}
