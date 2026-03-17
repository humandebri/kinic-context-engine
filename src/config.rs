// Where: src/config.rs
// What: CLI-facing runtime configuration wrapper.
// Why: Keep construction of registry/provider dependencies out of the binary entrypoint.
pub use kinic_context_core::config::ReadConfig;
