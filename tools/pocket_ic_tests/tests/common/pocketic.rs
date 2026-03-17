// Where: tools/pocket_ic_tests/tests/common/pocketic.rs
// What: Shared PocketIC deploy/build helpers for catalog and fake memory canisters.
// Why: Keep the E2E tests focused on behavior instead of wasm build and canister setup boilerplate.
#![allow(dead_code)]

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use anyhow::{Context, Result, anyhow};
use candid::{Decode, Encode, Principal};
use kinic_context_core::types::{FilterSourcesArgs, ResolvedCatalogSource, SourceMetadata, SourceUpsert};
use pocket_ic::{PocketIc, PocketIcBuilder};

const CYCLES: u128 = 2_000_000_000_000;
const CONTROLLER_BYTES: [u8; 29] = [7; 29];

#[derive(Clone, Copy)]
pub struct TestCanisters {
    pub controller: Principal,
}

impl TestCanisters {
    pub fn new() -> Self {
        Self {
            controller: Principal::self_authenticating(&CONTROLLER_BYTES),
        }
    }
}

pub fn ensure_pocket_ic_server() -> Result<PathBuf> {
    let path = std::env::var("POCKET_IC_BIN")
        .context("POCKET_IC_BIN must point to pocket-ic-server for ignored PocketIC tests")?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(anyhow!(
            "POCKET_IC_BIN must point to pocket-ic-server for ignored PocketIC tests"
        ));
    }
    Ok(PathBuf::from(trimmed))
}

pub fn pocket_ic() -> PocketIc {
    let server_binary = ensure_pocket_ic_server()
        .expect("PocketIC binary should exist when ignored tests run");
    PocketIcBuilder::new()
        .with_server_binary(server_binary)
        .build()
}

pub fn install_catalog_canister(pic: &mut PocketIc, test_canisters: TestCanisters) -> Result<Principal> {
    let canister_id = pic.create_canister_with_settings(Some(test_canisters.controller), None);
    pic.add_cycles(canister_id, CYCLES);
    pic.install_canister(
        canister_id,
        catalog_wasm()?,
        Encode!()?,
        Some(test_canisters.controller),
    );
    Ok(canister_id)
}

pub fn install_fake_memory_instance(
    pic: &mut PocketIc,
    test_canisters: TestCanisters,
    results: Vec<(f32, String)>,
) -> Result<Principal> {
    let canister_id = pic.create_canister_with_settings(Some(test_canisters.controller), None);
    pic.add_cycles(canister_id, CYCLES);
    pic.install_canister(
        canister_id,
        fake_memory_wasm()?,
        Encode!(&results)?,
        Some(test_canisters.controller),
    );
    Ok(canister_id)
}

pub fn replace_catalog(
    pic: &PocketIc,
    test_canisters: TestCanisters,
    catalog_id: Principal,
    sources: Vec<SourceUpsert>,
) -> Result<()> {
    let payload = Encode!(&sources)?;
    pic.update_call(
        catalog_id,
        test_canisters.controller,
        "admin_replace_catalog",
        payload,
    )
    .map(|_| ())
    .map_err(|error| anyhow!(error.to_string()))
}

pub fn get_source(
    pic: &PocketIc,
    catalog_id: Principal,
    source_id: &str,
) -> Result<Option<SourceMetadata>> {
    let response = pic
        .query_call(
            catalog_id,
            Principal::anonymous(),
            "get_source",
            Encode!(&source_id.to_string())?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(Decode!(&response, Option<SourceMetadata>)?)
}

pub fn resolve_sources(
    pic: &PocketIc,
    catalog_id: Principal,
    query: &str,
    limit: u32,
) -> Result<Vec<ResolvedCatalogSource>> {
    let response = pic
        .query_call(
            catalog_id,
            Principal::anonymous(),
            "resolve_sources",
            Encode!(&query.to_string(), &limit)?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(Decode!(&response, Vec<ResolvedCatalogSource>)?)
}

pub fn filter_sources(
    pic: &PocketIc,
    catalog_id: Principal,
    args: FilterSourcesArgs,
) -> Result<Vec<SourceMetadata>> {
    let response = pic
        .query_call(
            catalog_id,
            Principal::anonymous(),
            "filter_sources",
            Encode!(&args)?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(Decode!(&response, Vec<SourceMetadata>)?)
}

fn catalog_wasm() -> Result<Vec<u8>> {
    static WASM: OnceLock<Vec<u8>> = OnceLock::new();
    Ok(WASM
        .get_or_init(|| build_catalog_wasm().expect("catalog wasm should build"))
        .clone())
}

fn fake_memory_wasm() -> Result<Vec<u8>> {
    static WASM: OnceLock<Vec<u8>> = OnceLock::new();
    Ok(WASM
        .get_or_init(|| build_fake_memory_wasm().expect("fake memory wasm should build"))
        .clone())
}

fn build_catalog_wasm() -> Result<Vec<u8>> {
    let root = workspace_root()?;
    run(Command::new("cargo")
        .args(["build", "-p", "catalog_canister", "--target", "wasm32-wasip1", "--release"])
        .current_dir(&root))?;
    let input = root.join("target/wasm32-wasip1/release/catalog_canister.wasm");
    let output = root.join("target/wasm32-wasip1/release/catalog_canister-wasi2ic.wasm");
    run(Command::new("wasi2ic").arg(&input).arg(&output).current_dir(&root))?;
    std::fs::read(output).context("failed to read catalog wasm")
}

fn build_fake_memory_wasm() -> Result<Vec<u8>> {
    let root = workspace_root()?;
    run(Command::new("cargo")
        .args([
            "build",
            "-p",
            "fake_memory_instance",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .current_dir(&root))?;
    std::fs::read(root.join("target/wasm32-unknown-unknown/release/fake_memory_instance.wasm"))
        .context("failed to read fake memory instance wasm")
}

fn workspace_root() -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().context("failed to resolve workspace root")
}

fn run(command: &mut Command) -> Result<()> {
    let output = command.output().context("failed to spawn build command")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "command failed: {}\nstdout:\n{}\nstderr:\n{}",
            format_command(command),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        ))
    }
}

fn format_command(command: &Command) -> String {
    let program = command.get_program().to_string_lossy().to_string();
    let args = command
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("{program} {args}")
}
