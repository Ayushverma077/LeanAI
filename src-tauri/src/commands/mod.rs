//! The typed Tauri command boundary.
//!
//! Every command takes one named request struct and returns one response
//! struct or an `AppError`. No command accepts a free-form string that becomes
//! a filesystem path, a shell argument or a SQL fragment (backlog 1.2).

pub mod agent;
pub mod bundles;
pub mod context;
pub mod diagnostics;
pub mod git;
pub mod models;
pub mod projects;
pub mod settings;

use serde::Serialize;

/// Envelope for long operations that report progress over an event channel.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub phase: String,
    pub files_seen: u64,
    pub directories_seen: u64,
    pub bytes_seen: u64,
}

pub const SCAN_PROGRESS_EVENT: &str = "leanai://scan-progress";
