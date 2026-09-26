use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ignore::WalkBuilder;

use crate::aiignore;
use crate::classify::{self, Exclusion, FileClass};
use crate::error::{CoreError, Result};
use crate::gitinfo;
use crate::inventory::{FileEntry, Inventory, ScanIssue, ScanStats};
use crate::policy::{IgnoreSource, Policy};
use crate::project;

/// Cooperative cancellation shared with the UI. Checked between directory
/// entries so a cancelled scan leaves no partial state behind (FR-06).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Progress reported while walking, so the UI can stay responsive.
#[derive(Debug, Clone, Copy)]
pub struct ScanProgress {
    pub files_seen: u64,
    pub directories_seen: u64,
    pub bytes_seen: u64,
}

#[derive(Clone)]
pub struct ScanOptions {
    pub policy: Policy,
    /// Hash text file contents during the scan. Required for context freshness
    /// and bundle determinism checks; can be disabled for a fast first paint.
    pub hash_contents: bool,
    /// Emitted roughly every `progress_every` files.
    pub progress_every: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            policy: Policy::default(),
            hash_contents: true,
            progress_every: 500,
        }
    }
}

/// Walks a project root and produces a deterministic inventory.
///
/// The walk is single-threaded on purpose: ordering is part of the product
/// contract (FR-09) and a parallel walk would require a full re-sort anyway on
/// trees this size.
pub fn scan(
    root: &Path,
    options: &ScanOptions,
    cancel: &CancelToken,
    mut on_progress: impl FnMut(ScanProgress),
) -> Result<Inventory> {
    let started = Instant::now();
    let root = project::canonical_root(root)?;
    let policy = &options.policy;

    let mut builder = WalkBuilder::new(&root);
    builder
        .standard_filters(false)
        .hidden(false)
        .parents(true)
        .git_ignore(policy.respect_git_ignore)
        .git_global(policy.respect_git_ignore)
        .git_exclude(policy.respect_git_ignore)
        .require_git(false)
        .ignore(false)
        .follow_links(false)
        .max_depth(Some(policy.limits.max_depth));

    if policy.respect_ai_ignore {
        // `.aiignore` uses gitignore syntax and is layered above `.gitignore`
        // (docs/adr/0003-ignore-precedence.md).
        builder.add_custom_ignore_filename(aiignore::FILE_NAME);
    }

    let skip_root = root.clone();
    builder.filter_entry(move |entry| {
        let Some(name) = entry.file_name().to_str() else {
            return false;
        };
        if entry.path() == skip_root {
            return true;
        }
        let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
        !(is_dir && classify::is_always_skipped_dir(name))
    });

    let mut files = Vec::new();
    let mut issues = Vec::new();
    let mut stats = ScanStats::default();

    for result in builder.build() {
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                let path = match &error {
                    ignore::Error::WithPath { path, .. } => project::relative_path(&root, path)
                        .unwrap_or_else(|_| "<unknown>".to_string()),
                    _ => "<unknown>".to_string(),
                };
                issues.push(ScanIssue {
                    path,
                    message: sanitize(&error.to_string()),
                });
                continue;
            }
        };

        let path = entry.path();
        if path == root {
            continue;
        }

        let file_type = entry.file_type();
        if file_type.is_some_and(|kind| kind.is_dir()) {
            stats.directories_seen += 1;
            continue;
        }

        let relative = match project::relative_path(&root, path) {
            Ok(relative) => relative,
            Err(_) => {
                // The walker produced a path outside the approved root. This
                // should be impossible; record it rather than silently drop it.
                issues.push(ScanIssue {
                    path: path.to_string_lossy().into_owned(),
                    message: "skipped: resolved outside the project root".to_string(),
                });
                continue;
            }
        };

        stats.files_seen += 1;
        if files.len() >= policy.limits.max_files_scanned {
            stats.truncated = true;
            break;
        }

        let is_symlink = file_type.is_some_and(|kind| kind.is_symlink());
        let entry = classify_entry(&root, &relative, is_symlink, options, &mut issues);
        stats.bytes_seen += entry.size_bytes;
        files.push(entry);

        if options.progress_every > 0 && stats.files_seen % options.progress_every == 0 {
            on_progress(ScanProgress {
                files_seen: stats.files_seen,
                directories_seen: stats.directories_seen,
                bytes_seen: stats.bytes_seen,
            });
        }
    }

    if cancel.is_cancelled() {
        return Err(CoreError::Cancelled);
    }

    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    stats.elapsed_ms = started.elapsed().as_millis() as u64;
    issues.sort_by(|a, b| a.path.cmp(&b.path));

    let source_revision = gitinfo::source_revision(&root, &files);

    on_progress(ScanProgress {
        files_seen: stats.files_seen,
        directories_seen: stats.directories_seen,
        bytes_seen: stats.bytes_seen,
    });

    Ok(Inventory {
        root: root.to_string_lossy().into_owned(),
        project_fingerprint: project::fingerprint(&root),
        source_revision,
        policy_version: policy.version,
        scanned_at_ms: project::now_ms(),
        files,
        issues,
        stats,
    })
}

