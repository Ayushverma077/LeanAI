//! Automatic project-local storage for the generated context document.
//!
//! LeanAI writes `PROJECT_CONTEXT.md` only when doing so cannot destroy
//! anything: the file is missing, or it is exactly what LeanAI last wrote. A
//! file someone else wrote, or edited since, is left alone unless the user
//! explicitly asks to replace it, and the caller is told why (ADR 0016).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::context::{DOCUMENT_NAME, FILE_MARKER};
use crate::{CoreError, Result};

/// What is at `PROJECT_CONTEXT.md` in the project, compared with what LeanAI
/// last wrote there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// No file yet.
    Missing,
    /// Exactly what LeanAI last wrote.
    Current,
    /// Written by LeanAI but changed since, by hand or by another copy of
    /// LeanAI.
    Edited,
    /// Not written by LeanAI.
    Foreign,
    /// Tracked by Git. Ignore rules cannot hide a tracked file, so LeanAI
    /// never writes it.
    TrackedByGit,
    /// A directory or symbolic link; LeanAI never writes through it.
    NotAFile,
    /// Present but could not be read, so it cannot be checked.
    Unreadable,
}

impl FileState {
    /// Whether LeanAI may write the file without asking.
    pub fn writable_without_asking(self) -> bool {
        matches!(self, FileState::Missing | FileState::Current)
    }

    /// Whether the user may choose to replace the file.
    pub fn replaceable(self) -> bool {
        matches!(self, FileState::Edited | FileState::Foreign)
    }

    /// What to tell the user when LeanAI left the file alone.
    pub fn alert(self) -> Option<&'static str> {
        match self {
            FileState::Missing | FileState::Current => None,
            FileState::Edited => Some(
                "PROJECT_CONTEXT.md in this project is not the one LeanAI last wrote: it was edited, or written by another copy of LeanAI. LeanAI left it as it is and keeps the new map inside the app.",
            ),
            FileState::Foreign => Some(
                "This project already has a PROJECT_CONTEXT.md that LeanAI did not create. LeanAI left it as it is and keeps its own map inside the app.",
            ),
            FileState::TrackedByGit => Some(
                "PROJECT_CONTEXT.md is tracked by Git, so LeanAI will not write it. The map is kept inside the app. To let LeanAI manage the file, stop tracking it with `git rm --cached PROJECT_CONTEXT.md`.",
            ),
            FileState::NotAFile => Some(
                "PROJECT_CONTEXT.md in this project is a folder or a link, so LeanAI will not write it. The map is kept inside the app.",
            ),
            FileState::Unreadable => Some(
                "PROJECT_CONTEXT.md in this project could not be read, so LeanAI will not write it. The map is kept inside the app.",
            ),
        }
    }
}

/// Result of [`save_to_project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveOutcome {
    Written(PathBuf),
    /// Nothing was written; the state says why.
    Kept(FileState),
}

/// Checks `PROJECT_CONTEXT.md` in `root`. `last_written` is the SHA-256 of
/// the text LeanAI last wrote there, when known.
pub fn file_state(root: &Path, last_written: Option<&str>) -> Result<FileState> {
    let root = crate::project::canonical_root(root)?;
    let target = root.join(DOCUMENT_NAME);
    let exists = match std::fs::symlink_metadata(&target) {
        Ok(metadata) if !metadata.file_type().is_file() => return Ok(FileState::NotAFile),
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(_) => return Ok(FileState::Unreadable),
    };
    if tracked_by_git(&root, &target)? {
        return Ok(FileState::TrackedByGit);
    }
    if !exists {
        return Ok(FileState::Missing);
    }
    let Ok(bytes) = std::fs::read(&target) else {
        return Ok(FileState::Unreadable);
    };
    if last_written.is_some_and(|hash| crate::project::sha256_hex(&bytes) == hash) {
        return Ok(FileState::Current);
    }
    Ok(if String::from_utf8_lossy(&bytes).contains(FILE_MARKER) {
        FileState::Edited
    } else {
        FileState::Foreign
    })
}

