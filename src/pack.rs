// Where: src/pack.rs
// What: Pack planning and token estimation helpers for hybrid retrieval.
// Why: Keep selection and budget policy explicit so tests can measure efficiency changes.
use crate::model::{PackResolvedSource, ResolvedSource, SourceSnippet};
use std::collections::BTreeSet;

const MIN_SOURCE_SCORE_RATIO: f32 = 0.45;
const LOW_TOKEN_BUDGET: usize = 256;
const MEDIUM_TOKEN_BUDGET: usize = 1_200;

pub fn build_resolved_source_details(
    candidates: &[ResolvedSource],
    selected: &[ResolvedSource],
) -> Vec<PackResolvedSource> {
    candidates
        .iter()
        .map(|candidate| PackResolvedSource {
            source_id: candidate.source_id.clone(),
            title: candidate.title.clone(),
            score: candidate.score,
            reasons: candidate.reasons.clone(),
            queried: selected
                .iter()
                .any(|selected_source| selected_source.source_id == candidate.source_id),
        })
        .collect()
}

pub fn estimate_pack_tokens(evidence: &[SourceSnippet]) -> usize {
    evidence.iter().map(approximate_tokens).sum()
}

pub fn per_source_top_k(selected_sources: usize, max_tokens: usize) -> usize {
    if selected_sources == 0 {
        return 1;
    }

    let per_source_budget = max_tokens / selected_sources.max(1);
    if per_source_budget < LOW_TOKEN_BUDGET {
        1
    } else if per_source_budget < MEDIUM_TOKEN_BUDGET {
        2
    } else {
        3
    }
}

pub fn select_sources_for_pack(
    candidates: &[ResolvedSource],
    max_sources: usize,
    max_tokens: usize,
) -> Vec<ResolvedSource> {
    if candidates.is_empty() || max_sources == 0 {
        return Vec::new();
    }

    let source_cap = source_limit_for_budget(max_sources, max_tokens);
    let leader_score = candidates[0].score.max(0.01);
    let mut selected = Vec::new();
    let mut seen_source_ids = BTreeSet::new();

    for candidate in candidates {
        if selected.len() >= source_cap {
            break;
        }
        if !seen_source_ids.insert(candidate.source_id.clone()) {
            continue;
        }
        if selected.is_empty() || candidate.score >= leader_score * MIN_SOURCE_SCORE_RATIO {
            selected.push(candidate.clone());
        }
    }

    if selected.is_empty() {
        selected.push(candidates[0].clone());
    }

    selected
}

fn approximate_tokens(snippet: &SourceSnippet) -> usize {
    let chars = snippet.title.chars().count()
        + snippet.snippet.chars().count()
        + snippet.citation.chars().count();
    chars.div_ceil(4)
}

fn source_limit_for_budget(max_sources: usize, max_tokens: usize) -> usize {
    let budget_cap = if max_tokens < LOW_TOKEN_BUDGET {
        1
    } else if max_tokens < MEDIUM_TOKEN_BUDGET {
        2
    } else {
        3
    };
    max_sources.max(1).min(budget_cap)
}