fn classify_entry(
    root: &Path,
    relative: &str,
    is_symlink: bool,
    options: &ScanOptions,
    issues: &mut Vec<ScanIssue>,
) -> FileEntry {
    let policy = &options.policy;
    let absolute = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));

    let metadata = match std::fs::symlink_metadata(&absolute) {
        Ok(metadata) => metadata,
        Err(error) => {
            issues.push(ScanIssue {
                path: relative.to_string(),
                message: sanitize(&error.to_string()),
            });
            return unreadable_entry(relative, 0, &error.to_string());
        }
    };
    let size = metadata.len();
    let modified_ms = metadata.modified().ok().and_then(project::system_time_ms);

    // Order matters: credential-sensitive beats every other classification so a
    // `.env` is never reported merely as "text".
    if classify::is_sensitive_path(relative) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::CredentialSensitive,
            policy,
            Some(Exclusion::policy(
                "This path commonly stores credentials. Selecting it requires a per-file confirmation.",
                Some(relative.to_string()),
            )),
            None,
        );
    }

    if is_symlink {
        let escapes = project::resolve_within_root(root, relative).is_err();
        let reason = if escapes {
            "Symbolic link resolves outside the project root and was not followed."
        } else {
            "Symbolic link. LeanAI does not follow links by default; select the target file instead."
        };
        return build(
            relative,
            size,
            modified_ms,
            FileClass::Symlink,
            policy,
            Some(Exclusion::policy(reason, None)),
            None,
        );
    }

    let (size_limit, limit_name) = classify::size_limit_for(relative, policy);
    if size > size_limit {
        let detail = if limit_name == "max_data_file_bytes" {
            format!(
                "{} of structured data exceeds the {} limit for data files. Data this large is token-dense and rarely useful as context.",
                human_bytes(size),
                human_bytes(size_limit)
            )
        } else {
            format!(
                "{} exceeds the {} limit for a single file.",
                human_bytes(size),
                human_bytes(size_limit)
            )
        };
        return build(
            relative,
            size,
            modified_ms,
            FileClass::TooLarge,
            policy,
            Some(Exclusion::policy(detail, Some(limit_name.to_string()))),
            None,
        );
    }

    if classify::has_binary_extension(relative) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::Binary,
            policy,
            Some(Exclusion::policy(
                "Binary file type.",
                Some(format!("*.{}", relative.rsplit('.').next().unwrap_or(""))),
            )),
            None,
        );
    }

    // Content probe: bounded read, never the whole file.
    let probe = match probe_head(&absolute, policy.limits.binary_probe_bytes) {
        Ok(probe) => probe,
        Err(error) => {
            issues.push(ScanIssue {
                path: relative.to_string(),
                message: sanitize(&error.to_string()),
            });
            return unreadable_entry(relative, size, &error.to_string());
        }
    };
    if classify::looks_binary(&probe) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::Binary,
            policy,
            Some(Exclusion::policy(
                "Content probe found binary data in the first bytes of the file.",
                None,
            )),
            None,
        );
    }

    let hash = if options.hash_contents {
        match std::fs::read(&absolute) {
            Ok(bytes) => {
                if std::str::from_utf8(&bytes).is_err() {
                    return build(
                        relative,
                        size,
                        modified_ms,
                        FileClass::UnsupportedEncoding,
                        policy,
                        Some(Exclusion::policy(
                            "File is not valid UTF-8. Convert it before bundling.",
                            None,
                        )),
                        None,
                    );
                }
                Some(project::sha256_hex(&bytes))
            }
            Err(error) => {
                issues.push(ScanIssue {
                    path: relative.to_string(),
                    message: sanitize(&error.to_string()),
                });
                return unreadable_entry(relative, size, &error.to_string());
            }
        }
    } else {
        None
    };

    if classify::is_lockfile(relative) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::Lockfile,
            policy,
            Some(Exclusion::policy(
                "Dependency lockfile: large and rarely useful as context.",
                None,
            )),
            hash,
        );
    }

    if classify::is_generated_path(relative) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::Generated,
            policy,
            Some(Exclusion::policy(
                "Generated, vendored or minified output.",
                None,
            )),
            hash,
        );
    }

    if classify::is_hidden(relative) {
        return build(
            relative,
            size,
            modified_ms,
            FileClass::HiddenMetadata,
            policy,
            Some(Exclusion::policy(
                "Hidden or editor/OS metadata file.",
                None,
            )),
            hash,
        );
    }

    build(
        relative,
        size,
        modified_ms,
        FileClass::SourceText,
        policy,
        None,
        hash,
    )
}

fn build(
    relative: &str,
    size: u64,
    modified_ms: Option<u64>,
    class: FileClass,
    policy: &Policy,
    exclusion: Option<Exclusion>,
    content_hash: Option<String>,
) -> FileEntry {
    let selectable = class.selectable_by_default(policy);
    FileEntry {
        path: relative.to_string(),
        size_bytes: size,
        class,
        selectable,
        // A file can be selectable and still carry an explanation (a lockfile
        // when the user turned the lockfile filter off, for example).
        exclusion: if selectable && class == FileClass::SourceText {
            None
        } else {
            exclusion
        },
        content_hash,
        modified_ms,
    }
}

fn unreadable_entry(relative: &str, size: u64, reason: &str) -> FileEntry {
    FileEntry {
        path: relative.to_string(),
        size_bytes: size,
        class: FileClass::Unreadable,
        selectable: false,
        exclusion: Some(Exclusion {
            source: IgnoreSource::LeanAiPolicy,
            reason: format!("Could not read this file: {}", sanitize(reason)),
            rule: None,
        }),
        content_hash: None,
        modified_ms: None,
    }
}

/// Reads at most `limit` bytes from the head of a file.
fn probe_head(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let mut buffer = Vec::with_capacity(limit.min(8_192));
    file.take(limit as u64).read_to_end(&mut buffer)?;
    Ok(buffer)
}

/// Strips absolute paths out of OS error strings so displayed errors and
/// diagnostic bundles do not leak the user's directory layout.
fn sanitize(message: &str) -> String {
    message
        .split_whitespace()
        .map(|token| {
            if token.starts_with('/') || token.contains(":\\") {
                "<path>"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
