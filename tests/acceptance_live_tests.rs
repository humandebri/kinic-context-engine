// Where: tests/acceptance_live_tests.rs
// What: Opt-in live acceptance coverage against the catalog canister and memory instances.
// Why: Validate the catalog-based read-only flow against real canisters without adding write paths.
use assert_cmd::Command;
use serde_json::Value;

use kinic_context_core::{catalog, client::QueryClient, config::ReadConfig, launcher, memory};

fn extract_object(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("stdout should be valid JSON")
}

#[test]
#[ignore = "requires live catalog and memory instance canisters"]
fn pack_succeeds_against_live_catalog() {
    let config = ReadConfig::from_env().expect("live config should exist");
    let output = Command::cargo_bin("kinic-context-cli")
        .expect("binary should build")
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", config.catalog_canister_id)
        .env("KINIC_CONTEXT_IC_HOST", config.ic_host)
        .args(["pack", "protect route in next.js with supabase auth"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let json = extract_object(&output);
    assert!(json["resolved_sources"]
        .as_array()
        .expect("resolved_sources should be an array")
        .len()
        > 0);
    assert!(json["evidence"]
        .as_array()
        .expect("evidence should be an array")
        .len()
        > 0);
}

#[tokio::test]
#[ignore = "requires live catalog and memory instance canisters"]
async fn catalog_resolves_source_and_memory_search_succeeds() {
    let config = ReadConfig::from_env().expect("live config should exist");
    let client = QueryClient::new(&config.ic_host, config.fetch_root_key)
        .await
        .expect("query client should be created");
    let nextjs = catalog::get_source(&client, &config.catalog_canister_id, "/vercel/next.js")
        .await
        .expect("catalog query should succeed")
        .expect("next.js source should exist");

    let canister_id = nextjs
        .canister_ids
        .first()
        .expect("next.js source should expose a memory instance canister");
    let embedding = vec![0.0_f32; 1024];
    let _ = memory::search(&client, canister_id, embedding)
        .await
        .expect("memory search should succeed");
}

#[tokio::test]
#[ignore = "requires live catalog, launcher, and memory instance canisters"]
async fn launcher_lists_instances_when_configured() {
    let config = ReadConfig::from_env().expect("live config should exist");
    let launcher_id = config
        .launcher_canister_id
        .as_deref()
        .expect("KINIC_CONTEXT_LAUNCHER_CANISTER_ID must be set");
    let client = QueryClient::new(&config.ic_host, config.fetch_root_key)
        .await
        .expect("query client should be created");

    let states = launcher::list_instances(&client, launcher_id)
        .await
        .expect("launcher list_instance should succeed");
    assert!(!states.is_empty());
}
