// Where: tools/fake_memory_instance/src/lib.rs
// What: SQLite-backed fake source canister for PocketIC integration tests.
// Why: Keep L0 section narrowing and L2 hybrid retrieval behavior testable without live canisters.
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use ic_cdk::pre_upgrade;
use ic_cdk::{init, post_upgrade, query, update};
#[cfg(not(target_arch = "wasm32"))]
use ic_hybrid_engine::HybridEngine;
use kinic_context_core::types::{
    HybridQueryFilters, HybridQueryRequest, HybridSearchResult, IndexedDocument, SectionIndexRecord,
};

#[cfg(not(target_arch = "wasm32"))]
mod hybrid_adapter;
mod sqlite_runtime;

#[cfg(not(target_arch = "wasm32"))]
use hybrid_adapter::{search_payload, to_engine_document, to_engine_request, to_wire_result};
#[cfg(not(target_arch = "wasm32"))]
use sqlite_runtime::{
    Connection, close_connection, open_database_connection, params, prepare_database, rusqlite,
};
#[cfg(target_arch = "wasm32")]
use sqlite_runtime::{Connection, execute, migrate, params, query_all, query_column_strings};

const RRF_K: f32 = 60.0;
const SEARCH_TOP_K: u32 = 64;
const SECTIONS_MIGRATION: &str = include_str!("../migrations/002_sections.sql");
#[cfg(target_arch = "wasm32")]
const WASM_DOCUMENTS_MIGRATION: &str = "
CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    snippet TEXT NOT NULL,
    citation TEXT NOT NULL,
    version TEXT,
    content TEXT NOT NULL,
    section TEXT,
    tags_json TEXT NOT NULL DEFAULT '[]',
    embedding_json TEXT NOT NULL,
    search_text_primary TEXT NOT NULL DEFAULT '',
    search_text_secondary TEXT NOT NULL DEFAULT ''
);

CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts
USING fts5(primary_text, secondary_text, tags_text, tokenize='trigram');
";
#[cfg(target_arch = "wasm32")]
const MIGRATIONS: &[sqlite_runtime::Migration] = &[
    sqlite_runtime::Migration {
        version: 0,
        sql: WASM_DOCUMENTS_MIGRATION,
    },
    sqlite_runtime::Migration {
        version: 1,
        sql: SECTIONS_MIGRATION,
    },
];

