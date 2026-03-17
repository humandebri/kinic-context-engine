// Where: tools/catalog_canister/src/lib.rs
// What: Catalog-only canister that stores source metadata and backing memory instance IDs.
// Why: Replace CLI-local source maps with a shared read API while keeping RAG retrieval in memory instances.
use std::cmp::Ordering;

use ic_cdk::{
    api::{is_controller, msg_caller},
    init, post_upgrade, pre_upgrade, query, trap, update,
};
use kinic_context_core::types::{
    FilterSourcesArgs, ResolvedCatalogSource, SourceMetadata, SourceUpsert,
};
use rusqlite::Transaction;

mod seeds;
mod sqlite_runtime;

use sqlite_runtime::{Connection, OptionalExtension, close_connection, params, with_connection};

static MIGRATIONS: &[ic_sql_migrate::Migration] = ic_sql_migrate::include_migrations!();

#[init]
fn init() {
    run_migrations_and_seeds();
}

#[pre_upgrade]
fn pre_upgrade() {
    close_connection();
}

#[post_upgrade]
fn post_upgrade() {
    run_migrations_and_seeds();
}

#[query]
fn list_sources() -> Vec<SourceMetadata> {
    with_connection(|conn| load_all_sources(&conn)).unwrap_or_else(|error| trap(&error))
}

#[query]
fn get_source(source_id: String) -> Option<SourceMetadata> {
    with_connection(|conn| load_source(&conn, &source_id)).unwrap_or_else(|error| trap(&error))
}

#[query]
fn resolve_sources(query: String, limit: u32) -> Vec<ResolvedCatalogSource> {
    let normalized = normalize(&query);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut results: Vec<ResolvedCatalogSource> = list_sources()
        .into_iter()
        .filter_map(|source| {
            let mut score = 0.0_f32;
            let mut reasons = Vec::new();

            for alias in &source.aliases {
                let alias_normalized = normalize(alias);
                if tokens
                    .iter()
                    .all(|token| alias_normalized.contains(token) || *token == alias_normalized)
                {
                    score += 1.0;
                    reasons.push(format!("matched alias `{alias}`"));
                } else if normalized.contains(&alias_normalized) {
                    score += 0.5;
                    reasons.push(format!("partially matched alias `{alias}`"));
                }
            }

            if normalized.contains(&normalize(&source.title)) {
                score += 0.7;
                reasons.push("matched title".to_string());
            }

            if normalized.contains(&normalize(&source.source_id)) {
                score += 0.8;
                reasons.push("matched source_id".to_string());
            }

            if score <= 0.0 {
                return None;
            }

            Some(ResolvedCatalogSource {
                source_id: source.source_id,
                title: source.title,
                score,
                reasons,
            })
        })
        .collect();

    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    results.truncate(limit.max(1) as usize);
    results
}

#[query]
fn filter_sources(args: FilterSourcesArgs) -> Vec<SourceMetadata> {
    let mut filtered: Vec<SourceMetadata> = list_sources()
        .into_iter()
        .filter(|source| {
            matches_optional(&args.domain, &source.domain)
                && matches_optional(&args.trust, &source.trust)
                && args.version.as_ref().is_none_or(|version| {
                    source.supported_versions.iter().any(|item| item == version)
                })
        })
        .collect();
    filtered.truncate(args.limit.unwrap_or(u32::MAX) as usize);
    filtered
}

#[update]
fn admin_upsert_source(source: SourceUpsert) {
    ensure_controller();
    with_connection(|mut conn| upsert_source(&mut conn, &source))
        .unwrap_or_else(|error| trap(&error));
}

