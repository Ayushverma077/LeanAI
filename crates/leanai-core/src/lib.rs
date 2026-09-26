//! Deterministic core of LeanAI Desktop.
//!
//! Everything in this crate runs without a network connection, an API key or a
//! model. That is a product requirement, not an accident: file discovery,
//! filtering, bundling, token estimation and context indexing are the parts a
//! user has to be able to trust, so they are implemented as ordinary,
//! testable code (Instructions.md §3.4, principle 1).
//!
//! The Tauri layer in `src-tauri` adds process, database and UI concerns on top
//! and holds no policy logic of its own.

pub mod agent;
pub mod aiignore;
pub mod approval;
pub mod benchmark;
pub mod catalog;
pub mod classify;
pub mod concat;
pub mod context;
pub mod error;
pub mod gguf;
pub mod gitinfo;
pub mod inventory;
pub mod manifest;
pub mod policy;
pub mod project;
pub mod provider;
pub mod retrieval;
pub mod routing;
pub mod secrets;
pub mod selection;
pub mod symbols;
pub mod tokenizer;
pub mod walker;

pub use error::{CoreError, Result};

/// Bundles everything needed to describe the running policy to the UI, so the
/// frontend never hard-codes a copy of the rules.
pub fn policy_description() -> serde_json::Value {
    let precedence: Vec<serde_json::Value> = [
        policy::IgnoreSource::GitGlobal,
        policy::IgnoreSource::GitInfoExclude,
        policy::IgnoreSource::GitIgnore,
        policy::IgnoreSource::AiIgnore,
        policy::IgnoreSource::LeanAiPolicy,
        policy::IgnoreSource::UserOverride,
    ]
    .iter()
    .map(|source| {
        serde_json::json!({
            "source": source,
            "label": source.label(),
            "rank": source.rank(),
        })
    })
    .collect();

    let limit_reasons: Vec<serde_json::Value> = policy::LIMIT_REASONS
        .iter()
        .map(|(key, reason)| serde_json::json!({ "key": key, "reason": reason }))
        .collect();

    serde_json::json!({
        "policy_version": policy::POLICY_VERSION,
        "limits": policy::Limits::default(),
        "limit_reasons": limit_reasons,
        "ignore_precedence": precedence,
        "secret_scan_disclaimer": secrets::SecretReport::DISCLAIMER,
        "always_skipped_directories": policy::ALWAYS_SKIPPED_DIRS,
    })
}
