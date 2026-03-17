// Where: tools/catalog_canister/src/sqlite_runtime.rs
// What: Runtime SQLite wrapper for wasm canisters and host-side tests.
// Why: ic-rusqlite only supports wasm32, so host tests need a compatible shim with the same surface.
#[cfg(target_arch = "wasm32")]
pub use ic_rusqlite::{Connection, OptionalExtension, close_connection, params, with_connection};

#[cfg(not(target_arch = "wasm32"))]
use std::{cell::RefCell, cell::RefMut, path::PathBuf};

#[cfg(not(target_arch = "wasm32"))]
pub use rusqlite::{Connection, OptionalExtension, params};

#[cfg(not(target_arch = "wasm32"))]
thread_local! {
    static CONNECTION: RefCell<Option<Connection>> = const { RefCell::new(None) };
}

#[cfg(not(target_arch = "wasm32"))]
pub fn close_connection() {
    CONNECTION.with(|conn| {
        conn.borrow_mut().take();
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn with_connection<F, R>(f: F) -> R
where
    F: FnOnce(RefMut<'_, Connection>) -> R,
{
    CONNECTION.with(|conn| {
        let conn_mut = conn.borrow_mut();
        let conn_mut = RefMut::filter_map(conn_mut, |maybe_conn| {
            if maybe_conn.is_none() {
                *maybe_conn = Some(
                    Connection::open(db_path()).expect("host sqlite connection must open"),
                );
            }
            maybe_conn.as_mut()
        })
        .expect("connection must exist");
        f(conn_mut)
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn db_path() -> PathBuf {
    std::env::temp_dir().join("kinic-context-catalog.sqlite3")
}
