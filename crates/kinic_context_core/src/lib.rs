// Where: crates/kinic_context_core/src/lib.rs
// What: Read-only IC client core shared by the kinic-context CLI.
// Why: Keep anonymous query plumbing separate from CLI concerns and exclude write/auth code paths.
pub mod catalog;
pub mod client;
pub mod config;
pub mod launcher;
pub mod memory;
pub mod types;