#[init]
fn init(documents: Vec<IndexedDocument>) {
    run_migrations().expect("schema migration must succeed");
    reset(0);
    for document in documents {
        insert_document(document);
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[pre_upgrade]
fn pre_upgrade() {
    close_connection();
}

#[post_upgrade]
fn post_upgrade() {
    run_migrations().expect("schema migration must succeed");
}

#[update]
fn reset(_dim: u32) {
    clear_index().expect("reset must succeed");
}

#[update]
fn insert_document(document: IndexedDocument) {
    insert_indexed_document(&document).expect("insert must succeed");
}

#[update]
fn insert_section(record: SectionIndexRecord) {
    #[cfg(target_arch = "wasm32")]
    {
        sqlite_runtime::with_update(|conn| insert_section_record(conn, &record))
            .expect("insert must succeed");
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let conn = open_database_connection().expect("sqlite connection must open");
        insert_section_record(&conn, &record).expect("insert must succeed");
    }
}

#[query]
fn search(query_embedding: Vec<f32>) -> Vec<(f32, String)> {
    #[cfg(target_arch = "wasm32")]
    {
        return search_impl(&query_embedding).expect("vector search must succeed");
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        with_hybrid_engine(|engine| {
            let mut results = engine
                .search(&query_embedding, SEARCH_TOP_K, Some(SEARCH_TOP_K))
                .map_err(|error| error.to_string())?;
            results.retain(|item| item.score > 0.0);
            Ok(results
                .into_iter()
                .map(|item| (item.score, search_payload(&item)))
                .collect())
        })
        .expect("vector search must succeed")
    }
}

#[query]
fn hybrid_query(request: HybridQueryRequest) -> Vec<HybridSearchResult> {
    hybrid_query_impl(&request).expect("hybrid query must succeed")
}

fn hybrid_query_impl(request: &HybridQueryRequest) -> Result<Vec<HybridSearchResult>, String> {
    let filters = request.filters.clone().unwrap_or_default();
    let policy = infer_retrieval_policy(request, &filters);
    let (results, _) = hybrid_query_with_policy(request, &filters, &policy)?;
    Ok(results)
}

fn hybrid_query_with_policy(
    request: &HybridQueryRequest,
    filters: &HybridQueryFilters,
    policy: &RetrievalPolicy,
) -> Result<(Vec<HybridSearchResult>, RetrievalDiagnostics), String> {
    #[cfg(target_arch = "wasm32")]
    {
        hybrid_query_with_policy_in_db(request, filters, policy)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        hybrid_query_with_policy_at_path(
            sqlite_runtime::database_path().as_path(),
            request,
            filters,
            policy,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn hybrid_query_with_policy_at_path(
    database_path: &Path,
    request: &HybridQueryRequest,
    filters: &HybridQueryFilters,
    policy: &RetrievalPolicy,
) -> Result<(Vec<HybridSearchResult>, RetrievalDiagnostics), String> {
    let conn = Connection::open(database_path).map_err(|error| error.to_string())?;
    let version = request.version.as_deref();
    let section_ids = resolve_section_candidates(&conn, request, version, filters, policy)?;
    let scoped_request = to_engine_request(request, filters);
    let results = with_hybrid_engine_at_path(database_path, |engine| {
        engine
            .hybrid_query(&scoped_request)
            .map_err(|error| error.to_string())
    })?;
    let fallback_used = results
        .iter()
        .any(|item| item.breakdown.keyword_score <= 0.0);
    let mut by_key: HashMap<String, HybridSearchResult> = HashMap::new();
    for result in results {
        let wire_result = to_wire_result(result);
        if !result_allowed(&wire_result, filters, &section_ids) {
            continue;
        }
        let adjusted = adjust_result(wire_result, filters, policy, &section_ids);
        let key = result_key(&adjusted);
        match by_key.get(&key) {
            Some(existing) if existing.score >= adjusted.score => {}
            _ => {
                by_key.insert(key, adjusted);
            }
        }
    }
    let mut ranked = by_key.into_values().collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.score.total_cmp(&left.score));
    let document_candidate_count = ranked.len();
    ranked.truncate(request.top_k.max(1) as usize);
    Ok((
        ranked,
        RetrievalDiagnostics {
            section_candidate_count: section_ids.len(),
            document_candidate_count,
            fallback_used,
        },
    ))
}

fn section_limit(request: &HybridQueryRequest, policy: &RetrievalPolicy) -> usize {
    let derived = (request.top_k.max(1) as usize * policy.section_overflow)
        .max(policy.min_section_candidates);
    policy
        .max_section_candidates
        .map(|limit| derived.min(limit))
        .unwrap_or(derived)
}

fn run_migrations() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        sqlite_runtime::init_db()?;
        return migrate(MIGRATIONS);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        run_migrations_at_path(sqlite_runtime::database_path().as_path())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_migrations_at_path(database_path: &Path) -> Result<(), String> {
    run_document_migrations_at_path(database_path)?;
    run_section_migrations_at_path(database_path)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn run_document_migrations_at_path(database_path: &Path) -> Result<(), String> {
    with_hybrid_engine_at_path(database_path, |engine| {
        engine.migrate().map_err(|error| error.to_string())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn run_section_migrations_at_path(database_path: &Path) -> Result<(), String> {
    let conn = Connection::open(database_path).map_err(|error| error.to_string())?;
    conn.execute_batch(SECTIONS_MIGRATION)
        .map_err(|error| error.to_string())
}

fn clear_index() -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return sqlite_runtime::with_update(clear_index_in_db);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        clear_index_at_path(sqlite_runtime::database_path().as_path())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_index_at_path(database_path: &Path) -> Result<(), String> {
    clear_document_index_at_path(database_path)?;
    clear_section_index_at_path(database_path)?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_document_index_at_path(database_path: &Path) -> Result<(), String> {
    with_hybrid_engine_at_path(database_path, |engine| {
        engine.clear_documents().map_err(|error| error.to_string())
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_section_index_at_path(database_path: &Path) -> Result<(), String> {
    let conn = Connection::open(database_path).map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM sections", [])
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM sections_fts", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn insert_indexed_document(document: &IndexedDocument) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        return sqlite_runtime::with_update(|conn| insert_indexed_document_in_db(conn, document));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        insert_indexed_document_at_path(sqlite_runtime::database_path().as_path(), document)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn insert_indexed_document_at_path(
    database_path: &Path,
    document: &IndexedDocument,
) -> Result<(), String> {
    with_hybrid_engine_at_path(database_path, |engine| {
        engine
            .insert_document(&to_engine_document(document))
            .map_err(|error| error.to_string())
    })
}

fn insert_section_record(conn: &Connection, record: &SectionIndexRecord) -> Result<(), String> {
    let scope_key = section_scope_key(&record.section_id, record.version.as_deref());
    let version = record.version.clone().unwrap_or_default();
    conn.execute(
        "INSERT INTO sections (scope_key, section_id, title, summary, version, embedding_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(scope_key) DO UPDATE SET
            title = excluded.title,
            summary = excluded.summary,
            version = excluded.version,
            embedding_json = excluded.embedding_json",
        params![
            scope_key,
            record.section_id,
            record.title,
            record.summary,
            version,
            serde_json::to_string(&record.embedding).map_err(|error| error.to_string())?,
        ],
    )
    .map_err(|error| error.to_string())?;
    let scope_key = section_scope_key(&record.section_id, record.version.as_deref());
    #[cfg(target_arch = "wasm32")]
    let row_id = conn
        .query_scalar::<i64>(
            "SELECT id FROM sections WHERE scope_key = ?1",
            params![scope_key],
        )
        .map_err(|error| error.to_string())?;
    #[cfg(not(target_arch = "wasm32"))]
    let row_id = conn
        .query_row(
            "SELECT id FROM sections WHERE scope_key = ?1",
            params![scope_key],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| error.to_string())?;
    conn.execute("DELETE FROM sections_fts WHERE rowid = ?1", params![row_id])
        .map_err(|error| error.to_string())?;
    conn.execute(
        "INSERT INTO sections_fts(rowid, title, summary) VALUES (?1, ?2, ?3)",
        params![row_id, record.title, record.summary],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn with_hybrid_engine<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&mut HybridEngine) -> Result<R, String>,
{
    with_hybrid_engine_at_path(sqlite_runtime::database_path().as_path(), f)
}

#[cfg(not(target_arch = "wasm32"))]
fn with_hybrid_engine_at_path<F, R>(database_path: &Path, f: F) -> Result<R, String>
where
    F: FnOnce(&mut HybridEngine) -> Result<R, String>,
{
    prepare_database();
    let mut engine = HybridEngine::open(database_path).map_err(|error| error.to_string())?;
    f(&mut engine)
}

#[cfg(target_arch = "wasm32")]
fn clear_index_in_db(conn: &Connection) -> Result<(), String> {
    execute(conn, "DELETE FROM documents_fts", params![])?;
    execute(conn, "DELETE FROM documents", params![])?;
    execute(conn, "DELETE FROM sections_fts", params![])?;
    execute(conn, "DELETE FROM sections", params![])?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn insert_indexed_document_in_db(
    conn: &Connection,
    document: &IndexedDocument,
) -> Result<(), String> {
    let tags_json = serde_json::to_string(&document.tags).map_err(|error| error.to_string())?;
    let embedding_json =
        serde_json::to_string(&document.embedding).map_err(|error| error.to_string())?;
    let primary = join_search_text([&document.title, &document.snippet, &document.content]);
    let secondary = document.tags.join(" ");
    let version = document.version.clone().unwrap_or_default();
    let section = document.section.clone().unwrap_or_default();
    execute(
        conn,
        "INSERT INTO documents (
            title, snippet, citation, version, content, section, tags_json,
            embedding_json, search_text_primary, search_text_secondary
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            document.title.clone(),
            document.snippet.clone(),
            document.citation.clone(),
            version,
            document.content.clone(),
            section,
            tags_json,
            embedding_json,
            primary,
            secondary,
        ],
    )?;
    let row_id = conn
        .query_scalar::<i64>("SELECT last_insert_rowid()", params![])
        .map_err(|error| error.to_string())?;
    execute(
        conn,
        "INSERT INTO documents_fts(rowid, primary_text, secondary_text, tags_text)
         VALUES (?1, ?2, ?3, ?4)",
        params![row_id, primary, document.content, secondary],
    )
}

#[cfg(target_arch = "wasm32")]
fn search_impl(query_embedding: &[f32]) -> Result<Vec<(f32, String)>, String> {
    let mut rows = sqlite_runtime::with_query(load_documents)?;
    rows.iter_mut()
        .for_each(|row| row.score = cosine_similarity(query_embedding, &row.embedding));
    rows.retain(|row| row.score > 0.0);
    rows.sort_by(|left, right| right.score.total_cmp(&left.score));
    rows.truncate(SEARCH_TOP_K as usize);
    rows.into_iter()
        .map(|row| Ok((row.score, row.search_payload()?)))
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn hybrid_query_with_policy_in_db(
    request: &HybridQueryRequest,
    filters: &HybridQueryFilters,
    policy: &RetrievalPolicy,
) -> Result<(Vec<HybridSearchResult>, RetrievalDiagnostics), String> {
    sqlite_runtime::with_query(|conn| {
        let version = request.version.as_deref();
        let section_ids = resolve_section_candidates(conn, request, version, filters, policy)?;
        let mut documents = load_documents(conn)?;
        let keyword_ids = keyword_document_ids(conn, &request.query_text, version, request)?;
        let mut vector_ranked = documents
            .iter()
            .map(|document| {
                (
                    document.id,
                    cosine_similarity(&request.query_embedding, &document.embedding),
                )
            })
            .filter(|(_, score)| *score > 0.0)
            .collect::<Vec<_>>();
        vector_ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
        let vector_limit = request
            .candidate_limit
            .unwrap_or_else(|| request.top_k.saturating_mul(4).max(16))
            as usize;
        vector_ranked.truncate(vector_limit);

        let keyword_ranks = rank_map_i64(&keyword_ids);
        let vector_ids = vector_ranked.iter().map(|(id, _)| *id).collect::<Vec<_>>();
        let vector_ranks = rank_map_i64(&vector_ids);
        let vector_scores = vector_ranked.into_iter().collect::<HashMap<_, _>>();

        let mut results = Vec::new();
        for document in documents.drain(..) {
            let keyword_score = keyword_ranks
                .get(&document.id)
                .copied()
                .map(reciprocal_rank_from_rank)
                .unwrap_or(0.0);
            let vector_score = vector_ranks
                .get(&document.id)
                .copied()
                .map(reciprocal_rank_from_rank)
                .unwrap_or(0.0);
            if keyword_score <= 0.0 && vector_score <= 0.0 {
                continue;
            }
            let mut result = document.to_result(
                keyword_score * policy.keyword_weight
                    + vector_score * policy.vector_weight
                    + vector_scores.get(&document.id).copied().unwrap_or(0.0) * 0.001,
                keyword_score,
                vector_score,
            );
            if !result_allowed(&result, filters, &section_ids) {
                continue;
            }
            result = adjust_result(result, filters, policy, &section_ids);
            results.push(result);
        }

        let fallback_used = results
            .iter()
            .any(|item| item.keyword_score.unwrap_or(0.0) <= 0.0);
        results.sort_by(|left, right| right.score.total_cmp(&left.score));
        let document_candidate_count = results.len();
        results.truncate(request.top_k.max(1) as usize);
        Ok((
            results,
            RetrievalDiagnostics {
                section_candidate_count: section_ids.len(),
                document_candidate_count,
                fallback_used,
            },
        ))
    })
}

#[cfg(target_arch = "wasm32")]
fn load_documents(conn: &Connection) -> Result<Vec<DocumentRow>, String> {
    query_all(
        conn,
        "SELECT id, title, snippet, citation, version, content, section, tags_json, embedding_json
         FROM documents",
        params![],
        |row| {
            let tags_json = row.get::<String>(7)?;
            let embedding_json = row.get::<String>(8)?;
            let tags = serde_json::from_str::<Vec<String>>(&tags_json)
                .map_err(|error| ic_sqlite_vfs::DbError::Sqlite(1, error.to_string()))?;
            let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json)
                .map_err(|error| ic_sqlite_vfs::DbError::Sqlite(1, error.to_string()))?;
            Ok(DocumentRow {
                id: row.get::<i64>(0)?,
                title: row.get::<String>(1)?,
                snippet: row.get::<String>(2)?,
                citation: row.get::<String>(3)?,
                version: empty_to_none(row.get::<Option<String>>(4)?),
                section: empty_to_none(row.get::<Option<String>>(6)?),
                tags,
                embedding,
                score: 0.0,
            })
        },
    )
}

#[cfg(target_arch = "wasm32")]
fn keyword_document_ids(
    conn: &Connection,
    query_text: &str,
    version: Option<&str>,
    request: &HybridQueryRequest,
) -> Result<Vec<i64>, String> {
    let normalized = normalize_query_text(query_text);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let limit = request
        .candidate_limit
        .unwrap_or_else(|| request.top_k.saturating_mul(4).max(16)) as i64;
    let version = version.unwrap_or_default();
    query_all(
        conn,
        "SELECT d.id
         FROM documents_fts f
         JOIN documents d ON d.id = f.rowid
         WHERE documents_fts MATCH ?1
           AND (?2 = '' OR d.version = ?2)
         ORDER BY bm25(documents_fts, 4.0, 1.0, 0.5)
         LIMIT ?3",
        params![normalized, version, limit],
        |row| row.get::<i64>(0),
    )
}

#[cfg(target_arch = "wasm32")]
fn rank_map_i64(ids: &[i64]) -> HashMap<i64, usize> {
    ids.iter()
        .copied()
        .enumerate()
        .map(|(rank, id)| (id, rank))
        .collect()
}

#[cfg(target_arch = "wasm32")]
fn join_search_text(parts: [&str; 3]) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn adjust_result(
    mut result: HybridSearchResult,
    filters: &HybridQueryFilters,
    policy: &RetrievalPolicy,
    section_ids: &[String],
) -> HybridSearchResult {
    let tags = result.tags.clone().unwrap_or_default();
    let section = result.section.clone();
    result.score += policy.section_match_boost(&section);
    result.match_reasons = Some(match_reasons(
        &tags,
        &section,
        filters,
        section_ids,
        policy,
        result.keyword_score.unwrap_or(0.0),
        result.vector_score.unwrap_or(0.0),
    ));
    result
}

#[cfg(not(target_arch = "wasm32"))]
fn result_key(result: &HybridSearchResult) -> String {
    format!(
        "{}\u{1f}{}\u{1f}{}",
        result.title,
        result.citation,
        result.version.as_deref().unwrap_or_default()
    )
}

fn resolve_section_candidates(
    conn: &Connection,
    request: &HybridQueryRequest,
    version: Option<&str>,
    filters: &HybridQueryFilters,
    policy: &RetrievalPolicy,
) -> Result<Vec<String>, String> {
    if let Some(section) = filters.section.clone() {
        return Ok(vec![section]);
    }

    let sections = load_sections(conn, version)?;
    if sections.is_empty() {
        return Ok(Vec::new());
    }

    let limit = section_limit(request, policy);
    let keyword_ids = keyword_section_ids(conn, &request.query_text, version, limit)?;
    let vector_ids = vector_section_ids(&sections, &request.query_embedding, limit);
    let keyword_ranks = rank_map_string(&keyword_ids);
    let vector_ranks = rank_map_string(&vector_ids);

    let mut ranked = sections
        .into_iter()
        .map(|section| {
            let keyword_score = keyword_ranks
                .get(&section.section_id)
                .copied()
                .map(reciprocal_rank_from_rank)
                .unwrap_or(0.0);
            let vector_score = vector_ranks
                .get(&section.section_id)
                .copied()
                .map(reciprocal_rank_from_rank)
                .unwrap_or(0.0);
            let score = keyword_score * policy.keyword_weight
                + vector_score * policy.vector_weight
                + policy.section_id_boost(&section.section_id);
            (section.section_id, score)
        })
        .filter(|(_, score)| *score > 0.0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked.truncate(limit);
    Ok(ranked
        .into_iter()
        .map(|(section_id, _)| section_id)
        .collect())
}

#[cfg(not(target_arch = "wasm32"))]
fn load_sections(conn: &Connection, version: Option<&str>) -> Result<Vec<SectionRow>, String> {
    let sql = "SELECT section_id, embedding_json
        FROM sections
        WHERE (?1 IS NULL OR version = ?1)";
    let mut stmt = conn.prepare(sql).map_err(|error| error.to_string())?;
    stmt.query_map(rusqlite::params![version], |row| {
        let embedding_json = row.get::<_, String>(1)?;
        let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                embedding_json.len(),
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
        Ok(SectionRow {
            section_id: row.get(0)?,
            embedding,
        })
    })
    .map_err(|error| error.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn load_sections(conn: &Connection, version: Option<&str>) -> Result<Vec<SectionRow>, String> {
    let sql = "SELECT section_id, embedding_json
        FROM sections
        WHERE (?1 = '' OR version = ?1)";
    let version = version.unwrap_or_default();
    query_all(conn, sql, params![version], |row| {
        let embedding_json = row.get::<String>(1)?;
        let embedding = serde_json::from_str::<Vec<f32>>(&embedding_json)
            .map_err(|error| ic_sqlite_vfs::DbError::Sqlite(1, error.to_string()))?;
        Ok(SectionRow {
            section_id: row.get::<String>(0)?,
            embedding,
        })
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn keyword_section_ids(
    conn: &Connection,
    query_text: &str,
    version: Option<&str>,
    limit: usize,
) -> Result<Vec<String>, String> {
    let normalized = normalize_query_text(query_text);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let sql = "SELECT s.section_id
        FROM sections_fts f
        JOIN sections s ON s.id = f.rowid
        WHERE sections_fts MATCH ?1
          AND (?2 IS NULL OR s.version = ?2)
        ORDER BY bm25(sections_fts, 4.0, 1.0)
        LIMIT ?3";
    let mut stmt = conn.prepare(sql).map_err(|error| error.to_string())?;
    stmt.query_map(
        rusqlite::params![normalized, version, limit as i64],
        |row| row.get::<_, String>(0),
    )
    .map_err(|error| error.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn keyword_section_ids(
    conn: &Connection,
    query_text: &str,
    version: Option<&str>,
    limit: usize,
) -> Result<Vec<String>, String> {
    let normalized = normalize_query_text(query_text);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }
    let sql = "SELECT s.section_id
        FROM sections_fts f
        JOIN sections s ON s.id = f.rowid
        WHERE sections_fts MATCH ?1
          AND (?2 = '' OR s.version = ?2)
        ORDER BY bm25(sections_fts, 4.0, 1.0)
        LIMIT ?3";
    let version = version.unwrap_or_default();
    query_column_strings(conn, sql, params![normalized, version, limit as i64])
}

fn vector_section_ids(
    sections: &[SectionRow],
    query_embedding: &[f32],
    limit: usize,
) -> Vec<String> {
    let mut ranked = sections
        .iter()
        .map(|section| {
            (
                section.section_id.clone(),
                cosine_similarity(query_embedding, &section.embedding),
            )
        })
        .filter(|(_, score)| *score > 0.0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.total_cmp(&left.1));
    ranked.truncate(limit);
    ranked
        .into_iter()
        .map(|(section_id, _)| section_id)
        .collect()
}

fn rank_map_string(ids: &[String]) -> HashMap<String, usize> {
    ids.iter()
        .cloned()
        .enumerate()
        .map(|(rank, id)| (id, rank))
        .collect()
}

fn match_reasons(
    tags: &[String],
    section: &Option<String>,
    filters: &HybridQueryFilters,
    section_ids: &[String],
    policy: &RetrievalPolicy,
    keyword_score: f32,
    vector_score: f32,
) -> Vec<String> {
    let mut reasons = Vec::new();
    reasons.push(format!("policy:{}", policy.kind.as_str()));
    if keyword_score > 0.0 {
        reasons.push("candidate:keyword".to_string());
        reasons.push("keyword:candidate".to_string());
    } else {
        reasons.push("candidate:fallback".to_string());
    }
    if vector_score > 0.0 {
        reasons.push("vector:candidate".to_string());
    }
    if !section_ids.is_empty() {
        reasons.push("section:candidate".to_string());
    }
    if filters.section.is_some() {
        reasons.push("filter:section".to_string());
    }
    if let Some(tag) = primary_tag(filters) {
        if tags
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(tag))
        {
            reasons.push(format!("filter:tag:{tag}"));
        }
    }
    if let Some(section_id) = section.as_deref() {
        if policy.section_id_boost(section_id) > 0.0 {
            reasons.push(format!("policy:section:{section_id}"));
        }
    }
    reasons
}

fn reciprocal_rank_from_rank(rank: usize) -> f32 {
    1.0 / (RRF_K + rank as f32 + 1.0)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    if left.is_empty() || right.is_empty() || left.len() != right.len() {
        return 0.0;
    }
    let dot = left.iter().zip(right).map(|(l, r)| l * r).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm * right_norm)
    }
}

fn primary_tag(filters: &HybridQueryFilters) -> Option<&str> {
    filters.tags.first().map(String::as_str)
}

fn normalize_query_text(query_text: &str) -> String {
    query_text
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch.is_whitespace() {
                ch.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn result_allowed(
    result: &HybridSearchResult,
    filters: &HybridQueryFilters,
    section_ids: &[String],
) -> bool {
    if !tags_allowed(result.tags.as_deref(), &filters.tags) {
        return false;
    }
    if let Some(expected) = filters.section.as_deref() {
        return result.section.as_deref() == Some(expected);
    }
    if section_ids.is_empty() {
        return true;
    }
    result
        .section
        .as_deref()
        .is_some_and(|section| section_ids.iter().any(|candidate| candidate == section))
}

fn tags_allowed(actual: Option<&[String]>, expected: &[String]) -> bool {
    if expected.is_empty() {
        return true;
    }
    let Some(actual) = actual else {
        return false;
    };
    expected.iter().all(|tag| {
        actual
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(tag))
    })
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn empty_to_none(value: Option<String>) -> Option<String> {
    value.filter(|item| !item.is_empty())
}

fn section_scope_key(section_id: &str, version: Option<&str>) -> String {
    format!("{}::{section_id}", version.unwrap_or_default())
}

#[derive(Clone, Copy)]
enum RetrievalPolicyKind {
    Exact,
    Migration,
    Ambiguous,
    Task,
}

impl RetrievalPolicyKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Migration => "migration",
            Self::Ambiguous => "ambiguous",
            Self::Task => "task",
        }
    }
}

struct RetrievalPolicy {
    kind: RetrievalPolicyKind,
    keyword_weight: f32,
    vector_weight: f32,
    section_overflow: usize,
    max_section_candidates: Option<usize>,
    min_section_candidates: usize,
    preferred_sections: Vec<String>,
}

impl RetrievalPolicy {
    fn section_id_boost(&self, section_id: &str) -> f32 {
        if self
            .preferred_sections
            .iter()
            .any(|candidate| section_id.eq_ignore_ascii_case(candidate))
        {
            0.02
        } else {
            0.0
        }
    }

    fn section_match_boost(&self, section: &Option<String>) -> f32 {
        section
            .as_deref()
            .map(|section_id| self.section_id_boost(section_id))
            .unwrap_or(0.0)
    }
}

fn infer_retrieval_policy(
    request: &HybridQueryRequest,
    filters: &HybridQueryFilters,
) -> RetrievalPolicy {
    if filters.section.is_some() || !filters.tags.is_empty() {
        return RetrievalPolicy {
            kind: RetrievalPolicyKind::Exact,
            keyword_weight: request.keyword_weight.unwrap_or(0.8),
            vector_weight: request.vector_weight.unwrap_or(0.2),
            section_overflow: 1,
            max_section_candidates: None,
            min_section_candidates: 1,
            preferred_sections: Vec::new(),
        };
    }

    let normalized = normalize_query_text(&request.query_text);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let has_migration = tokens.iter().any(|token| {
        matches!(
            *token,
            "migration" | "migrate" | "upgrade" | "breaking" | "version" | "versions"
        )
    });
    if has_migration {
        return RetrievalPolicy {
            kind: RetrievalPolicyKind::Migration,
            keyword_weight: request.keyword_weight.unwrap_or(0.75),
            vector_weight: request.vector_weight.unwrap_or(0.25),
            section_overflow: 2,
            max_section_candidates: Some(2),
            min_section_candidates: 2,
            preferred_sections: vec![
                "migration".to_string(),
                "upgrade".to_string(),
                "versioning".to_string(),
            ],
        };
    }

    let topic_markers = [
        "next",
        "react",
        "supabase",
        "launchagent",
        "auth",
        "middleware",
    ];
    let distinct_topic_hits = topic_markers
        .iter()
        .filter(|marker| tokens.iter().any(|token| token == *marker))
        .count();
    let has_hooks_marker = tokens
        .iter()
        .any(|token| matches!(*token, "hooks" | "hook"));
    let has_launchagent_marker = tokens
        .iter()
        .any(|token| matches!(*token, "launchagent" | "plist"));
    let has_next_react_pair =
        tokens.iter().any(|token| *token == "next") && tokens.iter().any(|token| *token == "react");
    let has_ambiguous = tokens.iter().any(|token| {
        matches!(
            *token,
            "hooks" | "hook" | "compare" | "vs" | "launchagent" | "plist"
        )
    }) || distinct_topic_hits >= 3
        || (tokens.iter().any(|token| *token == "next")
            && tokens.iter().any(|token| *token == "react"));
    if has_ambiguous {
        return RetrievalPolicy {
            kind: RetrievalPolicyKind::Ambiguous,
            keyword_weight: request.keyword_weight.unwrap_or(0.45),
            vector_weight: request.vector_weight.unwrap_or(0.55),
            section_overflow: 3,
            max_section_candidates: if has_launchagent_marker
                || (has_hooks_marker && has_next_react_pair)
            {
                Some(1)
            } else {
                Some(2)
            },
            min_section_candidates: 3,
            preferred_sections: if has_launchagent_marker {
                vec!["launchd".to_string()]
            } else if has_hooks_marker && has_next_react_pair {
                vec!["routing".to_string()]
            } else {
                Vec::new()
            },
        };
    }

    let has_task_language = tokens.iter().any(|token| {
        matches!(
            *token,
            "protect" | "build" | "setup" | "implement" | "with" | "using" | "route"
        )
    });
    if has_task_language || tokens.len() > 3 {
        return RetrievalPolicy {
            kind: RetrievalPolicyKind::Task,
            keyword_weight: request.keyword_weight.unwrap_or(0.35),
            vector_weight: request.vector_weight.unwrap_or(0.65),
            section_overflow: 1,
            max_section_candidates: Some(1),
            min_section_candidates: 2,
            preferred_sections: vec![
                "auth".to_string(),
                "middleware".to_string(),
                "routing".to_string(),
            ],
        };
    }

    RetrievalPolicy {
        kind: RetrievalPolicyKind::Exact,
        keyword_weight: request.keyword_weight.unwrap_or(0.7),
        vector_weight: request.vector_weight.unwrap_or(0.3),
        section_overflow: 1,
        max_section_candidates: None,
        min_section_candidates: 1,
        preferred_sections: tokens.iter().map(|token| (*token).to_string()).collect(),
    }
}

struct SectionRow {
    section_id: String,
    embedding: Vec<f32>,
}

#[cfg(target_arch = "wasm32")]
struct DocumentRow {
    id: i64,
    title: String,
    snippet: String,
    citation: String,
    version: Option<String>,
    section: Option<String>,
    tags: Vec<String>,
    embedding: Vec<f32>,
    score: f32,
}

#[cfg(target_arch = "wasm32")]
impl DocumentRow {
    fn search_payload(&self) -> Result<String, String> {
        serde_json::to_string(&serde_json::json!({
            "title": self.title,
            "snippet": self.snippet,
            "citation": self.citation,
            "version": self.version,
            "section": self.section,
            "tags": self.tags,
        }))
        .map_err(|error| error.to_string())
    }

    fn to_result(&self, score: f32, keyword_score: f32, vector_score: f32) -> HybridSearchResult {
        HybridSearchResult {
            title: self.title.clone(),
            snippet: self.snippet.clone(),
            citation: self.citation.clone(),
            version: self.version.clone(),
            score,
            keyword_score: Some(keyword_score),
            vector_score: Some(vector_score),
            section: self.section.clone(),
            tags: Some(self.tags.clone()),
            match_reasons: None,
        }
    }
}

#[allow(dead_code)]
struct RetrievalDiagnostics {
    section_candidate_count: usize,
    document_candidate_count: usize,
    fallback_used: bool,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BenchmarkPolicyMode {
    Baseline,
    Current,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, PartialEq)]
pub struct RetrievalBenchmarkResult {
    pub results: Vec<HybridSearchResult>,
    pub section_candidate_count: usize,
    pub document_candidate_count: usize,
    pub fallback_used: bool,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn evaluate_query_for_benchmark(
    documents: &[IndexedDocument],
    request: &HybridQueryRequest,
    mode: BenchmarkPolicyMode,
) -> Result<RetrievalBenchmarkResult, String> {
    let temp_dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    let database_path = temp_dir.path().join("benchmark.sqlite3");
    run_migrations_at_path(&database_path)?;
    clear_index_at_path(&database_path)?;
    for document in documents {
        insert_indexed_document_at_path(&database_path, document)?;
    }
    let conn = Connection::open(&database_path).map_err(|error| error.to_string())?;
    for section in derive_sections(documents)? {
        insert_section_record(&conn, &section)?;
    }
    let filters = request.filters.clone().unwrap_or_default();
    let policy = match mode {
        BenchmarkPolicyMode::Baseline => baseline_retrieval_policy(request),
        BenchmarkPolicyMode::Current => infer_retrieval_policy(request, &filters),
    };
    let (results, diagnostics) =
        hybrid_query_with_policy_at_path(&database_path, request, &filters, &policy)?;
    Ok(RetrievalBenchmarkResult {
        results,
        section_candidate_count: diagnostics.section_candidate_count,
        document_candidate_count: diagnostics.document_candidate_count,
        fallback_used: diagnostics.fallback_used,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn baseline_retrieval_policy(request: &HybridQueryRequest) -> RetrievalPolicy {
    RetrievalPolicy {
        kind: RetrievalPolicyKind::Task,
        keyword_weight: request.keyword_weight.unwrap_or(0.5),
        vector_weight: request.vector_weight.unwrap_or(0.5),
        section_overflow: 1,
        max_section_candidates: None,
        min_section_candidates: 1,
        preferred_sections: Vec::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn derive_sections(documents: &[IndexedDocument]) -> Result<Vec<SectionIndexRecord>, String> {
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

#[cfg(not(target_arch = "wasm32"))]
struct SectionAccumulator {
    title: String,
    snippets: Vec<String>,
    embedding_sum: Vec<f32>,
    embedding_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result_with_tags(tags: &[&str]) -> HybridSearchResult {
        HybridSearchResult {
            title: "Result".to_string(),
            snippet: "Snippet".to_string(),
            citation: "https://example.com".to_string(),
            version: None,
            score: 1.0,
            keyword_score: Some(1.0),
            vector_score: Some(0.0),
            section: Some("middleware".to_string()),
            tags: Some(tags.iter().map(|tag| (*tag).to_string()).collect()),
            match_reasons: None,
        }
    }

    #[test]
    fn result_allowed_requires_all_requested_tags_case_insensitively() {
        let filters = HybridQueryFilters {
            section: None,
            tags: vec!["AUTH".to_string(), "cookies".to_string()],
        };
        assert!(result_allowed(
            &result_with_tags(&["auth", "cookies", "next.js"]),
            &filters,
            &[],
        ));
        assert!(!result_allowed(
            &result_with_tags(&["auth", "next.js"]),
            &filters,
            &[],
        ));
    }

    #[test]
    fn empty_to_none_normalizes_sqlite_empty_string_sentinel() {
        assert_eq!(empty_to_none(Some(String::new())), None);
        assert_eq!(
            empty_to_none(Some("15".to_string())),
            Some("15".to_string())
        );
        assert_eq!(empty_to_none(None), None);
    }
}
