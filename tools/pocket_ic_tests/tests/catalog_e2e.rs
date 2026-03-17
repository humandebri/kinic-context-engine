// Where: tools/pocket_ic_tests/tests/catalog_e2e.rs
// What: PocketIC deploy and engine-level end-to-end tests for the catalog and fake memory canisters.
// Why: Verify the read path end-to-end without relying on external embedding APIs or live ICP canisters.
mod common;

use anyhow::Result;
use kinic_context_cli::{
    catalog::IcSourceCatalog,
    engine::ContextEngine,
    provider::IcSourceQueryProvider,
};
use kinic_context_core::{client::QueryClient, types::FilterSourcesArgs};

use common::{
    fixtures::{missing_canister_id, nextjs_results, skill_results, source, supabase_results},
    pocketic::{
        TestCanisters, ensure_pocket_ic_server, filter_sources, get_source,
        install_catalog_canister, install_fake_memory_instance, pocket_ic, replace_catalog,
        resolve_sources,
    },
};

#[test]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn catalog_canister_deploys_and_resolves_fixture_sources() -> Result<()> {
    ensure_pocket_ic_server()?;

    let mut pic = pocket_ic();
    let test_canisters = TestCanisters::new();
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

    let nextjs = get_source(&pic, catalog_id, "/vercel/next.js")?
        .expect("next.js source should exist after replace_catalog");
    assert_eq!(nextjs.canister_ids[0], next_memory.to_text());

    let resolved = resolve_sources(&pic, catalog_id, "next middleware", 3)?;
    assert_eq!(resolved[0].source_id, "/vercel/next.js");

    let filtered = filter_sources(
        &pic,
        catalog_id,
        FilterSourcesArgs {
            domain: Some("code_docs".to_string()),
            trust: Some("official".to_string()),
            version: Some("15".to_string()),
            limit: Some(3),
        },
    )?;
    assert_eq!(filtered[0].source_id, "/vercel/next.js");

    let skill_filtered = filter_sources(
        &pic,
        catalog_id,
        FilterSourcesArgs {
            domain: Some("skill_knowledge".to_string()),
            trust: None,
            version: None,
            limit: Some(3),
        },
    )?;
    assert_eq!(skill_filtered[0].source_id, "/skills/nextjs/migration");
    Ok(())
}

#[test]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn engine_query_and_pack_work_against_pocket_ic() -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        ensure_pocket_ic_server()?;

        let mut pic = pocket_ic();
        let test_canisters = TestCanisters::new();
        let gateway = pic.make_live(None);
        let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
        let next_memory =
            install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
        let supabase_memory =
            install_fake_memory_instance(&mut pic, test_canisters, supabase_results())?;
        let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;
        let skill_memory =
            install_fake_memory_instance(&mut pic, test_canisters, skill_results())?;

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

        let client = QueryClient::new(gateway.as_ref(), false).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider = IcSourceQueryProvider::with_fixed_embedding(client, vec![0.0_f32; 4]);
        let engine = ContextEngine::new(catalog, provider);

        let query = engine
            .query("/vercel/next.js", "middleware cookies", Some("15"), 5)
            .await?;
        let query_json = serde_json::to_value(query)?;
        assert_eq!(query_json["snippets"][0]["title"], "Next.js Middleware");
        assert_eq!(
            query_json["snippets"][0]["citation"],
            "https://nextjs.org/docs/app/building-your-application/routing/middleware"
        );

        let pack = engine
            .pack("protect route in next.js with supabase auth", 3, 3000, false)
            .await?;
        let pack_json = serde_json::to_value(pack)?;
        assert!(
            pack_json["resolved_sources"]
                .as_array()
                .expect("resolved_sources should be an array")
                .len()
                >= 2
        );
        assert!(
            pack_json["evidence"]
                .as_array()
                .expect("evidence should be an array")
                .len()
                >= 2
        );

        let pack_with_empty = engine.pack("next react hooks", 3, 3000, false).await?;
        let empty_json = serde_json::to_value(pack_with_empty)?;
        assert!(
            empty_json["warnings"]
                .as_array()
                .expect("warnings should be an array")
                .iter()
                .any(|warning| warning["kind"] == "empty_source")
        );

        let skill_pack = engine.pack("next migration", 5, 3000, true).await?;
        let skill_pack_json = serde_json::to_value(skill_pack)?;
        assert!(
            skill_pack_json["evidence"]
                .as_array()
                .expect("evidence should be an array")
                .iter()
                .any(|item| item["source_id"] == "/skills/nextjs/migration")
        );
        Ok(())
    })
}

#[test]
#[ignore = "requires POCKET_IC_BIN=/path/to/pocket-ic-server"]
fn engine_query_and_pack_error_contracts_stay_stable() -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        ensure_pocket_ic_server()?;

        let mut pic = pocket_ic();
        let test_canisters = TestCanisters::new();
        let gateway = pic.make_live(None);
        let catalog_id = install_catalog_canister(&mut pic, test_canisters)?;
        let next_memory =
            install_fake_memory_instance(&mut pic, test_canisters, nextjs_results())?;
        let react_memory = install_fake_memory_instance(&mut pic, test_canisters, Vec::new())?;
        let skill_memory =
            install_fake_memory_instance(&mut pic, test_canisters, skill_results())?;

        replace_catalog(
            &pic,
            test_canisters,
            catalog_id,
            vec![
                source("/vercel/next.js", vec![next_memory.to_text(), missing_canister_id()]),
                source("/supabase/docs", Vec::new()),
                source("/react/docs", vec![react_memory.to_text()]),
                source("/skills/nextjs/migration", vec![skill_memory.to_text()]),
            ],
        )?;

        let client = QueryClient::new(gateway.as_ref(), false).await?;
        let catalog = IcSourceCatalog::new(client.clone(), catalog_id.to_text());
        let provider = IcSourceQueryProvider::with_fixed_embedding(client, vec![0.0_f32; 4]);
        let engine = ContextEngine::new(catalog, provider);

        let missing_source = engine.query("/unknown/source", "middleware", None, 5).await;
        assert!(missing_source.is_err());

        let empty_canisters = engine.query("/supabase/docs", "auth", None, 5).await;
        assert!(empty_canisters.is_err());

        let partial = engine
            .query("/vercel/next.js", "middleware cookies", None, 5)
            .await?;
        let partial_json = serde_json::to_value(partial)?;
        assert_eq!(partial_json["snippets"][0]["title"], "Next.js Middleware");

        let version_filtered = engine
            .query("/vercel/next.js", "middleware cookies", Some("999"), 5)
            .await?;
        let version_json = serde_json::to_value(version_filtered)?;
        assert_eq!(
            version_json["snippets"]
                .as_array()
                .expect("snippets should be an array")
                .len(),
            0
        );

        let pack = engine.pack("react hooks", 3, 3000, false).await?;
        let pack_json = serde_json::to_value(pack)?;
        assert!(
            pack_json["warnings"]
                .as_array()
                .expect("warnings should be an array")
                .iter()
                .any(|warning| warning["kind"] == "empty_source")
        );
        Ok(())
    })
}
