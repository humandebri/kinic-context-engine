// Where: tools/pocket_ic_tests/tests/common/fixtures.rs
// What: Shared fixture data for PocketIC catalog and fake memory tests.
// Why: Keep source metadata and payload fixtures consistent across engine and CLI E2E tests.
#![allow(dead_code)]

use kinic_context_core::types::SourceUpsert;

pub fn source(source_id: &str, canister_ids: Vec<String>) -> SourceUpsert {
    match source_id {
        "/vercel/next.js" => SourceUpsert {
            source_id: source_id.to_string(),
            title: "Next.js Docs".to_string(),
            aliases: vec!["next".to_string(), "next.js".to_string(), "middleware".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids,
            supported_versions: vec!["14".to_string(), "15".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://nextjs.org/docs".to_string()],
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
        "/skills/nextjs/migration" => SourceUpsert {
            source_id: source_id.to_string(),
            title: "Next.js Migration Skill".to_string(),
            aliases: vec![
                "next migration".to_string(),
                "nextjs migration".to_string(),
                "upgrade".to_string(),
            ],
            trust: "curated".to_string(),
            domain: "skill_knowledge".to_string(),
            skill_kind: Some("migration".to_string()),
            targets: vec!["nextjs".to_string()],
            capabilities: vec![
                "auth".to_string(),
                "middleware".to_string(),
                "routing".to_string(),
            ],
            canister_ids,
            supported_versions: Vec::new(),
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec![
                "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md"
                    .to_string(),
            ],
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

pub fn nextjs_results() -> Vec<(f32, String)> {
    vec![(
        0.98,
        serde_json::json!({
            "title": "Next.js Middleware",
            "snippet": "Use middleware to inspect cookies and redirect unauthenticated users.",
            "citation": "https://nextjs.org/docs/app/building-your-application/routing/middleware",
            "version": "15",
            "content": "Full Next.js middleware docs chunk"
        })
        .to_string(),
    )]
}

pub fn supabase_results() -> Vec<(f32, String)> {
    vec![(
        0.88,
        serde_json::json!({
            "title": "Supabase Next.js Auth",
            "snippet": "Refresh auth state on the server before rendering protected routes.",
            "citation": "https://supabase.com/docs/guides/auth/server-side/nextjs",
            "version": "2026",
            "content": "Full Supabase auth docs chunk"
        })
        .to_string(),
    )]
}

pub fn missing_canister_id() -> String {
    "2vxsx-fae".to_string()
}

pub fn skill_results() -> Vec<(f32, String)> {
    vec![(
        0.91,
        serde_json::json!({
            "source_id": "/skills/nextjs/migration",
            "title": "Next.js Migration Skill",
            "snippet": "Check official migration guides and validate breaking changes before upgrading.",
            "citation": "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md",
            "content": "Prefer official migration notes, verify middleware behavior, and review auth integration changes.",
            "section": "migration",
            "tags": ["next.js", "migration", "upgrade"],
            "retrieved_at": "2026-03-17T00:00:00Z"
        })
        .to_string(),
    )]
}