#[update]
fn admin_replace_catalog(sources: Vec<SourceUpsert>) {
    ensure_controller();
    with_connection(|mut conn| {
        conn.execute("DELETE FROM source_aliases", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM source_targets", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM source_capabilities", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM source_canisters", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM source_versions", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM source_citations", [])
            .map_err(|error| error.to_string())?;
        conn.execute("DELETE FROM sources", [])
            .map_err(|error| error.to_string())?;

        for source in &sources {
            upsert_source(&mut conn, source)?;
        }
        Ok::<(), String>(())
    })
    .unwrap_or_else(|error| trap(&error));
}

fn run_migrations_and_seeds() {
    with_connection(|mut conn| {
        let conn: &mut Connection = &mut conn;
        ic_sql_migrate::sqlite::migrate(conn, MIGRATIONS).expect("catalog migrations must run");
        ic_sql_migrate::sqlite::seed(conn, seeds::SEEDS).expect("catalog seeds must run");
    });
}

fn ensure_controller() {
    let principal = msg_caller();
    if !is_controller(&principal) {
        trap("catalog admin methods are controller-only");
    }
}

fn load_all_sources(conn: &Connection) -> Result<Vec<SourceMetadata>, String> {
    let mut stmt = conn
        .prepare("SELECT source_id FROM sources ORDER BY source_id")
        .map_err(|error| error.to_string())?;
    let source_ids = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    source_ids
        .into_iter()
        .map(|source_id| {
            load_source(conn, &source_id)?
                .ok_or_else(|| format!("source disappeared during scan: {source_id}"))
        })
        .collect()
}

fn load_source(conn: &Connection, source_id: &str) -> Result<Option<SourceMetadata>, String> {
    let row = conn
        .query_row(
            "SELECT source_id, title, trust, domain, skill_kind, retrieved_at FROM sources WHERE source_id = ?1",
            params![source_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;

    row.map(|(source_id, title, trust, domain, skill_kind, retrieved_at)| {
        Ok(SourceMetadata {
            aliases: collect_values(conn, "SELECT alias FROM source_aliases WHERE source_id = ?1 ORDER BY alias", &source_id)?,
            targets: collect_values(conn, "SELECT target FROM source_targets WHERE source_id = ?1 ORDER BY target", &source_id)?,
            capabilities: collect_values(conn, "SELECT capability FROM source_capabilities WHERE source_id = ?1 ORDER BY capability", &source_id)?,
            canister_ids: collect_values(conn, "SELECT canister_id FROM source_canisters WHERE source_id = ?1 ORDER BY canister_id", &source_id)?,
            supported_versions: collect_values(conn, "SELECT version FROM source_versions WHERE source_id = ?1 ORDER BY version", &source_id)?,
            citations: collect_values(conn, "SELECT citation FROM source_citations WHERE source_id = ?1 ORDER BY citation", &source_id)?,
            source_id,
            title,
            trust,
            domain,
            skill_kind,
            retrieved_at,
        })
    })
    .transpose()
}

fn collect_values(conn: &Connection, sql: &str, source_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|error| error.to_string())?;
    stmt.query_map(params![source_id], |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn upsert_source(conn: &mut Connection, source: &SourceUpsert) -> Result<(), String> {
    let tx = conn.transaction().map_err(|error| error.to_string())?;
    tx.execute(
        "INSERT INTO sources (source_id, title, domain, trust, skill_kind, retrieved_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(source_id) DO UPDATE SET
             title = excluded.title,
             domain = excluded.domain,
             trust = excluded.trust,
             skill_kind = excluded.skill_kind,
             retrieved_at = excluded.retrieved_at",
        params![
            &source.source_id,
            &source.title,
            &source.domain,
            &source.trust,
            &source.skill_kind,
            &source.retrieved_at
        ],
    )
    .map_err(|error| error.to_string())?;
    replace_values(&tx, "source_aliases", "alias", &source.source_id, &source.aliases)?;
    replace_values(&tx, "source_targets", "target", &source.source_id, &source.targets)?;
    replace_values(
        &tx,
        "source_capabilities",
        "capability",
        &source.source_id,
        &source.capabilities,
    )?;
    replace_values(
        &tx,
        "source_canisters",
        "canister_id",
        &source.source_id,
        &source.canister_ids,
    )?;
    replace_values(
        &tx,
        "source_versions",
        "version",
        &source.source_id,
        &source.supported_versions,
    )?;
    replace_values(
        &tx,
        "source_citations",
        "citation",
        &source.source_id,
        &source.citations,
    )?;
    tx.commit().map_err(|error| error.to_string())
}

fn replace_values(
    tx: &Transaction<'_>,
    table: &str,
    column: &str,
    source_id: &str,
    values: &[String],
) -> Result<(), String> {
    tx.execute(
        &format!("DELETE FROM {table} WHERE source_id = ?1"),
        params![source_id],
    )
    .map_err(|error| error.to_string())?;
    for value in values {
        tx.execute(
            &format!("INSERT INTO {table} (source_id, {column}) VALUES (?1, ?2)"),
            params![source_id, value],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() || char == '.' || char == '/' {
                char.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect()
}

fn matches_optional(expected: &Option<String>, actual: &str) -> bool {
    expected.as_ref().is_none_or(|value| value == actual)
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_catalog(conn: &mut Connection) {
        conn.execute("DELETE FROM source_aliases", []).unwrap();
        conn.execute("DELETE FROM source_targets", []).unwrap();
        conn.execute("DELETE FROM source_capabilities", []).unwrap();
        conn.execute("DELETE FROM source_canisters", []).unwrap();
        conn.execute("DELETE FROM source_versions", []).unwrap();
        conn.execute("DELETE FROM source_citations", []).unwrap();
        conn.execute("DELETE FROM sources", []).unwrap();
    }

    fn sample_source() -> SourceUpsert {
        SourceUpsert {
            source_id: "/vercel/next.js".to_string(),
            title: "Next.js Docs".to_string(),
            aliases: vec!["next".to_string(), "middleware".to_string()],
            trust: "official".to_string(),
            domain: "code_docs".to_string(),
            skill_kind: None,
            targets: Vec::new(),
            capabilities: Vec::new(),
            canister_ids: vec!["aaaaa-aa".to_string(), "bbbbb-bb".to_string()],
            supported_versions: vec!["14".to_string(), "15".to_string()],
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec!["https://nextjs.org/docs".to_string()],
        }
    }

    fn sample_skill_source() -> SourceUpsert {
        SourceUpsert {
            source_id: "/skills/nextjs/migration".to_string(),
            title: "Next.js Migration Skill".to_string(),
            aliases: vec!["next migration".to_string(), "upgrade".to_string()],
            trust: "curated".to_string(),
            domain: "skill_knowledge".to_string(),
            skill_kind: Some("migration".to_string()),
            targets: vec!["nextjs".to_string()],
            capabilities: vec![
                "auth".to_string(),
                "middleware".to_string(),
                "routing".to_string(),
            ],
            canister_ids: vec!["skill-aa".to_string()],
            supported_versions: Vec::new(),
            retrieved_at: "2026-03-17T00:00:00Z".to_string(),
            citations: vec![
                "https://github.com/ICME-Lab/kinic-context-engine/blob/main/skills/nextjs/migration/SKILL.md"
                    .to_string(),
            ],
        }
    }

    #[test]
    fn get_source_returns_inserted_canisters() {
        let _guard = test_lock().lock().unwrap_or_else(|error| error.into_inner());
        run_migrations_and_seeds();
        with_connection(|mut conn| {
            reset_catalog(&mut conn);
            upsert_source(&mut conn, &sample_source()).unwrap();

            let source = load_source(&conn, "/vercel/next.js")
                .unwrap()
                .expect("source must exist");
            assert_eq!(source.canister_ids.len(), 2);
            assert!(source.skill_kind.is_none());
        });
        close_connection();
    }

    #[test]
    fn resolve_sources_matches_aliases() {
        let _guard = test_lock().lock().unwrap_or_else(|error| error.into_inner());
        run_migrations_and_seeds();
        with_connection(|mut conn| {
            reset_catalog(&mut conn);
            upsert_source(&mut conn, &sample_source()).unwrap();
        });

        let results = resolve_sources("next middleware".to_string(), 5);
        assert_eq!(results[0].source_id, "/vercel/next.js");
        close_connection();
    }

    #[test]
    fn filter_sources_filters_by_domain_and_version() {
        let _guard = test_lock().lock().unwrap_or_else(|error| error.into_inner());
        run_migrations_and_seeds();
        with_connection(|mut conn| {
            reset_catalog(&mut conn);
            upsert_source(&mut conn, &sample_source()).unwrap();
        });

        let filtered = filter_sources(FilterSourcesArgs {
            domain: Some("code_docs".to_string()),
            trust: Some("official".to_string()),
            version: Some("15".to_string()),
            limit: Some(10),
        });
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].source_id, "/vercel/next.js");
        close_connection();
    }

    #[test]
    fn get_source_returns_skill_metadata() {
        let _guard = test_lock().lock().unwrap_or_else(|error| error.into_inner());
        run_migrations_and_seeds();
        with_connection(|mut conn| {
            reset_catalog(&mut conn);
            upsert_source(&mut conn, &sample_skill_source()).unwrap();

            let source = load_source(&conn, "/skills/nextjs/migration")
                .unwrap()
                .expect("skill source must exist");
            assert_eq!(source.skill_kind.as_deref(), Some("migration"));
            assert_eq!(source.targets, vec!["nextjs".to_string()]);
            assert_eq!(
                source.capabilities,
                vec![
                    "auth".to_string(),
                    "middleware".to_string(),
                    "routing".to_string()
                ]
            );
        });
        close_connection();
    }
}
