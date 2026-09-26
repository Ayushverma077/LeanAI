use std::path::Path;

use git2::{Repository, Status, StatusOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{CoreError, Result};
use crate::inventory::FileEntry;
use crate::project;

/// Which changes a git-diff selection should include (backlog 4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffScope {
    /// Changes staged in the index.
    Staged,
    /// Changes in the working tree that are not staged.
    Unstaged,
    /// Files git does not track.
    Untracked,
    /// Everything that differs from an explicit base reference.
    AgainstRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRequest {
    pub scopes: Vec<DiffScope>,
    /// Required when `scopes` contains `AgainstRef`; e.g. `main`, `HEAD~3`.
    pub base_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub scope: DiffScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitState {
    pub is_repository: bool,
    pub head_ref: Option<String>,
    pub head_commit: Option<String>,
    pub is_dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub timestamp_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRemoteStatus {
    pub is_repository: bool,
    pub current_branch: Option<String>,
    pub remotes: Vec<RemoteInfo>,
    pub upstream_branch: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub unpushed_commits: Vec<CommitSummary>,
    pub is_dirty: bool,
}

pub fn git_state(root: &Path) -> GitState {
    let Ok(repo) = Repository::open(root) else {
        return GitState {
            is_repository: false,
            head_ref: None,
            head_commit: None,
            is_dirty: false,
        };
    };
    let head = repo.head().ok();
    GitState {
        is_repository: true,
        head_ref: head
            .as_ref()
            .and_then(|head| head.shorthand().map(str::to_string)),
        head_commit: head
            .as_ref()
            .and_then(|head| head.peel_to_commit().ok())
            .map(|commit| commit.id().to_string()),
        is_dirty: is_dirty(&repo),
    }
}

fn is_dirty(repo: &Repository) -> bool {
    let mut options = StatusOptions::new();
    options.include_untracked(true).include_ignored(false);
    repo.statuses(Some(&mut options))
        .map(|statuses| !statuses.is_empty())
        .unwrap_or(false)
}

/// Returns the remote tracking status of a repository (remotes, upstream branch, ahead/behind counts).
pub fn remote_status(root: &Path) -> Result<GitRemoteStatus> {
    let repo = match Repository::open(root) {
        Ok(repo) => repo,
        Err(_) => {
            return Ok(GitRemoteStatus {
                is_repository: false,
                current_branch: None,
                remotes: Vec::new(),
                upstream_branch: None,
                ahead: 0,
                behind: 0,
                unpushed_commits: Vec::new(),
                is_dirty: false,
            });
        }
    };

    let dirty = is_dirty(&repo);

    let mut remotes = Vec::new();
    if let Ok(remote_names) = repo.remotes() {
        for name in remote_names.iter().flatten() {
            if let Ok(remote) = repo.find_remote(name) {
                if let Some(url) = remote.url() {
                    let protocol = if url.starts_with("git@") || url.starts_with("ssh://") {
                        "ssh".to_string()
                    } else if url.starts_with("https://") || url.starts_with("http://") {
                        "https".to_string()
                    } else {
                        "other".to_string()
                    };
                    remotes.push(RemoteInfo {
                        name: name.to_string(),
                        url: url.to_string(),
                        protocol,
                    });
                }
            }
        }
    }

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            return Ok(GitRemoteStatus {
                is_repository: true,
                current_branch: None,
                remotes,
                upstream_branch: None,
                ahead: 0,
                behind: 0,
                unpushed_commits: Vec::new(),
                is_dirty: dirty,
            });
        }
    };

    let current_branch = head.shorthand().map(str::to_string);
    let head_oid = head.target();

    let mut upstream_branch = None;
    let mut ahead = 0;
    let mut behind = 0;
    let mut unpushed_commits = Vec::new();

    if let Some(branch_name) = &current_branch {
        if let Ok(local_branch) = repo.find_branch(branch_name, git2::BranchType::Local) {
            if let Ok(upstream) = local_branch.upstream() {
                if let Ok(Some(upstream_name)) = upstream.name() {
                    upstream_branch = Some(upstream_name.to_string());
                }
                if let (Some(local_oid), Some(up_oid)) = (head_oid, upstream.get().target()) {
                    if let Ok((a, b)) = repo.graph_ahead_behind(local_oid, up_oid) {
                        ahead = a;
                        behind = b;
                    }

                    if ahead > 0 {
                        if let Ok(mut revwalk) = repo.revwalk() {
                            let _ = revwalk.push(local_oid);
                            let _ = revwalk.hide(up_oid);
                            for oid in revwalk.take(ahead.min(50)).flatten() {
                                if let Ok(commit) = repo.find_commit(oid) {
                                    let summary = commit.summary().unwrap_or("").to_string();
                                    let author =
                                        commit.author().name().unwrap_or("Unknown").to_string();
                                    let timestamp_ms = commit.time().seconds() * 1000;
                                    unpushed_commits.push(CommitSummary {
                                        id: oid.to_string(),
                                        summary,
                                        author,
                                        timestamp_ms,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(GitRemoteStatus {
        is_repository: true,
        current_branch,
        remotes,
        upstream_branch,
        ahead,
        behind,
        unpushed_commits,
        is_dirty: dirty,
    })
}

/// A revision identifier stored with every bundle and context document.
///
/// Prefers the git commit (with a `+dirty` marker when the tree has local
/// changes). Projects that are not git repositories get a content hash of the
/// inventory so determinism is still verifiable (FR-12).
pub fn source_revision(root: &Path, files: &[FileEntry]) -> String {
    if let Ok(repo) = Repository::open(root) {
        if let Ok(commit) = repo.head().and_then(|head| head.peel_to_commit()) {
            let suffix = if is_dirty(&repo) { "+dirty" } else { "" };
            return format!("git:{}{}", commit.id(), suffix);
        }
    }
    let mut hasher = Sha256::new();
    hasher.update(b"leanai.scan/v1");
    for file in files {
        hasher.update(file.path.as_bytes());
        hasher.update(b"\0");
        hasher.update(file.size_bytes.to_le_bytes());
        if let Some(hash) = &file.content_hash {
            hasher.update(hash.as_bytes());
        }
    }
    format!("scan:{}", &project::hex(&hasher.finalize())[..40])
}

/// Lists changed files for the requested scopes.
pub fn changed_files(root: &Path, request: &DiffRequest) -> Result<Vec<ChangedFile>> {
    let repo =
        Repository::open(root).map_err(|_| CoreError::NotAGitRepository(root.to_path_buf()))?;
    let mut changed = Vec::new();

    let wants = |scope: DiffScope| request.scopes.contains(&scope);

    if wants(DiffScope::Staged) || wants(DiffScope::Unstaged) || wants(DiffScope::Untracked) {
        let mut options = StatusOptions::new();
        options
            .include_untracked(wants(DiffScope::Untracked))
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        for entry in repo.statuses(Some(&mut options))?.iter() {
            let Some(path) = entry.path() else { continue };
            let status = entry.status();
            let scope = if status.intersects(
                Status::INDEX_NEW
                    | Status::INDEX_MODIFIED
                    | Status::INDEX_DELETED
                    | Status::INDEX_RENAMED
                    | Status::INDEX_TYPECHANGE,
            ) {
                DiffScope::Staged
            } else if status.contains(Status::WT_NEW) {
                DiffScope::Untracked
            } else {
                DiffScope::Unstaged
            };
            if !wants(scope) {
                continue;
            }
            changed.push(ChangedFile {
                path: path.replace('\\', "/"),
                status: describe(status),
                scope,
            });
        }
    }

    if wants(DiffScope::AgainstRef) {
        let base_ref = request.base_ref.as_deref().ok_or_else(|| {
            CoreError::InvalidSelection("a base reference is required for diff-against-ref".into())
        })?;
        let object = repo
            .revparse_single(base_ref)
            .map_err(|_| CoreError::Git(format!("unknown git reference `{base_ref}`")))?;
        let tree = object.peel_to_commit()?.tree()?;
        let diff = repo.diff_tree_to_workdir_with_index(Some(&tree), None)?;
        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                    changed.push(ChangedFile {
                        path: project::normalize_separators(path),
                        status: format!("{:?}", delta.status()).to_lowercase(),
                        scope: DiffScope::AgainstRef,
                    });
                }
                true
            },
            None,
            None,
            None,
        )?;
    }

    changed.sort_by(|a, b| a.path.cmp(&b.path));
    changed.dedup_by(|a, b| a.path == b.path);
    Ok(changed)
}

fn describe(status: Status) -> String {
    let mut parts = Vec::new();
    if status.intersects(Status::INDEX_NEW | Status::WT_NEW) {
        parts.push("added");
    }
    if status.intersects(Status::INDEX_MODIFIED | Status::WT_MODIFIED) {
        parts.push("modified");
    }
    if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
        parts.push("deleted");
    }
    if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
        parts.push("renamed");
    }
    if parts.is_empty() {
        parts.push("changed");
    }
    parts.join("+")
}
