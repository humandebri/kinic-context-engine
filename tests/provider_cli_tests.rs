// Where: tests/provider_cli_tests.rs
// What: Wiki CLI-backed source query provider contract tests.
// Why: Keep snippet retrieval scoped to docs chunks and canonical metadata citations.
use std::fs;

use kinic_context_cli::{
    model::SourceMetadata,
    provider::{SourceQueryProvider, WikiCliSourceQueryProvider},
};
use tempfile::tempdir;

#[tokio::test]
async fn wiki_cli_provider_returns_only_docs_chunks_with_metadata_citation() {
    let temp = tempdir().expect("temp dir");
    let script = temp.path().join("wiki_cli_stub.py");
    fs::write(
        &script,
        r#"
import json
import sys

args = sys.argv[1:]
if "search-remote" in args:
    print(json.dumps([
        {"path": "/Wiki/sources/vercel__next_js/index.md", "score": 5.0, "snippet": "index"},
        {"path": "/Wiki/sources/vercel__next_js/15/middleware-s0000-c0000.md", "score": 3.0, "snippet": "Use middleware."},
        {"path": "/Wiki/sources/vercel__next_js/15/bad-s0000-c0001.md", "score": 2.0, "snippet": "Bad metadata."}
    ]))
elif "read-node" in args:
    path = args[args.index("--path") + 1]
    if path.endswith("middleware-s0000-c0000.md"):
        metadata = json.dumps({
            "source_id": "/vercel/next.js",
            "title": "Next.js Middleware",
            "trust": "official",
            "domain": "code_docs",
            "version": "15",
            "citation": "https://nextjs.org/docs/middleware",
            "chunk_id": "chunk-a",
            "retrieved_at": "2026-03-18T00:00:00Z"
        })
    elif path.endswith("bad-s0000-c0001.md"):
        metadata = json.dumps({
            "source_id": "/vercel/next.js",
            "title": "Bad Chunk",
            "citation": "https://nextjs.org/docs/bad"
        })
    else:
        metadata = json.dumps({
            "source_id": "/vercel/next.js",
            "title": "Next.js Docs",
            "trust": "official",
            "domain": "code_docs"
        })
    print(json.dumps({"path": path, "metadata_json": metadata}))
else:
    raise SystemExit(2)
"#,
    )
    .expect("write stub");

    let provider =
        WikiCliSourceQueryProvider::new(format!("python3 {}", script.display()), "db_test".into());
    let snippets = provider
        .query(
            SourceMetadata {
                source_id: "/vercel/next.js".into(),
                title: "Next.js Docs".into(),
                aliases: vec![],
                trust: "official".into(),
                domain: "code_docs".into(),
                supported_versions: vec!["15".into()],
                retrieved_at: "2026-03-18T00:00:00Z".into(),
                citations: vec![],
            },
            "middleware",
            Some("15"),
            3,
        )
        .await
        .expect("query source");

    assert_eq!(snippets.len(), 1);
    assert_eq!(snippets[0].title, "Next.js Middleware");
    assert_eq!(snippets[0].citation, "https://nextjs.org/docs/middleware");
    assert_eq!(snippets[0].version.as_deref(), Some("15"));
}