fn tracked_by_git(root: &Path, target: &Path) -> Result<bool> {
    match git2::Repository::discover(root) {
        Ok(repo) => {
            let Some(workdir) = repo.workdir() else {
                return Ok(false);
            };
            // Match the canonical project root, including Windows' verbatim
            // path prefix and symlink resolution. The target may not exist yet.
            let workdir = crate::project::canonical_root(workdir)?;
            let relative = target.strip_prefix(workdir).map_err(|_| {
                CoreError::Git("The context file is outside the Git working directory.".into())
            })?;
            let in_index = repo.index()?.get_path(relative, 0).is_some();
            let in_head = repo
                .head()
                .and_then(|head| head.peel_to_tree())
                .is_ok_and(|tree| tree.get_path(relative).is_ok());
            Ok(in_index || in_head)
        }
        Err(error) if error.code() == git2::ErrorCode::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

/// Writes `markdown` to `PROJECT_CONTEXT.md` after installing the Git
/// exclusion, when that cannot destroy anything (see [`FileState`]).
///
/// `last_written` is the SHA-256 of what LeanAI last wrote, so an unchanged
/// LeanAI file is updated silently. `replace` is the user's explicit choice to
/// overwrite an edited or foreign file; a tracked file, a folder or a link is
/// never written.
pub fn save_to_project(
    root: &Path,
    markdown: &str,
    last_written: Option<&str>,
    replace: bool,
) -> Result<SaveOutcome> {
    let root = crate::project::canonical_root(root)?;
    let target = root.join(DOCUMENT_NAME);
    let ignore_path = root.join(".gitignore");
    match std::fs::symlink_metadata(&ignore_path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(CoreError::Io(
                "Cannot save context: .gitignore must be a regular file, not a directory or symbolic link."
                    .to_string(),
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let state = file_state(&root, last_written)?;
    // A file that already holds exactly this text has nothing to lose, even
    // when LeanAI's record of what it last wrote is missing or behind.
    let identical = state.replaceable()
        && std::fs::read(&target).is_ok_and(|bytes| bytes == markdown.as_bytes());
    if !(state.writable_without_asking() || identical || (replace && state.replaceable())) {
        return Ok(SaveOutcome::Kept(state));
    }

    let mut ignore = match std::fs::read_to_string(&ignore_path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let rule = format!("/{DOCUMENT_NAME}");
    // Put the rule last so earlier negations cannot re-include the document.
    if ignore.lines().last() != Some(rule.as_str()) {
        if !ignore.is_empty() && !ignore.ends_with('\n') {
            ignore.push('\n');
        }
        ignore.push_str(&rule);
        ignore.push('\n');
        std::fs::write(&ignore_path, ignore)?;
    }
    std::fs::write(&target, markdown)?;
    Ok(SaveOutcome::Written(target))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::sha256_hex;

    const OURS: &str = "# Project Context\n\n<!-- leanai.context/v2 revision=scan:x -->\n";

    fn written(outcome: SaveOutcome) -> PathBuf {
        match outcome {
            SaveOutcome::Written(path) => path,
            other => panic!("expected a write, got {other:?}"),
        }
    }

    #[test]
    fn saves_and_excludes_context_while_preserving_existing_rules() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let original = "node_modules/\n/PROJECT_CONTEXT.md\n!*.md";
        std::fs::write(dir.path().join(".gitignore"), original).unwrap();

        let target = written(save_to_project(dir.path(), OURS, None, false).unwrap());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), OURS);
        assert!(repo.status_should_ignore(Path::new(DOCUMENT_NAME)).unwrap());
        let ignore = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(ignore, format!("{original}\n/{DOCUMENT_NAME}\n"));

        // LeanAI's own unchanged file is updated without asking.
        let updated = format!("{OURS}\n## Overview\n");
        written(
            save_to_project(
                dir.path(),
                &updated,
                Some(&sha256_hex(OURS.as_bytes())),
                false,
            )
            .unwrap(),
        );
        assert_eq!(std::fs::read_to_string(target).unwrap(), updated);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            ignore
        );
    }

    #[test]
    fn creates_ignore_file_in_projects_without_git() {
        let dir = tempfile::tempdir().unwrap();
        written(save_to_project(dir.path(), OURS, None, false).unwrap());
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "/PROJECT_CONTEXT.md\n"
        );
    }

    #[test]
    fn protects_tracked_context_in_a_nested_project() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let project = dir.path().join("nested");
        std::fs::create_dir(&project).unwrap();
        let target = project.join(DOCUMENT_NAME);

        // The destination does not need to exist to check Git tracking.
        assert_eq!(file_state(&project, None).unwrap(), FileState::Missing);
        std::fs::write(&target, "tracked content").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(&Path::new("nested").join(DOCUMENT_NAME))
            .unwrap();
        index.write().unwrap();

        // Exercise the canonical root returned to callers on every platform.
        let root = crate::project::canonical_root(&project).unwrap();
        assert_eq!(
            save_to_project(&root, OURS, None, true).unwrap(),
            SaveOutcome::Kept(FileState::TrackedByGit)
        );
        assert_eq!(std::fs::read_to_string(target).unwrap(), "tracked content");
        assert!(!project.join(".gitignore").exists());
    }

    #[test]
    fn keeps_a_file_leanai_did_not_write_unless_asked() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(DOCUMENT_NAME);
        std::fs::write(&target, "# Our own notes\n").unwrap();

        assert_eq!(file_state(dir.path(), None).unwrap(), FileState::Foreign);
        assert_eq!(
            save_to_project(dir.path(), OURS, None, false).unwrap(),
            SaveOutcome::Kept(FileState::Foreign)
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "# Our own notes\n"
        );
        assert!(
            !dir.path().join(".gitignore").exists(),
            "nothing is touched when the file is kept"
        );

        // The user's explicit choice replaces it.
        written(save_to_project(dir.path(), OURS, None, true).unwrap());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), OURS);

        // Writing the same text again is safe even without a record of it,
        // as when two builds finish one after the other.
        written(save_to_project(dir.path(), OURS, None, false).unwrap());
    }

    #[test]
    fn keeps_a_leanai_file_that_was_edited_since() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join(DOCUMENT_NAME);
        std::fs::write(&target, format!("{OURS}\nmy edit\n")).unwrap();
        let last = sha256_hex(OURS.as_bytes());

        assert_eq!(
            file_state(dir.path(), Some(&last)).unwrap(),
            FileState::Edited
        );
        assert_eq!(
            save_to_project(dir.path(), OURS, Some(&last), false).unwrap(),
            SaveOutcome::Kept(FileState::Edited)
        );
        assert!(std::fs::read_to_string(&target)
            .unwrap()
            .contains("my edit"));
        assert!(FileState::Edited.replaceable());
        assert!(FileState::Edited
            .alert()
            .unwrap()
            .contains("not the one LeanAI last wrote"));
    }

    #[test]
    fn never_writes_a_tracked_context_file() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let target = dir.path().join(DOCUMENT_NAME);
        std::fs::write(&target, "tracked content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(DOCUMENT_NAME)).unwrap();
        index.write().unwrap();

        for replace in [false, true] {
            assert_eq!(
                save_to_project(dir.path(), "private content", None, replace).unwrap(),
                SaveOutcome::Kept(FileState::TrackedByGit)
            );
        }
        assert_eq!(std::fs::read_to_string(target).unwrap(), "tracked content");
        assert!(!dir.path().join(".gitignore").exists());
        assert!(!FileState::TrackedByGit.replaceable());
        assert!(FileState::TrackedByGit
            .alert()
            .unwrap()
            .contains("git rm --cached"));
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlink_destinations_including_dangling_links() {
        // The context file itself: kept, never written through.
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let destination = outside.path().join("untouched");
        std::os::unix::fs::symlink(&destination, dir.path().join(DOCUMENT_NAME)).unwrap();
        assert_eq!(
            save_to_project(dir.path(), OURS, None, true).unwrap(),
            SaveOutcome::Kept(FileState::NotAFile)
        );
        assert!(!destination.exists());

        // The ignore file: an error, since excluding the document is required.
        let dir = tempfile::tempdir().unwrap();
        let destination = outside.path().join("untouched-ignore");
        std::os::unix::fs::symlink(&destination, dir.path().join(".gitignore")).unwrap();
        assert!(save_to_project(dir.path(), OURS, None, false).is_err());
        assert!(!destination.exists());
    }
}
