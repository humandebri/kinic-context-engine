// Where: tools/pocket_ic_tests/tests/cli_flow.rs
// What: PocketIC-backed CLI binary tests for the public executable contract.
// Why: Keep the binary boundary focused on `resolve` so runtime-only test hooks stay out of production paths.
mod common;

use anyhow::Result;
use assert_cmd::Command;
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread::{self, JoinHandle},
};

use common::{
    fixtures::{nextjs_migration_results, nextjs_results, source, supabase_results},
    pocketic::{
        TestCanisters, ensure_pocket_ic_server, install_catalog_canister,
        install_fake_memory_instance, pocket_ic, replace_catalog,
    },
};

fn cli() -> Command {
    Command::cargo_bin("kinic-context-cli").expect("CLI binary should build")
}

fn parse_json(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("stdout should contain valid JSON")
}

fn mock_embedding_server() -> Result<(String, JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let handle = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0_u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = r#"{"embedding":[0.9,0.1,0.0,0.0],"model":"intfloat/multilingual-e5-large"}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body,
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Ok((format!("http://{}", address), handle))
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn cli_resolve_contract_works_against_pocket_ic() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let next_migration_memory =
        install_fake_memory_instance(&mut pic, test_canisters, nextjs_migration_results())?;
    let supabase_memory =
        install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source(
                "/vercel/next.js",
                vec![next_memory.to_text(), next_migration_memory.to_text()],
            ),
            source("/supabase/docs", vec![supabase_memory.to_text()]),
            source("/react/docs", vec![react_memory.to_text()]),
        ],
    )?;

    {
        let resolve_output = cli()
            .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
            .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
            .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
            .arg("resolve")
            .arg("next middleware")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let resolved = parse_json(&resolve_output);
        assert_eq!(
            resolved["candidate_sources"][0]["source_id"],
            "/vercel/next.js"
        );
        assert!(resolved.get("evidence").is_none());

        let listed_output = cli()
            .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
            .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
            .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
            .arg("list-sources")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let listed = parse_json(&listed_output);
        assert_eq!(listed["count"], 3);

        let filtered_output = cli()
            .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
            .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
            .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
            .args([
                "filter-sources",
                "--domain",
                "code_docs",
                "--trust",
                "official",
                "--version",
                "15",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let filtered = parse_json(&filtered_output);
        assert_eq!(filtered["count"], 1);
        assert_eq!(filtered["sources"][0]["source_id"], "/vercel/next.js");

        let migration_resolve_output = cli()
            .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
            .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
            .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
            .args(["resolve", "next migration"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let migration_resolved = parse_json(&migration_resolve_output);
        assert_eq!(
            migration_resolved["candidate_sources"][0]["source_id"],
            "/vercel/next.js"
        );

        let (endpoint, server) = mock_embedding_server()?;
        let packed_budget_output = cli()
            .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
            .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
            .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
            .env("EMBEDDING_API_ENDPOINT", endpoint)
            .args([
                "pack",
                "protect route in next.js with supabase auth",
                "--max-tokens",
                "10",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        server.join().expect("embedding mock server should stop");
        let budget_pack = parse_json(&packed_budget_output);
        assert_eq!(
            budget_pack["evidence"]
                .as_array()
                .expect("evidence should be an array")
                .len(),
            0
        );
        assert_eq!(budget_pack["token_budget"], 10);

        Ok(())
    }
}

#[test]
#[serial_test::serial]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn cli_unknown_source_still_fails_before_provider_execution() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source("/vercel/next.js", vec![next_memory.to_text()]),
            source("/supabase/docs", Vec::new()),
            source("/react/docs", Vec::new()),
        ],
    )?;

    cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .env("KINIC_CONTEXT_FETCH_ROOT_KEY", "true")
        .arg("query")
        .arg("/unknown/source")
        .arg("middleware")
        .assert()
        .failure();
    Ok(())
}
