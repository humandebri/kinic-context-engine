// Where: tools/catalog_canister/src/sqlite_runtime.rs
// What: SQLite runtime boundary for catalog canister storage.
// Why: Keep IC stable-memory VFS access separate from host-side rusqlite tests.

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
pub use ic_sqlite_vfs::db::connection::Connection;
#[cfg(target_arch = "wasm32")]
pub use ic_sqlite_vfs::{DefaultMemoryImpl, MemoryId, MemoryManager, params};

#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, path::PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub use rusqlite::{Connection, params};

#[cfg(target_arch = "wasm32")]
const CATALOG_SQLITE_MEMORY_ID: u8 = 120;

pub struct Migration {
    pub version: u64,
    pub sql: &'static str,
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));
}

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static CONNECTION: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
pub fn init_db() -> Result<(), String> {
    MEMORY_MANAGER.with(|manager| {
        ic_sqlite_vfs::Db::init(
            manager
                .borrow()
                .get(MemoryId::new(CATALOG_SQLITE_MEMORY_ID)),
        )
        .map_err(|error| error.to_string())
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn init_db() -> Result<(), String> {
    with_connection(|_| Ok(()))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
pub fn close_connection() {
    CONNECTION.with(|conn| {
        conn.borrow_mut().take();
    });
}

#[cfg(target_arch = "wasm32")]
pub fn with_query<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    ic_sqlite_vfs::Db::query(|conn| f(conn).map_err(db_error)).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_query<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    with_connection(f)
}

#[cfg(target_arch = "wasm32")]
pub fn with_update<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    ic_sqlite_vfs::Db::update(|conn| f(conn).map_err(db_error)).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_update<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    with_connection(f)
}

#[cfg(target_arch = "wasm32")]
pub fn migrate(migrations: &[Migration]) -> Result<(), String> {
    let migrations = migrations
        .iter()
        .map(|migration| ic_sqlite_vfs::db::migrate::Migration {
            version: migration.version,
            sql: migration.sql,
        })
        .collect::<Vec<_>>();
    ic_sqlite_vfs::Db::migrate(&migrations).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn migrate(migrations: &[Migration]) -> Result<(), String> {
    with_update(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS __ic_sqlite_migrations (
                version INTEGER PRIMARY KEY NOT NULL
            )",
        )
        .map_err(|error| error.to_string())?;
        for migration in migrations {
            let version = i64::try_from(migration.version).map_err(|error| error.to_string())?;
            let exists = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM __ic_sqlite_migrations WHERE version = ?1)",
                    params![version],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|error| error.to_string())?;
            if exists != 0 {
                continue;
            }
            conn.execute_batch(migration.sql)
                .map_err(|error| error.to_string())?;
            conn.execute(
                "INSERT INTO __ic_sqlite_migrations(version) VALUES (?1)",
                params![version],
            )
            .map_err(|error| error.to_string())?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
pub fn execute(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
) -> Result<(), String> {
    conn.execute(sql, values).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn execute<P: rusqlite::Params>(conn: &Connection, sql: &str, values: P) -> Result<(), String> {
    conn.execute(sql, values)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

pub fn load_source_ids(conn: &Connection) -> Result<Vec<String>, String> {
    query_column_strings(
        conn,
        "SELECT source_id FROM sources ORDER BY source_id",
        params![],
    )
}

pub fn load_source_base(
    conn: &Connection,
    source_id: &str,
) -> Result<Option<(String, String, String, String, Option<String>, String)>, String> {
    query_optional_source_base(
        conn,
        "SELECT source_id, title, trust, domain, skill_kind, retrieved_at FROM sources WHERE source_id = ?1",
        params![source_id],
    )
}

pub fn collect_values(
    conn: &Connection,
    sql: &str,
    source_id: &str,
) -> Result<Vec<String>, String> {
    query_column_strings(conn, sql, params![source_id])
}

#[cfg(target_arch = "wasm32")]
fn query_column_strings(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
) -> Result<Vec<String>, String> {
    conn.query_column::<String>(sql, values)
        .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn query_column_strings<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    values: P,
) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(sql).map_err(|error| error.to_string())?;
    stmt.query_map(values, |row| row.get::<_, String>(0))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn query_optional_source_base(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
) -> Result<Option<(String, String, String, String, Option<String>, String)>, String> {
    conn.query_optional(sql, values, |row| {
        let skill_kind = empty_to_none(row.get::<Option<String>>(4)?);
        Ok((
            row.get::<String>(0)?,
            row.get::<String>(1)?,
            row.get::<String>(2)?,
            row.get::<String>(3)?,
            skill_kind,
            row.get::<String>(5)?,
        ))
    })
    .map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn query_optional_source_base<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    values: P,
) -> Result<Option<(String, String, String, String, Option<String>, String)>, String> {
    match conn.query_row(sql, values, |row| {
        let skill_kind = empty_to_none(row.get::<_, Option<String>>(4)?);
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            skill_kind,
            row.get::<_, String>(5)?,
        ))
    }) {
        Ok(row) => Ok(Some(row)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.filter(|item| !item.is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
fn with_connection<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    CONNECTION.with(|conn| {
        let mut slot = conn.borrow_mut();
        if slot.is_none() {
            *slot = Some(Connection::open(db_path()).map_err(|error| error.to_string())?);
        }
        f(slot.as_ref().expect("connection must exist"))
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn db_path() -> PathBuf {
    std::env::temp_dir().join(format!("kinic-context-catalog-{}.sqlite3", std::process::id()))
}

#[cfg(target_arch = "wasm32")]
fn db_error(message: String) -> ic_sqlite_vfs::DbError {
    ic_sqlite_vfs::DbError::Sqlite(1, message)
}
