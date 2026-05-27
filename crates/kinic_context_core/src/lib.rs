// Where: crates/kinic_context_core/src/lib.rs
// What: Shared types and config for the kinic-context CLI.
// Why: Keep output contracts separate from CLI execution concerns.
pub mod types;

#[cfg(not(target_family = "wasm"))]
pub mod config;
