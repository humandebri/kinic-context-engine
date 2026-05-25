// Where: src/embed_protocol.rs
// What: Shared stdin/stdout contract for the local Rust embedding helper.
// Why: Keep source_ops and the helper binary on one stable JSON shape.
use serde::{Deserialize, Serialize};

pub const MODEL_NAME: &str = "intfloat/multilingual-e5-large";
pub const EMBEDDING_DIM: usize = 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EmbedKind {
    Query,
    Document,
    Section,
}

impl EmbedKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Document => "document",
            Self::Section => "section",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct EmbedRequest {
    pub kind: EmbedKind,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EmbedResponse {
    pub embedding: Vec<f32>,
    pub model: String,
}
