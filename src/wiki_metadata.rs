// Where: src/wiki_metadata.rs
// What: Metadata helpers for wiki-backed sources and chunks.
// Why: Treat node metadata_json as canonical identity, not lossy wiki paths.
use serde::Deserialize;

use crate::model::SourceMetadata;
use crate::wiki_cli::run_json;

pub(crate) const SOURCES_ROOT: &str = "/Wiki/sources";

#[derive(Debug, Deserialize)]
pub(crate) struct WikiSearchHit {
    pub path: String,
    #[serde(default)]
    pub score: Option<f32>,
    #[serde(default)]
    pub snippet: Option<String>,
    #[serde(default)]
    pub match_reasons: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct WikiNodeEntry {
    pub path: String,
}

#[derive(Debug, Deserialize)]
struct WikiNodeMetadata {
    metadata_json: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct WikiSourceMetadataJson {
    #[serde(default)]
    pub source_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub trust: String,
    #[serde(default)]
    pub domain: String,
    #[serde(default)]
    pub supported_versions: Vec<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub retrieved_at: String,
    #[serde(default)]
    pub citations: Vec<String>,
    #[serde(default)]
    pub citation: String,
    #[serde(default)]
    pub chunk_id: Option<String>,
}

pub(crate) fn source_prefix(source_id: &str) -> String {
    format!("{SOURCES_ROOT}/{}", source_slug(source_id))
}

pub(crate) fn source_slug(source_id: &str) -> String {
    source_id
        .trim_matches('/')
        .replace('/', "__")
        .replace('.', "_")
}

pub(crate) fn source_metadata_from_id(source_id: &str) -> SourceMetadata {
    SourceMetadata {
        source_id: source_id.to_string(),
        title: title_from_source_id(source_id),
        aliases: vec![source_id.trim_matches('/').to_string()],
        trust: "wiki".to_string(),
        domain: "wiki_sources".to_string(),
        supported_versions: Vec::new(),
        retrieved_at: String::new(),
        citations: Vec::new(),
    }
}

pub(crate) fn source_metadata_from_value(
    value: WikiSourceMetadataJson,
    path: &str,
) -> Option<SourceMetadata> {
    if value.source_id.is_empty() {
        eprintln!("skipped source metadata: {path}");
        return None;
    }
    let mut supported_versions = value.supported_versions;
    if supported_versions.is_empty()
        && let Some(version) = value.version
        && !version.is_empty()
    {
        supported_versions.push(version);
    }
    let mut citations = value.citations;
    if citations.is_empty() && !value.citation.is_empty() {
        citations.push(value.citation);
    }
    let aliases = if value.aliases.is_empty() {
        vec![value.source_id.trim_matches('/').to_string()]
    } else {
        value.aliases
    };
    Some(SourceMetadata {
        title: if value.title.is_empty() {
            title_from_source_id(&value.source_id)
        } else {
            value.title
        },
        source_id: value.source_id,
        aliases,
        trust: value.trust,
        domain: value.domain,
        supported_versions,
        retrieved_at: value.retrieved_at,
        citations,
    })
}

pub(crate) fn read_node_metadata(
    cli_bin: &str,
    database_id: &str,
    path: &str,
    skip_kind: &str,
) -> anyhow::Result<Option<WikiSourceMetadataJson>> {
    let node: WikiNodeMetadata = run_json(
        cli_bin,
        database_id,
        vec![
            "read-node".to_string(),
            "--path".to_string(),
            path.to_string(),
            "--metadata-only".to_string(),
            "--json".to_string(),
        ],
        "kinic-vfs-cli read-node failed",
    )?;
    Ok(parse_metadata(&node.metadata_json, path, skip_kind))
}

pub(crate) fn is_source_index_path(path: &str) -> bool {
    let Some(relative) = path.strip_prefix(&format!("{SOURCES_ROOT}/")) else {
        return false;
    };
    let parts = relative.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts[1] == "index.md"
}

pub(crate) fn is_docs_chunk_path(path: &str) -> bool {
    let Some(relative) = path.strip_prefix(&format!("{SOURCES_ROOT}/")) else {
        return false;
    };
    let parts = relative.split('/').collect::<Vec<_>>();
    parts.len() >= 3 && parts.last().is_some_and(|name| name.ends_with(".md"))
}

fn parse_metadata(
    metadata_json: &str,
    path: &str,
    skip_kind: &str,
) -> Option<WikiSourceMetadataJson> {
    match serde_json::from_str(metadata_json) {
        Ok(value) => Some(value),
        Err(_) => {
            eprintln!("skipped {skip_kind} metadata: {path}");
            None
        }
    }
}

fn title_from_source_id(source_id: &str) -> String {
    source_id
        .trim_matches('/')
        .replace(['/', '.', '_', '-'], " ")
        .split_whitespace()
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
