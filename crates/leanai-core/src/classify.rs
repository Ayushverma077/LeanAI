use std::path::Path;

use globset::{Glob, GlobSet, GlobSetBuilder};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::policy::{
    IgnoreSource, Policy, ALWAYS_SKIPPED_DIRS, BINARY_EXTENSIONS, DATA_EXTENSIONS,
    DOCS_PATH_MARKERS, GENERATED_PATH_MARKERS, GENERATED_SUFFIXES, LOCKFILE_NAMES,
    SENSITIVE_PATH_PATTERNS, TEST_FILE_MARKERS, TEST_PATH_MARKERS,
};

/// How LeanAI classifies a path. Mirrors backlog item 2.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileClass {
    /// Readable text that is eligible for a bundle.
    SourceText,
    /// Detected as binary by extension policy or content probe.
    Binary,
    /// Build output, vendored code, minified assets or codegen.
    Generated,
    /// A dependency lockfile.
    Lockfile,
    /// A path that commonly stores credentials.
    CredentialSensitive,
    /// Dotfile or OS metadata not otherwise classified.
    HiddenMetadata,
    /// Exceeds `Limits::max_file_bytes`.
    TooLarge,
    /// A symbolic link.
    Symlink,
    /// Not valid UTF-8 and no safe decoding was available.
    UnsupportedEncoding,
    /// Present but could not be read (permissions, lock, IO error).
    Unreadable,
}

impl FileClass {
    /// Whether a file of this class may be added to a bundle without an
    /// explicit per-file override.
    pub fn selectable_by_default(&self, policy: &Policy) -> bool {
        match self {
            FileClass::SourceText => true,
            FileClass::Generated => !policy.exclude_generated,
            FileClass::Lockfile => !policy.exclude_lockfiles,
            FileClass::HiddenMetadata => policy.include_hidden,
            FileClass::Symlink => policy.follow_symlinks,
            FileClass::Binary
            | FileClass::CredentialSensitive
            | FileClass::TooLarge
            | FileClass::UnsupportedEncoding
            | FileClass::Unreadable => false,
        }
    }

    /// Classes that can never be enabled by a folder-level "select all"
    /// (FR-08). Overriding these requires a per-file, high-friction action.
    pub fn requires_explicit_override(&self) -> bool {
        matches!(
            self,
            FileClass::CredentialSensitive
                | FileClass::Binary
                | FileClass::UnsupportedEncoding
                | FileClass::Unreadable
                | FileClass::TooLarge
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            FileClass::SourceText => "text",
            FileClass::Binary => "binary",
            FileClass::Generated => "generated",
            FileClass::Lockfile => "lockfile",
            FileClass::CredentialSensitive => "credential-sensitive",
            FileClass::HiddenMetadata => "hidden/metadata",
            FileClass::TooLarge => "too large",
            FileClass::Symlink => "symlink",
            FileClass::UnsupportedEncoding => "unsupported encoding",
            FileClass::Unreadable => "unreadable",
        }
    }
}

/// Why a path is not selected, in words a user can act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Exclusion {
    pub source: IgnoreSource,
    pub reason: String,
    /// The rule that matched, when one exists (a glob, an ignore line).
    pub rule: Option<String>,
}

impl Exclusion {
    pub fn policy(reason: impl Into<String>, rule: Option<String>) -> Self {
        Self {
            source: IgnoreSource::LeanAiPolicy,
            reason: reason.into(),
            rule,
        }
    }
}

static SENSITIVE_SET: Lazy<GlobSet> = Lazy::new(|| build_globs(SENSITIVE_PATH_PATTERNS, false));
static SENSITIVE_ALLOW_SET: Lazy<GlobSet> =
    Lazy::new(|| build_globs(SENSITIVE_PATH_PATTERNS, true));

/// Builds a glob set from the pattern list. When `negated` is true only the
/// `!`-prefixed allow patterns are compiled, otherwise only the deny patterns.
fn build_globs(patterns: &[&str], negated: bool) -> GlobSet {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let is_negation = pattern.starts_with('!');
        if is_negation != negated {
            continue;
        }
        let cleaned = pattern.trim_start_matches('!');
        if let Ok(glob) = Glob::new(cleaned) {
            builder.add(glob);
        }
    }
    builder.build().expect("static glob patterns are valid")
}

