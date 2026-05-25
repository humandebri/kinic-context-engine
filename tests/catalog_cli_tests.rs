// Where: tests/catalog_cli_tests.rs
// What: Wiki CLI-backed catalog adapter contract tests.
// Why: Keep list/filter behavior tied to existing kinic-vfs-cli JSON shapes.
use std::fs;

use kinic_context_cli::catalog::{SourceCatalog, WikiCliSourceCatalog};
use kinic_context_core::types::FilterSourcesArgs;
use tempfile::tempdir;

#[tokio::test]
async fn wiki_cli_catalog_lists_and_filters_sources_from_node_metadata() {
    let temp = tempdir().expect("temp dir");
    let script = temp.path().join("wiki_cli_stub.py");
    fs::write(
        &script,
        r#"
import json
import sys

args = sys.argv[1:]
if "--marker" not in args:
    raise SystemExit(3)
if "search-remote" in args:
    print(json.dumps([
        {"path": "/Wiki/sources/bad/15/broken-s0000-c0000.md", "score": 9.0, "match_reasons": ["broken"]},
        {"path": "/Wiki/sources/foo_bar__docs/1/intro-s0000-c0000.md", "score": 2.0, "match_reasons": ["content_fts"]},
        {"path": "/Wiki/sources/foo_bar__docs/1/other-s0000-c0001.md", "score": 1.0, "match_reasons": ["duplicate"]}
    ]))
elif "list-nodes" in args:
    print(json.dumps([
        {"path": "/Wiki/sources/vercel__next_js/index.md", "kind": "File", "updated_at": 1, "etag": "a", "has_children": True},
        {"path": "/Wiki/sources/vercel__next_js/15/middleware-s0000-c0000.md", "kind": "File", "updated_at": 1, "etag": "b", "has_children": False},
        {"path": "/Wiki/sources/bad/index.md", "kind": "File", "updated_at": 1, "etag": "c", "has_children": False},
        {"path": "/Wiki/sources/supabase__docs/index.md", "kind": "File", "updated_at": 1, "etag": "d", "has_children": False}
    ]))
elif "read-node" in args:
    path = args[args.index("--path") + 1]
    if path.startswith("/Wiki/sources/bad/"):
        metadata = "{bad"
    elif path == "/Wiki/sources/foo_bar__docs/1/intro-s0000-c0000.md":
        metadata = json.dumps({
            "source_id": "/foo_bar/docs",
            "title": "Foo Bar Docs",
            "trust": "official",
            "domain": "code_docs",
            "version": "1",
            "citation": "https://example.com/foo_bar",
            "chunk_id": "chunk-a",
            "retrieved_at": "2026-03-18T00:00:00Z"
        })
    elif path == "/Wiki/sources/foo_bar__docs/1/other-s0000-c0001.md":
        metadata = json.dumps({
            "source_id": "/foo_bar/docs",
            "title": "Foo Bar Docs",
            "trust": "official",
            "domain": "code_docs",
            "version": "1",
            "citation": "https://example.com/foo_bar/other",
            "chunk_id": "chunk-b",
            "retrieved_at": "2026-03-18T00:00:00Z"
        })
    elif path == "/Wiki/sources/supabase__docs/index.md":
        metadata = json.dumps({
            "source_id": "/supabase/docs",
            "title": "Supabase Docs",
            "trust": "community",
            "domain": "code_docs",
            "version": "2",
            "citation": "https://supabase.com/docs",
            "retrieved_at": "2026-03-18T00:00:00Z"
        })
    else:
        metadata = json.dumps({
            "source_id": "/vercel/next.js",
            "title": "Next.js Docs",
            "aliases": ["next", "middleware"],
            "trust": "official",
            "domain": "code_docs",
            "supported_versions": ["14", "15"],
            "citations": ["https://nextjs.org/docs"],
            "retrieved_at": "2026-03-18T00:00:00Z"
        })
    print(json.dumps({"path": path, "metadata_json": metadata}))
else:
    raise SystemExit(2)
"#,
    )
    .expect("write stub");

    let catalog = WikiCliSourceCatalog::new(
        format!("python3 {} --marker", script.display()),
        "db_test".to_string(),
    );
    let sources = catalog.list_sources().await.expect("list sources");
    assert_eq!(
        sources
            .iter()
            .map(|source| source.source_id.as_str())
            .collect::<Vec<_>>(),
        vec!["/supabase/docs", "/vercel/next.js"]
    );

    let filtered = catalog
        .filter_sources(FilterSourcesArgs {
            domain: Some("code_docs".to_string()),
            trust: Some("official".to_string()),
            version: Some("15".to_string()),
            limit: Some(1),
        })
        .await
        .expect("filter sources");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].source_id, "/vercel/next.js");

    let resolved = catalog
        .resolve_sources("foo bar", 2)
        .await
        .expect("resolve sources");
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].source_id, "/foo_bar/docs");
    assert_eq!(resolved[0].title, "Foo Bar Docs");
}
