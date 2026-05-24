use kinic_context_core::types::SourceUpsert;

pub fn builtin_sources() -> Vec<SourceUpsert> {
    vec![
        SourceUpsert {
            source_id: "/vercel/next.js".to_string(),
            title: "Next.js Docs".to_string(),
            aliases: vec![
                "next".to_string(),
                "nextjs".to_string(),
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
            canister_ids: Vec::new(),
            supported_versions: vec!["14".to_string(), "15".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec![
                "https://nextjs.org/docs".to_string(),
                "https://nextjs.org/docs/app/building-your-application/upgrading".to_string(),
            ],
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
    ]
}
