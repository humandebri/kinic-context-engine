// Where: tools/fake_memory_instance/src/hybrid_adapter.rs
// What: Adapter layer between canister wire types and the standalone hybrid SQLite engine.
// Why: Keep the canister API stable while delegating document retrieval to ic-hybrid-sqlite.
use ic_hybrid_engine as hybrid;
use kinic_context_core::types::{
    HybridQueryFilters, HybridQueryRequest, HybridSearchResult, IndexedDocument,
};
use serde_json::json;

pub fn to_engine_document(document: &IndexedDocument) -> hybrid::IndexedDocument {
    hybrid::IndexedDocument {
        external_id: None,
        kind: None,
        title: document.title.clone(),
        snippet: document.snippet.clone(),
        citation: document.citation.clone(),
        content: document.content.clone(),
        version: document.version.clone(),
        section: document.section.clone(),
        tags: document.tags.clone(),
        embedding: document.embedding.clone(),
        updated_at: None,
    }
}

pub fn to_engine_request(
    request: &HybridQueryRequest,
    filters: &HybridQueryFilters,
) -> hybrid::HybridQueryRequest {
    hybrid::HybridQueryRequest {
        query_text: request.query_text.clone(),
        query_embedding: request.query_embedding.clone(),
        version: request.version.clone(),
        top_k: request.top_k,
        keyword_candidate_limit: request.candidate_limit,
        vector_candidate_limit: request.candidate_limit,
        keyword_weight: request.keyword_weight,
        vector_weight: request.vector_weight,
        scoring_policy: None,
        filters: Some(hybrid::HybridQueryFilters {
            section: filters.section.clone(),
            tags: filters.tags.clone(),
            kinds: Vec::new(),
        }),
    }
}

pub fn to_wire_result(result: hybrid::HybridSearchResult) -> HybridSearchResult {
    HybridSearchResult {
        title: result.document.title,
        snippet: result.document.snippet,
        citation: result.document.citation,
        version: result.document.version,
        score: result.score,
        keyword_score: Some(result.breakdown.keyword_score),
        vector_score: Some(result.breakdown.vector_score),
        section: result.document.section,
        tags: Some(result.document.tags),
        match_reasons: Some(result.match_reasons),
    }
}

pub fn search_payload(result: &hybrid::VectorSearchResult) -> String {
    json!({
        "title": result.document.title,
        "snippet": result.document.snippet,
        "citation": result.document.citation,
        "version": result.document.version,
        "section": result.document.section,
        "tags": result.document.tags,
    })
    .to_string()
}