/// True when the relative path matches a credential-sensitive pattern and is
/// not one of the documented example-file exceptions.
pub fn is_sensitive_path(relative: &str) -> bool {
    let normalized = normalize(relative);
    if SENSITIVE_ALLOW_SET.is_match(&normalized) {
        return false;
    }
    SENSITIVE_SET.is_match(&normalized)
}

pub fn is_lockfile(relative: &str) -> bool {
    file_name(relative).is_some_and(|name| LOCKFILE_NAMES.contains(&name))
}

pub fn is_generated_path(relative: &str) -> bool {
    let normalized = format!("/{}", normalize(relative));
    if GENERATED_PATH_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return true;
    }
    GENERATED_SUFFIXES
        .iter()
        .any(|suffix| normalized.ends_with(suffix))
}

pub fn is_always_skipped_dir(name: &str) -> bool {
    ALWAYS_SKIPPED_DIRS.contains(&name)
}

/// True for tests, test data and snapshots.
pub fn is_test_path(relative: &str) -> bool {
    let normalized = format!("/{}", normalize(relative));
    if TEST_PATH_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
    {
        return true;
    }
    file_name(relative).is_some_and(|name| {
        TEST_FILE_MARKERS.iter().any(|marker| name.contains(marker))
            || name.starts_with("test_")
            || name.starts_with("Test")
    })
}

/// True for documentation, examples and benchmark paths.
pub fn is_docs_path(relative: &str) -> bool {
    let normalized = format!("/{}", normalize(relative));
    DOCS_PATH_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

/// True for structured data files, which are held to a lower size ceiling than
/// source: they are token-dense and rarely useful as context.
pub fn is_data_file(relative: &str) -> bool {
    extension(relative).is_some_and(|ext| DATA_EXTENSIONS.contains(&ext.as_str()))
}

/// The size ceiling that applies to this path.
pub fn size_limit_for(relative: &str, policy: &Policy) -> (u64, &'static str) {
    if is_data_file(relative) {
        (policy.limits.max_data_file_bytes, "max_data_file_bytes")
    } else {
        (policy.limits.max_file_bytes, "max_file_bytes")
    }
}

pub fn has_binary_extension(relative: &str) -> bool {
    extension(relative).is_some_and(|ext| BINARY_EXTENSIONS.contains(&ext.as_str()))
}

/// Whether a path is a dotfile or OS metadata file.
pub fn is_hidden(relative: &str) -> bool {
    normalize(relative)
        .split('/')
        .any(|segment| segment.starts_with('.') && segment != "." && segment != "..")
        || file_name(relative).is_some_and(|name| name == "Thumbs.db" || name == "desktop.ini")
}

/// Content probe. Reads at most `probe_len` bytes and decides whether the
/// buffer looks like binary data.
///
/// A NUL byte, or a high proportion of control bytes that are not ordinary
/// whitespace, means binary. This is the same heuristic git uses, and it never
/// requires reading the whole file (backlog 2.4).
pub fn looks_binary(buffer: &[u8]) -> bool {
    if buffer.is_empty() {
        return false;
    }
    // A UTF-8/UTF-16/UTF-32 BOM means text.
    if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return false;
    }
    if buffer.starts_with(&[0xFF, 0xFE]) || buffer.starts_with(&[0xFE, 0xFF]) {
        // UTF-16 is text but LeanAI cannot bundle it losslessly as UTF-8 source.
        return true;
    }
    if buffer.contains(&0) {
        return true;
    }
    let suspicious = buffer
        .iter()
        .filter(|byte| {
            let b = **byte;
            b < 0x09 || (0x0E..0x20).contains(&b) || b == 0x7F
        })
        .count();
    suspicious * 100 / buffer.len() > 10
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

fn file_name(path: &str) -> Option<&str> {
    Path::new(path).file_name().and_then(|name| name.to_str())
}

fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}
