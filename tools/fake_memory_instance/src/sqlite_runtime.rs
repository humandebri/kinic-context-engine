// Where: tools/fake_memory_instance/src/sqlite_runtime.rs
// What: Runtime SQLite wrapper for wasm canisters and host-side tests.
// Why: Keep stable-memory VFS details out of retrieval behavior.

#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, cell::RefMut, path::PathBuf};

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
pub use ic_sqlite_vfs::db::connection::Connection;
#[cfg(target_arch = "wasm32")]
pub use ic_sqlite_vfs::{DefaultMemoryImpl, MemoryId, MemoryManager, params};

#[cfg(not(target_arch = "wasm32"))]
pub use rusqlite::{self, Connection};

#[cfg(not(target_arch = "wasm32"))]
pub use rusqlite::params;

#[cfg(target_arch = "wasm32")]
const FAKE_MEMORY_SQLITE_MEMORY_ID: u8 = 121;

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
                .get(MemoryId::new(FAKE_MEMORY_SQLITE_MEMORY_ID)),
        )
        .map_err(|error| error.to_string())
    })
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
pub fn with_update<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(&Connection) -> Result<R, String>,
{
    ic_sqlite_vfs::Db::update(|conn| f(conn).map_err(db_error)).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
pub fn with_connection<F, R>(f: F) -> R
where
    F: FnOnce(RefMut<'_, Connection>) -> R,
{
    CONNECTION.with(|conn| {
        let conn_mut = conn.borrow_mut();
        let conn_mut = RefMut::filter_map(conn_mut, |maybe_conn| {
            if maybe_conn.is_none() {
                *maybe_conn = Some(
                    Connection::open(database_path()).expect("host sqlite connection must open"),
                );
            }
            maybe_conn.as_mut()
        })
        .expect("connection must exist");
        f(conn_mut)
    })
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
pub fn prepare_database() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn database_path() -> PathBuf {
    std::env::temp_dir().join("kinic-context-fake-memory.sqlite3")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_database_connection() -> rusqlite::Result<Connection> {
    prepare_database();
    Connection::open(database_path())
}

#[cfg(target_arch = "wasm32")]
pub fn execute(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
) -> Result<(), String> {
    conn.execute(sql, values).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn query_column_strings(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
) -> Result<Vec<String>, String> {
    conn.query_column::<String>(sql, values)
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub fn query_all<T, F>(
    conn: &Connection,
    sql: &str,
    values: &[&dyn ic_sqlite_vfs::db::ToSql],
    f: F,
) -> Result<Vec<T>, String>
where
    F: FnMut(&ic_sqlite_vfs::db::Row<'_>) -> Result<T, ic_sqlite_vfs::DbError>,
{
    conn.query_all(sql, values, f)
        .map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
pub struct Migration {
    pub version: u64,
    pub sql: &'static str,
}

#[cfg(target_arch = "wasm32")]
fn db_error(message: String) -> ic_sqlite_vfs::DbError {
    ic_sqlite_vfs::DbError::Sqlite(1, message)
}
