// Where: src/lib.rs
// What: Read-only context runtime for public source retrieval.
// Why: Expose a small, AI-safe core that powers the CLI without write paths.
pub mod catalog;
pub mod cli;
pub mod config;
pub mod engine;
pub mod model;
pub mod output;
pub mod provider;
