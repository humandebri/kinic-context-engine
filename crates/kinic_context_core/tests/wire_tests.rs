// Where: crates/kinic_context_core/tests/wire_tests.rs
// What: Candid wire-shape tests for source metadata and launcher queries.
// Why: Keep decode expectations aligned with the remaining public output contracts.
use candid::{Decode, Encode};
use kinic_context_core::{
    launcher::LauncherState,
    types::{FilterSourcesArgs, ResolvedCatalogSource, SourceMetadata},
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
        supported_versions: vec!["15".to_string()],
        retrieved_at: "2026-03-17T00:00:00Z".to_string(),
        citations: vec!["https://nextjs.org/docs".to_string()],
    }))
    .expect("encode should succeed");
    let decoded = Decode!(&encoded, Option<SourceMetadata>).expect("decode should succeed");
    assert_eq!(
        decoded.expect("source should exist").source_id,
        "/vercel/next.js"
    );
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
