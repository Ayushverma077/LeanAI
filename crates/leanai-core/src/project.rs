use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};

/// Canonicalises and validates a user-chosen project root.
///
/// Every later filesystem operation resolves against this value, so this is the
/// single place where the trust boundary is established (FR-01, NFR 7.1).
pub fn canonical_root(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let canonical =
        std::fs::canonicalize(path).map_err(|_| CoreError::InvalidRoot(path.to_path_buf()))?;
    if !canonical.is_dir() {
        return Err(CoreError::InvalidRoot(path.to_path_buf()));
    }
    Ok(canonical)
}

/// Resolves `relative` against `root` and refuses anything that escapes it.
///
/// Rejects absolute paths, `..` traversal, and symlinks whose target resolves
/// outside the root. Used by every read that takes a path from the UI, a
/// preset, a model response or a manifest.
pub fn resolve_within_root(root: &Path, relative: &str) -> Result<PathBuf> {
    let candidate = Path::new(relative);
    if candidate.is_absolute() {
        return Err(CoreError::OutsideRoot(candidate.to_path_buf()));
    }
    for component in candidate.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(CoreError::OutsideRoot(candidate.to_path_buf())),
        }
    }

    let joined = root.join(candidate);
    // Canonicalise when the path exists so a symlink cannot point outside the
    // root; fall back to a lexical check for paths that do not exist yet.
    let resolved = match std::fs::canonicalize(&joined) {
        Ok(resolved) => resolved,
        Err(_) => joined.clone(),
    };
    if !resolved.starts_with(root) {
        return Err(CoreError::OutsideRoot(joined));
    }
    Ok(resolved)
}

/// Converts an absolute path inside `root` into a normalised relative path.
pub fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| CoreError::OutsideRoot(path.to_path_buf()))?;
    Ok(normalize_separators(relative))
}

pub fn normalize_separators(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// A stable project identity that does not depend on the absolute path alone.
///
/// Combines the git repository's first-commit id when available (so a moved or
/// re-cloned checkout keeps its identity) with a hash of the canonical path as
/// a fallback. Never contains the raw path, so it is safe in telemetry and
/// exported manifests.
pub fn fingerprint(root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"leanai.project/v1");
    if let Some(root_commit) = git_root_commit(root) {
        hasher.update(b"git:");
        hasher.update(root_commit.as_bytes());
    } else {
        hasher.update(b"path:");
        hasher.update(root.to_string_lossy().as_bytes());
    }
    format!("proj_{}", &hex(&hasher.finalize())[..24])
}

fn git_root_commit(root: &Path) -> Option<String> {
    let repo = git2::Repository::open(root).ok()?;
    let head = repo.head().ok()?.peel_to_commit().ok()?;
    let mut walk = repo.revwalk().ok()?;
    walk.push(head.id()).ok()?;
    walk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)
        .ok()?;
    walk.next()?.ok().map(|oid| oid.to_string())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex(&hasher.finalize())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn system_time_ms(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_millis() as u64)
}
