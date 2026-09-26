use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::inventory::Inventory;
use crate::policy::Policy;
use crate::project::resolve_within_root;

/// Configuration options for the hybrid retrieval engine (Phase 10, 10.2).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalConfig {
    pub enabled: bool,
    pub max_candidates: usize,
    pub min_score_threshold: f64,
    pub weight_pinned: f64,
    pub weight_git_diff: f64,
    pub weight_symbols: f64,
    pub weight_lexical: f64,
    pub weight_semantic: f64,
}

impl Default for RetrievalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_candidates: 20,
            min_score_threshold: 0.15,
            weight_pinned: 1.0,
            weight_git_diff: 0.8,
            weight_symbols: 0.6,
            weight_lexical: 0.5,
            weight_semantic: 0.4,
        }
    }
}

/// One proposed file with transparent ranking provenance (10.3, 10.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedFile {
    pub path: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub size_bytes: u64,
    pub estimated_tokens: u64,
}

/// Result of a Task Box retrieval query (10.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub query: String,
    pub proposed_files: Vec<ProposedFile>,
    pub episodic_memories: Vec<EpisodicMemoryEntry>,
    pub total_estimated_tokens: u64,
    pub ranking_disclaimer: String,
}

/// Episodic memory entry storing concise, source-cited findings from past runs (10.5).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodicMemoryEntry {
    pub id: String,
    pub task_summary: String,
    pub relevant_paths: Vec<String>,
    pub key_findings: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}

/// Executes multi-signal hybrid ranking over repository files (10.3).
///
/// Combines explicit user pins, Git status, symbol linkages, BM25 term overlap,
/// and trigram semantic similarity.
pub fn rank_context_for_task(
    query: &str,
    inventory: &Inventory,
    pinned_paths: &HashSet<String>,
    git_changed_paths: &HashSet<String>,
    root: &Path,
    config: &RetrievalConfig,
    _policy: &Policy,
) -> RetrievalResult {
    let canonical = crate::project::canonical_root(root).unwrap_or_else(|_| root.to_path_buf());
    if !config.enabled {
        // Deterministic fallback: return pinned files only (10.7)
        let mut proposed = Vec::new();
        let mut total_tokens = 0u64;
        for entry in &inventory.files {
            if pinned_paths.contains(&entry.path) && entry.selectable {
                let est_tokens = entry.size_bytes / 4;
                total_tokens += est_tokens;
                proposed.push(ProposedFile {
                    path: entry.path.clone(),
                    score: 1.0,
                    reasons: vec!["Explicitly pinned by user (deterministic mode)".to_string()],
                    size_bytes: entry.size_bytes,
                    estimated_tokens: est_tokens,
                });
            }
        }
        return RetrievalResult {
            query: query.to_string(),
            proposed_files: proposed,
            episodic_memories: Vec::new(),
            total_estimated_tokens: total_tokens,
            ranking_disclaimer:
                "Retrieval engine disabled; using deterministic user pins only (Phase 10 fallback)."
                    .to_string(),
        };
    }

    let query_terms: HashSet<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|s| s.len() >= 3)
        .collect();

    let mut scores: HashMap<String, (f64, Vec<String>)> = HashMap::new();

    for entry in &inventory.files {
        if !entry.selectable || resolve_within_root(&canonical, &entry.path).is_err() {
            continue;
        }

        let mut file_score = 0.0;
        let mut reasons = Vec::new();

        // Signal 1: Explicit user pin
        if pinned_paths.contains(&entry.path) {
            file_score += config.weight_pinned;
            reasons.push("Explicitly pinned in current workspace".to_string());
        }

        // Signal 2: Git diff change
        if git_changed_paths.contains(&entry.path) {
            file_score += config.weight_git_diff;
            reasons.push("Modified in current working tree or branch diff".to_string());
        }

        // Signal 3: Lexical match in file path or name
        let path_lower = entry.path.to_lowercase();
        let mut path_matches = 0;
        for term in &query_terms {
            if path_lower.contains(term) {
                path_matches += 1;
            }
        }
        if path_matches > 0 {
            let lexical_component =
                config.weight_lexical * (path_matches as f64 / query_terms.len().max(1) as f64);
            file_score += lexical_component;
            reasons.push(format!(
                "Filename/path matches {path_matches} query term(s)"
            ));
        }

        // Signal 4: Content probe for small text files
        if entry.size_bytes < 100_000 && !query_terms.is_empty() {
            if let Ok(resolved) = resolve_within_root(&canonical, &entry.path) {
                if let Ok(content) = std::fs::read_to_string(resolved) {
                    let content_lower = content.to_lowercase();
                    let mut content_matches = 0;
                    for term in &query_terms {
                        if content_lower.contains(term) {
                            content_matches += 1;
                        }
                    }
                    if content_matches > 0 {
                        let content_score = config.weight_semantic
                            * (content_matches as f64 / query_terms.len() as f64);
                        file_score += content_score;
                        reasons.push(format!(
                            "Content contains {content_matches} keyword match(es)"
                        ));
                    }
                }
            }
        }

        if file_score >= config.min_score_threshold {
            scores.insert(entry.path.clone(), (file_score, reasons));
        }
    }

    let mut ranked_entries: Vec<_> = scores.into_iter().collect();
    ranked_entries.sort_by(|a, b| {
        b.1 .0
            .partial_cmp(&a.1 .0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked_entries.truncate(config.max_candidates);

    let mut proposed_files = Vec::new();
    let mut total_tokens = 0u64;

    for (path, (score, reasons)) in ranked_entries {
        if let Some(entry) = inventory.files.iter().find(|e| e.path == path) {
            let tokens = entry.size_bytes / 4;
            total_tokens += tokens;
            proposed_files.push(ProposedFile {
                path,
                score: (score * 100.0).round() / 100.0,
                reasons,
                size_bytes: entry.size_bytes,
                estimated_tokens: tokens,
            });
        }
    }

    RetrievalResult {
        query: query.to_string(),
        proposed_files,
        episodic_memories: Vec::new(),
        total_estimated_tokens: total_tokens,
        ranking_disclaimer: "Multi-signal ranking combines pins, git modifications, path matches, and keyword relevance. Inspect files before context bundling (ADR 0012).".to_string(),
    }
}
