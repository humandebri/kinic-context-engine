// Where: tools/pocket_ic_tests/tests/common/fixtures.rs
// What: Shared fixture data for PocketIC catalog and fake memory tests.
// Why: Keep source metadata and payload fixtures consistent across engine and CLI E2E tests.
#![allow(dead_code)]

use kinic_context_core::types::{IndexedDocument, SourceUpsert};

pub fn source(source_id: &str, canister_ids: Vec<String>) -> SourceUpsert {
    match source_id {
        "/vercel/next.js" => SourceUpsert {
            source_id: source_id.to_string(),
            title: "Next.js Docs".to_string(),
            aliases: vec![
                "next".to_string(),
                "next.js".to_string(),
                "middleware".to_string(),
                "next migration".to_string(),
                "nextjs migration".to_string(),
            ],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids,
            supported_versions: vec!["14".to_string(), "15".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec![
                "https://nextjs.org/docs".to_string(),
                "https://nextjs.org/docs/app/building-your-application/upgrading".to_string(),
            ],
        },
        "/supabase/docs" => SourceUpsert {
            source_id: source_id.to_string(),
            title: "Supabase Docs".to_string(),
            aliases: vec!["supabase".to_string(), "auth".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids,
            supported_versions: vec!["2026".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://supabase.com/docs".to_string()],
        },
        _ => SourceUpsert {
            source_id: source_id.to_string(),
            title: "React Docs".to_string(),
            aliases: vec!["react".to_string(), "hooks".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids,
            supported_versions: vec!["19".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://react.dev".to_string()],
        },
    }
}

pub fn nextjs_results() -> Vec<IndexedDocument> {
    vec![indexed_document(
        "Next.js Middleware",
        "Use middleware to inspect cookies and redirect unauthenticated users.",
        "https://nextjs.org/docs/app/building-your-application/routing/middleware",
        Some("15"),
        "Full Next.js middleware docs chunk",
        Some("middleware"),
        &["next.js", "auth", "cookies", "redirect"],
        vec![0.9, 0.1, 0.0, 0.0],
    )]
}

pub fn supabase_results() -> Vec<IndexedDocument> {
    vec![indexed_document(
        "Supabase Next.js Auth",
        "Refresh auth state on the server before rendering protected routes.",
        "https://supabase.com/docs/guides/auth/server-side/nextjs",
        Some("2026"),
        "Full Supabase auth docs chunk",
        Some("auth"),
        &["supabase", "auth", "next.js", "server"],
        vec![0.7, 0.3, 0.0, 0.0],
    )]
}

pub fn missing_canister_id() -> String {
    "2vxsx-fae".to_string()
}

pub fn nextjs_migration_results() -> Vec<IndexedDocument> {
    vec![indexed_document(
        "Next.js Upgrade Guide",
        "Check official migration guides and validate breaking changes before upgrading.",
        "https://nextjs.org/docs/app/building-your-application/upgrading",
        None,
        "Prefer official migration notes, verify middleware behavior, and review auth integration changes.",
        Some("migration"),
        &["next.js", "migration", "upgrade"],
        vec![0.8, 0.2, 0.0, 0.0],
    )]
}

pub fn launch_agent_results() -> Vec<IndexedDocument> {
    vec![indexed_document(
        "Tailscale LaunchAgent",
        "Use the macOS LaunchAgent plist to keep Tailscale running after login.",
        "https://tailscale.com/kb/launchagent",
        Some("1"),
        "LaunchAgent setup details for Tailscale on macOS.",
        Some("launchd"),
        &["tailscale", "macos", "launchagent"],
        vec![0.1, 0.9, 0.0, 0.0],
    )]
}

fn indexed_document(
    title: &str,
    snippet: &str,
    citation: &str,
    version: Option<&str>,
    content: &str,
    section: Option<&str>,
    tags: &[&str],
    embedding: Vec<f32>,
) -> IndexedDocument {
    IndexedDocument {
        title: title.to_string(),
        snippet: snippet.to_string(),
        citation: citation.to_string(),
        version: version.map(ToString::to_string),
        content: content.to_string(),
        section: section.map(ToString::to_string),
        tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
        embedding,
    }
}
