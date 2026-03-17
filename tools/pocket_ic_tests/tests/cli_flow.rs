// Where: tools/pocket_ic_tests/tests/cli_flow.rs
// What: PocketIC-backed CLI binary tests for the public executable contract.
// Why: Keep the binary boundary focused on `resolve` so runtime-only test hooks stay out of production paths.
mod common;

use anyhow::Result;
use assert_cmd::Command;
use serde_json::Value;

use common::{
    fixtures::{nextjs_results, skill_results, source, supabase_results},
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

#[tokio::test]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
async fn cli_resolve_contract_works_against_pocket_ic() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
    let gateway = pic.make_live(None);
    let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
    let next_memory = install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
    let supabase_memory =
        install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
    let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;
    let skill_memory = install_fake_memory_instance(&mut pic, test_canisters, skill_results())?;

    replace_catalog(
        &pic,
        test_canisters,
        catalog_id,
        vec![
            source("/vercel/next.js", vec![next_memory.to_text()]),
            source("/supabase/docs", vec![supabase_memory.to_text()]),
            source("/react/docs", vec![react_memory.to_text()]),
            source("/skills/nextjs/migration", vec![skill_memory.to_text()]),
        ],
    )?;

    let resolve_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .arg("resolve")
        .arg("next middleware")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolved = parse_json(&resolve_output);
    assert_eq!(resolved["candidate_sources"][0]["source_id"], "/vercel/next.js");
    assert!(resolved.get("evidence").is_none());
    assert!(
        resolved["candidate_sources"]
            .as_array()
            .expect("candidate_sources should be an array")
            .iter()
            .all(|item| item["source_id"] != "/skills/nextjs/migration")
    );

    let resolve_with_skills_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .args(["resolve", "next migration", "--include-skills"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let resolved_with_skills = parse_json(&resolve_with_skills_output);
    assert!(
        resolved_with_skills["candidate_sources"]
            .as_array()
            .expect("candidate_sources should be an array")
            .iter()
            .any(|item| item["source_id"] == "/skills/nextjs/migration")
    );

    let listed_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .arg("list-sources")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed = parse_json(&listed_output);
    assert_eq!(listed["count"], 3);
    assert!(
        listed["sources"]
            .as_array()
            .expect("sources should be an array")
            .iter()
            .all(|item| item["domain"] != "skill_knowledge")
    );

    let listed_with_skills_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .args(["list-sources", "--include-skills"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let listed_with_skills = parse_json(&listed_with_skills_output);
    assert_eq!(listed_with_skills["count"], 4);

    let filtered_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
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

    let packed_budget_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .args(["pack", "protect route in next.js with supabase auth", "--max-tokens", "10"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let budget_pack = parse_json(&packed_budget_output);
    assert_eq!(
        budget_pack["evidence"]
            .as_array()
            .expect("evidence should be an array")
            .len(),
        0
    );
    assert_eq!(budget_pack["token_budget"], 10);

    let skill_filtered_output = cli()
        .env("KINIC_CONTEXT_CATALOG_CANISTER_ID", catalog_id.to_text())
        .env("KINIC_CONTEXT_IC_HOST", gateway.as_ref())
        .args(["filter-sources", "--domain", "skill_knowledge"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let skill_filtered = parse_json(&skill_filtered_output);
    assert_eq!(skill_filtered["count"], 1);
    assert_eq!(skill_filtered["sources"][0]["source_id"], "/skills/nextjs/migration");
    Ok(())
}

#[tokio::test]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
async fn cli_unknown_source_still_fails_before_provider_execution() -> Result<()> {
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
        .arg("query")
        .arg("/unknown/source")
        .arg("middleware")
        .assert()
        .failure();
    Ok(())
}
