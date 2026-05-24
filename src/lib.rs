// Where: src/lib.rs
// What: Read-only context runtime for public source retrieval.
// Why: Expose a small, AI-safe core that powers the CLI without write paths.
pub mod benchmark;
pub mod catalog;
pub mod cli;
pub mod config;
pub mod e5;
pub mod embedding;
pub mod engine;
pub mod embed_protocol;
pub mod model;
pub mod output;
pub mod pack;
pub mod provider;
