// Where: crates/kinic_context_core/src/lib.rs
// What: Read-only IC client core shared by the kinic-context CLI.
// Why: Keep anonymous query plumbing separate from CLI concerns and exclude write/auth code paths.
pub mod types;

#[cfg(not(target_family = "wasm"))]
pub mod catalog;
#[cfg(not(target_family = "wasm"))]
pub mod client;
#[cfg(not(target_family = "wasm"))]
pub mod config;
#[cfg(not(target_family = "wasm"))]
pub mod launcher;
#[cfg(not(target_family = "wasm"))]
pub mod memory;
