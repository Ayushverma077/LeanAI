use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::classify::{Exclusion, FileClass};

/// One path in a scanned project. Paths are always relative to the approved
/// project root and use `/` separators on every platform, so a bundle produced
/// on Windows and macOS from the same revision is byte-identical (FR-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub path: String,
    pub size_bytes: u64,
    pub class: FileClass,
    /// May this file enter a bundle without a per-file override?
    pub selectable: bool,
    /// Present whenever `selectable` is false, or when the file is selectable
    /// but the user should know something about it.
    pub exclusion: Option<Exclusion>,
    /// SHA-256 of the file bytes. `None` when hashing was skipped (binary,
    /// oversized, unreadable) — freshness for those falls back to mtime+size.
    pub content_hash: Option<String>,
    /// Modification time in milliseconds since the Unix epoch, when available.
    pub modified_ms: Option<u64>,
}

impl FileEntry {
    pub fn extension(&self) -> Option<&str> {
        self.path.rsplit_once('.').map(|(_, ext)| ext)
    }
}

/// A path that could not be read. Scanning continues; the error is reported
/// per path rather than failing the whole scan (FR-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanIssue {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub files_seen: u64,
    pub directories_seen: u64,
    pub bytes_seen: u64,
    pub elapsed_ms: u64,
    /// True when the scan stopped at `Limits::max_files_scanned`.
    pub truncated: bool,
}

/// The complete result of one scan. Ordering of `files` is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inventory {
    /// Absolute, canonical project root as approved by the user.
    pub root: String,
    /// Stable identifier for the project (see `project::fingerprint`).
    pub project_fingerprint: String,
    /// Git commit + dirty marker, or a scan hash when the project is not a
    /// git repository.
    pub source_revision: String,
    pub policy_version: u32,
    pub scanned_at_ms: u64,
    pub files: Vec<FileEntry>,
    pub issues: Vec<ScanIssue>,
    pub stats: ScanStats,
}

impl Inventory {
    pub fn get(&self, path: &str) -> Option<&FileEntry> {
        self.files
            .binary_search_by(|entry| entry.path.as_str().cmp(path))
            .ok()
            .map(|index| &self.files[index])
    }

    pub fn selectable(&self) -> impl Iterator<Item = &FileEntry> {
        self.files.iter().filter(|entry| entry.selectable)
    }

    /// Count of files per class, for the exclusions summary panel.
    pub fn class_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for entry in &self.files {
            *counts.entry(entry.class.label().to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Total bytes of every selectable file, used for the "select all" warning.
    pub fn selectable_bytes(&self) -> u64 {
        self.selectable().map(|entry| entry.size_bytes).sum()
    }
}
