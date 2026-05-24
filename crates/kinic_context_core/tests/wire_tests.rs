// Where: crates/kinic_context_core/tests/wire_tests.rs
// What: Candid wire-shape tests for catalog, launcher, and memory instance queries.
// Why: Keep decode expectations aligned with the canister interfaces the CLI depends on.
use candid::{Decode, Encode};
use kinic_context_core::{
    launcher::LauncherState,
    types::{
        FilterSourcesArgs, HybridQueryFilters, HybridQueryRequest, HybridSearchResult,
        IndexedDocument, ResolvedCatalogSource, SectionIndexRecord, SourceMetadata, SourceUpsert,
    },
};

#[test]
fn catalog_list_request_round_trip_decodes_as_unit_args() {
    let encoded = candid::encode_args(()).expect("encode should succeed");
    let decoded = candid::decode_args::<()>(&encoded).expect("decode should succeed");
    assert_eq!(decoded, ());
}

#[test]
fn catalog_resolve_request_round_trip_decodes_as_tuple_args() {
    let encoded =
        candid::encode_args(("next middleware".to_string(), 3_u32)).expect("encode should succeed");
    let decoded = candid::decode_args::<(String, u32)>(&encoded).expect("decode should succeed");
    assert_eq!(decoded.1, 3);
}

#[test]
fn catalog_filter_request_round_trip_decodes_as_record() {
    let args = FilterSourcesArgs {
        domain: Some("code_docs".to_string()),
        trust: Some("official".to_string()),
        version: Some("15".to_string()),
        limit: Some(5),
    };
    let encoded = candid::encode_one(&args).expect("encode should succeed");
    let decoded = candid::decode_one::<FilterSourcesArgs>(&encoded).expect("decode should succeed");
    assert_eq!(decoded.version.as_deref(), Some("15"));
}

#[test]
fn memory_search_request_round_trip_decodes_as_embedding_vec() {
    let encoded =
        candid::encode_one(vec![0.1_f32, 0.2_f32, 0.3_f32]).expect("encode should succeed");
    let decoded = candid::decode_one::<Vec<f32>>(&encoded).expect("decode should succeed");
    assert_eq!(decoded.len(), 3);
}

#[test]
fn hybrid_query_request_round_trip_decodes_as_record() {
    let encoded = candid::encode_one(HybridQueryRequest {
        query_text: "LaunchAgent".to_string(),
        query_embedding: vec![0.1_f32, 0.2_f32, 0.3_f32],
        version: Some("1".to_string()),
        top_k: 3,
        candidate_limit: Some(24),
        keyword_weight: Some(0.65),
        vector_weight: Some(0.35),
        filters: Some(HybridQueryFilters {
            section: Some("launchd".to_string()),
            tags: vec!["tailscale".to_string(), "macos".to_string()],
        }),
    })
    .expect("encode should succeed");
    let decoded =
        candid::decode_one::<HybridQueryRequest>(&encoded).expect("decode should succeed");
    assert_eq!(decoded.query_text, "LaunchAgent");
    assert_eq!(decoded.top_k, 3);
    assert_eq!(decoded.candidate_limit, Some(24));
    assert_eq!(decoded.filters.expect("filters should exist").tags.len(), 2);
}

#[test]
fn hybrid_query_request_old_shape_still_decodes() {
    #[derive(candid::CandidType)]
    struct OldHybridQueryRequest {
        query_text: String,
        query_embedding: Vec<f32>,
        version: Option<String>,
        top_k: u32,
    }

    let encoded = candid::encode_one(OldHybridQueryRequest {
        query_text: "LaunchAgent".to_string(),
        query_embedding: vec![0.1_f32, 0.2_f32, 0.3_f32],
        version: Some("1".to_string()),
        top_k: 3,
    })
    .expect("encode should succeed");
    let decoded =
        candid::decode_one::<HybridQueryRequest>(&encoded).expect("decode should succeed");
    assert_eq!(decoded.query_text, "LaunchAgent");
    assert!(decoded.candidate_limit.is_none());
    assert!(decoded.filters.is_none());
}

#[test]
fn catalog_resolve_response_round_trip_decodes() {
    let encoded = Encode!(&vec![ResolvedCatalogSource {
        source_id: "/vercel/next.js".to_string(),
        title: "Next.js Docs".to_string(),
        score: 1.2,
        reasons: vec!["matched alias `next`".to_string()],
    }])
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Vec<ResolvedCatalogSource>).expect("decode should succeed");
    assert_eq!(decoded[0].source_id, "/vercel/next.js");
}

#[test]
fn catalog_get_response_round_trip_decodes() {
    let encoded = Encode!(&Some(SourceMetadata {
        source_id: "/vercel/next.js".to_string(),
        title: "Next.js Docs".to_string(),
        aliases: vec!["next".to_string()],
        trust: "official".to_string(),
        domain: "code_docs".to_string(),
        skill_kind: None,
        targets: Vec::new(),
        capabilities: Vec::new(),
        canister_ids: vec!["aaaaa-aa".to_string()],
        supported_versions: vec!["15".to_string()],
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        citations: vec!["https://nextjs.org/docs".to_string()],
    }))
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Option<SourceMetadata>).expect("decode should succeed");
    assert_eq!(decoded.expect("source should exist").canister_ids.len(), 1);
}

