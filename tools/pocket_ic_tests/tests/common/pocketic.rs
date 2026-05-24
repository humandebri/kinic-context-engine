// Where: tools/pocket_ic_tests/tests/common/pocketic.rs
// What: Shared PocketIC deploy/build helpers for catalog and fake memory canisters.
// Why: Keep the E2E tests focused on behavior instead of wasm build and canister setup boilerplate.
#![allow(dead_code)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use anyhow::{Context, Result, anyhow};
use candid::{Decode, Encode, Principal};
use kinic_context_core::types::{
    FilterSourcesArgs, HybridQueryRequest, HybridSearchResult, IndexedDocument,
    ResolvedCatalogSource, SectionIndexRecord, SourceMetadata, SourceUpsert,
};
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
    let server_binary =
        ensure_pocket_ic_server().expect("PocketIC binary should exist when ignored tests run");
    PocketIcBuilder::new()
        .with_server_binary(server_binary)
        .with_application_subnet()
        .build()
}

pub fn install_catalog_canister(
    pic: &mut PocketIc,
    test_canisters: TestCanisters,
) -> Result<Principal> {
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
    documents: Vec<IndexedDocument>,
) -> Result<Principal> {
    let sections = derive_sections(&documents)?;
    let canister_id = pic.create_canister_with_settings(Some(test_canisters.controller), None);
    pic.add_cycles(canister_id, CYCLES);
    pic.install_canister(
        canister_id,
        fake_memory_wasm()?,
        Encode!(&documents)?,
        Some(test_canisters.controller),
    );
    for section in sections {
        pic.update_call(
            canister_id,
            test_canisters.controller,
            "insert_section",
            Encode!(&section)?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    }
    Ok(canister_id)
}

pub fn upgrade_catalog_canister(
    pic: &PocketIc,
    test_canisters: TestCanisters,
    catalog_id: Principal,
) -> Result<()> {
    pic.upgrade_canister(
        catalog_id,
        catalog_wasm()?,
        Encode!()?,
        Some(test_canisters.controller),
    )
    .map_err(|error| anyhow!(error.to_string()))
}

pub fn upgrade_fake_memory_instance(
    pic: &PocketIc,
    test_canisters: TestCanisters,
    canister_id: Principal,
) -> Result<()> {
    pic.upgrade_canister(
        canister_id,
        fake_memory_wasm()?,
        Encode!()?,
        Some(test_canisters.controller),
    )
    .map_err(|error| anyhow!(error.to_string()))
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

pub fn search_memory(
    pic: &PocketIc,
    canister_id: Principal,
    query_embedding: Vec<f32>,
) -> Result<Vec<(f32, String)>> {
    let response = pic
        .query_call(
            canister_id,
            Principal::anonymous(),
            "search",
            Encode!(&query_embedding)?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(Decode!(&response, Vec<(f32, String)>)?)
}

pub fn hybrid_query_memory(
    pic: &PocketIc,
    canister_id: Principal,
    request: HybridQueryRequest,
) -> Result<Vec<HybridSearchResult>> {
    let response = pic
        .query_call(
            canister_id,
            Principal::anonymous(),
            "hybrid_query",
            Encode!(&request)?,
        )
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(Decode!(&response, Vec<HybridSearchResult>)?)
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
        .args([
            "build",
            "-p",
            "catalog_canister",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
        ])
        .current_dir(&root))?;
    let output = root.join("target/wasm32-unknown-unknown/release/catalog_canister.wasm");
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
    let output = root.join("target/wasm32-unknown-unknown/release/fake_memory_instance.wasm");
    std::fs::read(output).context("failed to read fake memory instance wasm")
}

fn workspace_root() -> Result<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize()
        .context("failed to resolve workspace root")
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

fn derive_sections(documents: &[IndexedDocument]) -> Result<Vec<SectionIndexRecord>> {
    let mut grouped: HashMap<(String, Option<String>), SectionAccumulator> = HashMap::new();
    for document in documents {
        let Some(section_id) = document
            .section
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let version = document.version.clone();
        let key = (section_id.to_string(), version.clone());
        let title = if document.title.trim().is_empty() {
            section_id
        } else {
            document.title.as_str()
        };
        let snippet = document.snippet.trim().to_string();
        let entry = grouped.entry(key).or_insert_with(|| SectionAccumulator {
            title: title.to_string(),
            snippets: Vec::new(),
            embedding_sum: vec![0.0; document.embedding.len()],
            embedding_count: 0,
        });
        if entry.title == section_id {
            entry.title = title.to_string();
        }
        if !snippet.is_empty() && !entry.snippets.iter().any(|item| item == &snippet) {
            entry.snippets.push(snippet);
        }
        for (slot, value) in entry.embedding_sum.iter_mut().zip(&document.embedding) {
            *slot += *value;
        }
        entry.embedding_count += 1;
    }

    let mut sections = grouped
        .into_iter()
        .filter_map(|((section_id, version), item)| {
            if item.embedding_count == 0 {
                return None;
            }
            let embedding = item
                .embedding_sum
                .into_iter()
                .map(|value| value / item.embedding_count as f32)
                .collect::<Vec<_>>();
            Some(SectionIndexRecord {
                section_id,
                title: item.title,
                summary: item
                    .snippets
                    .into_iter()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                version,
                embedding,
            })
        })
        .collect::<Vec<_>>();
    sections.sort_by(|left, right| {
        left.section_id
            .cmp(&right.section_id)
            .then_with(|| left.version.cmp(&right.version))
    });
    Ok(sections)
}

struct SectionAccumulator {
    title: String,
    snippets: Vec<String>,
    embedding_sum: Vec<f32>,
    embedding_count: usize,
}
