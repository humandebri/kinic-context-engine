// Where: tests/cli_tests.rs
// What: CLI-level safety and read-only behavior checks.
// Why: Ensure the public interface stays minimal and cite works without IC configuration.
use std::io::Write;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;
use tempfile::NamedTempFile;

fn bin() -> Command {
    Command::cargo_bin("kinic-context-cli").expect("binary should build")
}

#[test]
fn cite_extracts_citations_from_pack_file_without_env() {
    let pack = r#"{"query":"protect route in next.js with supabase auth","resolved_sources":["/vercel/next.js"],"evidence":[{"source_id":"/vercel/next.js","title":"Next.js Middleware","snippet":"Use middleware to inspect requests.","citation":"https://nextjs.org/docs/app/building-your-application/routing/middleware","trust":"official","retrieved_at":"2026-03-17T00:00:00Z","version":"15","stale":false,"score":1.0}],"warnings":[],"pack_summary":"Top evidence came from: Next.js Middleware","token_budget":3000}"#;
    let mut file = NamedTempFile::new().expect("temp file should be created");
    file.write_all(pack.as_bytes())
        .expect("pack fixture should be written");

    bin()
        .args(["cite", file.path().to_str().expect("utf-8 path")])
        .assert()
        .success()
        .stdout(contains(
            "https://nextjs.org/docs/app/building-your-application/routing/middleware",
        ));
}

#[test]
fn help_does_not_expose_write_commands() {
    bin()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("resolve"))
        .stdout(contains("query"))
        .stdout(contains("pack"))
        .stdout(contains("cite"))
        .stdout(contains("list-sources"))
        .stdout(contains("filter-sources"))
        .stdout(predicates::str::contains("insert").not())
        .stdout(predicates::str::contains("update").not())
        .stdout(predicates::str::contains("balance").not());
}

#[test]
fn runtime_manifest_does_not_pull_write_side_dependencies() {
    let manifest = std::fs::read_to_string("Cargo.toml").expect("Cargo.toml should exist");
    assert!(!manifest.contains("keyring"));
    assert!(!manifest.contains("ledger"));
    assert!(!manifest.contains("icrc-ledger-types"));
}

#[test]
fn help_exposes_include_skills_on_catalog_and_pack_commands() {
    bin()
        .args(["resolve", "--help"])
        .assert()
        .success()
        .stdout(contains("--include-skills"));

    bin()
        .args(["pack", "--help"])
        .assert()
        .success()
        .stdout(contains("--include-skills"));

    bin()
        .args(["list-sources", "--help"])
        .assert()
        .success()
        .stdout(contains("--include-skills"));

    bin()
        .args(["filter-sources", "--help"])
        .assert()
        .success()
        .stdout(contains("--include-skills"));
}