#[test]
fn catalog_get_response_old_skill_shape_round_trip_decodes() {
    let encoded = Encode!(&Some(SourceMetadata {
        source_id: "/skills/nextjs/migration".to_string(),
        title: "Next.js Migration Skill".to_string(),
        aliases: vec!["next migration".to_string()],
        trust: "curated".to_string(),
        domain: "skill_knowledge".to_string(),
        skill_kind: Some("migration".to_string()),
        targets: vec!["nextjs".to_string()],
        capabilities: vec!["auth".to_string(), "routing".to_string()],
        canister_ids: vec!["aaaaa-aa".to_string()],
        supported_versions: Vec::new(),
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        citations: vec!["https://github.com/ICME-Lab/kinic-context-engine".to_string()],
    }))
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Option<SourceMetadata>).expect("decode should succeed");
    let source = decoded.expect("source should exist");
    assert_eq!(source.skill_kind.as_deref(), Some("migration"));
    assert_eq!(source.targets, vec!["nextjs".to_string()]);
    assert_eq!(source.capabilities.len(), 2);
}

#[test]
fn catalog_upsert_request_skill_shape_round_trip_decodes() {
    let encoded = Encode!(&SourceUpsert {
        source_id: "/skills/nextjs/migration".to_string(),
        title: "Next.js Migration Skill".to_string(),
        aliases: vec!["next migration".to_string()],
        trust: "curated".to_string(),
        domain: "skill_knowledge".to_string(),
        skill_kind: Some("migration".to_string()),
        targets: vec!["nextjs".to_string()],
        capabilities: vec!["auth".to_string()],
        canister_ids: vec!["aaaaa-aa".to_string()],
        supported_versions: Vec::new(),
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        citations: vec!["https://github.com/ICME-Lab/kinic-context-engine".to_string()],
    })
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, SourceUpsert).expect("decode should succeed");
    assert_eq!(decoded.skill_kind.as_deref(), Some("migration"));
    assert_eq!(decoded.targets, vec!["nextjs".to_string()]);
    assert_eq!(decoded.capabilities, vec!["auth".to_string()]);
}

#[test]
fn launcher_list_response_round_trip_decodes() {
    let encoded = Encode!(&vec![
        LauncherState::Running(candid::Principal::anonymous())
    ])
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Vec<LauncherState>).expect("decode should succeed");
    assert_eq!(decoded.len(), 1);
}

#[test]
fn memory_search_response_round_trip_decodes() {
    let encoded = Encode!(&vec![(
        0.91_f32,
        "{\"title\":\"Next.js Middleware\"}".to_string()
    )])
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Vec<(f32, String)>).expect("decode should succeed");
    assert_eq!(decoded[0].0, 0.91_f32);
}

#[test]
fn hybrid_query_response_round_trip_decodes() {
    let encoded = Encode!(&vec![HybridSearchResult {
        title: "Tailscale LaunchAgent".to_string(),
        snippet: "Use a LaunchAgent plist on macOS.".to_string(),
        citation: "https://tailscale.com/kb/launchagent".to_string(),
        version: Some("1".to_string()),
        score: 0.42,
        keyword_score: Some(0.8),
        vector_score: Some(0.2),
        section: Some("launchd".to_string()),
        tags: Some(vec!["tailscale".to_string(), "macos".to_string()]),
        match_reasons: Some(vec![
            "keyword:title".to_string(),
            "vector:candidate".to_string(),
        ]),
    }])
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Vec<HybridSearchResult>).expect("decode should succeed");
    assert_eq!(decoded[0].title, "Tailscale LaunchAgent");
    assert_eq!(
        decoded[0].tags.as_ref().expect("tags should exist").len(),
        2
    );
}

#[test]
fn hybrid_query_response_old_shape_still_decodes() {
    #[derive(candid::CandidType)]
    struct OldHybridSearchResult {
        title: String,
        snippet: String,
        citation: String,
        version: Option<String>,
        score: f32,
    }

    let encoded = Encode!(&vec![OldHybridSearchResult {
        title: "Tailscale LaunchAgent".to_string(),
        snippet: "Use a LaunchAgent plist on macOS.".to_string(),
        citation: "https://tailscale.com/kb/launchagent".to_string(),
        version: Some("1".to_string()),
        score: 0.42,
    }])
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Vec<HybridSearchResult>).expect("decode should succeed");
    assert!(decoded[0].keyword_score.is_none());
    assert!(decoded[0].tags.is_none());
}

#[test]
fn indexed_document_round_trip_decodes() {
    let encoded = Encode!(&IndexedDocument {
        title: "Next.js Middleware".to_string(),
        snippet: "Use middleware to inspect requests.".to_string(),
        citation: "https://nextjs.org/docs/middleware".to_string(),
        version: Some("15".to_string()),
        content: "Full middleware chunk".to_string(),
        section: Some("middleware".to_string()),
        tags: vec!["next.js".to_string(), "auth".to_string()],
        embedding: vec![0.1_f32, 0.2_f32, 0.3_f32],
    })
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, IndexedDocument).expect("decode should succeed");
    assert_eq!(decoded.title, "Next.js Middleware");
    assert_eq!(decoded.tags.len(), 2);
    assert_eq!(decoded.embedding.len(), 3);
}

#[test]
fn section_index_record_round_trip_decodes() {
    let encoded = Encode!(&SectionIndexRecord {
        section_id: "middleware".to_string(),
        title: "Middleware".to_string(),
        summary: "Inspect cookies and redirect requests.".to_string(),
        version: Some("15".to_string()),
        embedding: vec![0.4_f32, 0.5_f32, 0.6_f32],
    })
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, SectionIndexRecord).expect("decode should succeed");
    assert_eq!(decoded.section_id, "middleware");
    assert_eq!(decoded.embedding.len(), 3);
}
