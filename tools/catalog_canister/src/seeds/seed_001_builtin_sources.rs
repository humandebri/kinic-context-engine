use ic_sql_migrate::MigrateResult;
use kinic_context_core::types::SourceUpsert;

use crate::sqlite_runtime::{Connection, params};

pub fn seed(conn: &Connection) -> MigrateResult<()> {
    for source in builtin_sources() {
        let source_id = source.source_id.clone();
        conn.execute(
            "INSERT OR IGNORE INTO sources (source_id, title, domain, trust, retrieved_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &source.source_id,
                &source.title,
                &source.domain,
                &source.trust,
                &source.retrieved_at
            ],
        )?;

        for alias in source.aliases {
            conn.execute(
                "INSERT OR IGNORE INTO source_aliases (source_id, alias) VALUES (?1, ?2)",
                params![&source_id, alias],
            )?;
        }

        for version in source.supported_versions {
            conn.execute(
                "INSERT OR IGNORE INTO source_versions (source_id, version) VALUES (?1, ?2)",
                params![&source_id, version],
            )?;
        }

        for citation in source.citations {
            conn.execute(
                "INSERT OR IGNORE INTO source_citations (source_id, citation) VALUES (?1, ?2)",
                params![&source_id, citation],
            )?;
        }
    }
    Ok(())
}

fn builtin_sources() -> Vec<SourceUpsert> {
    vec![
        SourceUpsert {
            source_id: "/vercel/next.js".to_string(),
            title: "Next.js Docs".to_string(),
            aliases: vec![
                "next".to_string(),
                "nextjs".to_string(),
                "next.js".to_string(),
                "middleware".to_string(),
            ],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids: Vec::new(),
            supported_versions: vec!["14".to_string(), "15".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://nextjs.org/docs".to_string()],
        },
        SourceUpsert {
            source_id: "/supabase/docs".to_string(),
            title: "Supabase Docs".to_string(),
            aliases: vec!["supabase".to_string(), "auth".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids: Vec::new(),
            supported_versions: vec!["2026".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://supabase.com/docs".to_string()],
        },
        SourceUpsert {
            source_id: "/react/docs".to_string(),
            title: "React Docs".to_string(),
            aliases: vec!["react".to_string(), "hooks".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids: Vec::new(),
            supported_versions: vec!["19".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://react.dev".to_string()],
        },
        SourceUpsert {
            source_id: "/skills/nextjs/migration".to_string(),
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
            canister_ids: Vec::new(),
            supported_versions: Vec::new(),
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec![
                "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md"
                    .to_string(),
            ],
        },
    ]
}
