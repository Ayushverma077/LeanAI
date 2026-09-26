use serde::{Deserialize, Serialize};

use crate::classify::FileClass;
use crate::error::{CoreError, Result};
use crate::inventory::Inventory;
use crate::policy::Policy;

/// A stored, replayable description of what the user chose.
///
/// Presets persist this rather than a resolved file list, so a preset stays
/// meaningful after files are added or removed — and every resolution is
/// re-checked against the current safety policy (backlog 4.1).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSpec {
    /// Explicitly selected files, relative to the project root.
    pub files: Vec<String>,
    /// Selected directories. Expands to every selectable file beneath them.
    pub directories: Vec<String>,
    /// Files the user removed after a directory selection added them.
    pub excluded: Vec<String>,
    /// Per-file confirmations that unlock a class which is blocked by default
    /// (a `.env`, a binary). Never applied by directory selection (FR-08).
    pub overrides: Vec<String>,
}

impl SelectionSpec {
    pub fn from_files(files: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            files: files.into_iter().map(Into::into).collect(),
            ..Default::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.directories.is_empty()
    }
}

/// A path that was requested but not included, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RejectedPath {
    pub path: String,
    pub reason: String,
    /// True when a per-file override would allow this file in.
    pub overridable: bool,
}

/// The resolved, ordered result of applying a `SelectionSpec` to an inventory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSelection {
    /// Included files in deterministic (lexicographic) order.
    pub files: Vec<String>,
    pub rejected: Vec<RejectedPath>,
    /// Paths in the spec that no longer exist in the project.
    pub missing: Vec<String>,
    pub total_bytes: u64,
}

impl ResolvedSelection {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Resolves a selection against the current inventory.
///
/// This is the single gate between "what the user clicked" and "what leaves the
/// device". Directory selection can only add files that are selectable under
/// the active policy; unlocking anything else requires the path to appear in
/// `overrides`.
pub fn resolve(
    inventory: &Inventory,
    spec: &SelectionSpec,
    policy: &Policy,
) -> Result<ResolvedSelection> {
    let mut result = ResolvedSelection::default();
    let mut chosen: Vec<&str> = Vec::new();

    let excluded: std::collections::HashSet<&str> =
        spec.excluded.iter().map(String::as_str).collect();
    let overrides: std::collections::HashSet<&str> =
        spec.overrides.iter().map(String::as_str).collect();

    // Directory selection: only files that are selectable by default.
    for directory in &spec.directories {
        let prefix = normalize_dir(directory);
        let mut matched = false;
        for entry in &inventory.files {
            if !entry.path.starts_with(&prefix) {
                continue;
            }
            matched = true;
            if excluded.contains(entry.path.as_str()) {
                continue;
            }
            if entry.selectable {
                chosen.push(&entry.path);
            }
            // Files blocked by policy are silently left out of a folder
            // selection; they are already listed in the exclusions panel with
            // their reason, and surfacing every one here would bury the signal.
        }
        if !matched {
            result.missing.push(directory.clone());
        }
    }

    // Explicit file selection: may use an override.
    for path in &spec.files {
        if excluded.contains(path.as_str()) {
            continue;
        }
        let Some(entry) = inventory.get(path) else {
            result.missing.push(path.clone());
            continue;
        };
        if entry.selectable {
            chosen.push(&entry.path);
            continue;
        }
        let overridable = !matches!(
            entry.class,
            FileClass::Unreadable | FileClass::UnsupportedEncoding
        );
        if overrides.contains(path.as_str()) && overridable {
            chosen.push(&entry.path);
            continue;
        }
        result.rejected.push(RejectedPath {
            path: path.clone(),
            reason: entry
                .exclusion
                .as_ref()
                .map(|exclusion| exclusion.reason.clone())
                .unwrap_or_else(|| format!("Excluded: {}.", entry.class.label())),
            overridable,
        });
    }

    chosen.sort_unstable();
    chosen.dedup();

    if chosen.len() > policy.limits.max_selected_files {
        return Err(CoreError::LimitExceeded(format!(
            "{} files selected; the limit is {}. Narrow the selection or raise the limit in Settings.",
            chosen.len(),
            policy.limits.max_selected_files
        )));
    }

    result.total_bytes = chosen
        .iter()
        .filter_map(|path| inventory.get(path))
        .map(|entry| entry.size_bytes)
        .sum();

    if result.total_bytes > policy.limits.max_bundle_bytes {
        return Err(CoreError::LimitExceeded(format!(
            "Selection is {} which exceeds the {} bundle limit.",
            crate::walker::human_bytes(result.total_bytes),
            crate::walker::human_bytes(policy.limits.max_bundle_bytes)
        )));
    }

    result.files = chosen.into_iter().map(str::to_string).collect();
    result.missing.sort();
    result.missing.dedup();
    result.rejected.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

/// One-click starting points, so opening a repository does not begin with
/// clicking a hundred checkboxes.
///
/// A recipe is a *rule*, evaluated against the current inventory, not a frozen
/// list — the same reasoning as presets. Every recipe only ever proposes files
/// the policy already allows, so none of them can reach a credential path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionRecipe {
    /// Source code: everything selectable, minus tests, docs, examples and
    /// benchmarks. The default starting point for a code task.
    SourceOnly,
    /// Everything the policy allows, including tests and docs.
    Everything,
    /// Only the tests, for when the test suite *is* the task.
    TestsOnly,
}

impl SelectionRecipe {
    pub fn label(&self) -> &'static str {
        match self {
            SelectionRecipe::SourceOnly => "Source code",
            SelectionRecipe::Everything => "Everything",
            SelectionRecipe::TestsOnly => "Tests",
        }
    }

    /// One sentence explaining what this recipe leaves out, shown after it runs
    /// so the selection is never a black box.
    pub fn description(&self) -> &'static str {
        match self {
            SelectionRecipe::SourceOnly => {
                "Source files, leaving out tests, documentation, examples and benchmarks."
            }
            SelectionRecipe::Everything => "Every file the safety policy allows.",
            SelectionRecipe::TestsOnly => "Test files, test data and snapshots.",
        }
    }

    fn accepts(&self, path: &str) -> bool {
        let is_test = crate::classify::is_test_path(path);
        match self {
            SelectionRecipe::SourceOnly => !is_test && !crate::classify::is_docs_path(path),
            SelectionRecipe::Everything => true,
            SelectionRecipe::TestsOnly => is_test,
        }
    }
}

/// What a recipe proposes, and what it deliberately left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedSelection {
    pub recipe: SelectionRecipe,
    pub files: Vec<String>,
    pub total_bytes: u64,
    /// Selectable files the recipe chose not to include.
    pub skipped_by_recipe: usize,
    pub description: &'static str,
}

/// Evaluates a recipe against the current inventory.
pub fn suggest(inventory: &Inventory, recipe: SelectionRecipe) -> SuggestedSelection {
    let mut files = Vec::new();
    let mut total_bytes = 0u64;
    let mut skipped_by_recipe = 0usize;

    for entry in inventory.selectable() {
        if recipe.accepts(&entry.path) {
            total_bytes += entry.size_bytes;
            files.push(entry.path.clone());
        } else {
            skipped_by_recipe += 1;
        }
    }
    files.sort();

    SuggestedSelection {
        recipe,
        files,
        total_bytes,
        skipped_by_recipe,
        description: recipe.description(),
    }
}

/// Every directory that holds at least one selectable file, with its totals.
///
/// Powers a flat folder picker: on a large repository, finding `src/api` by
/// expanding a tree of thousands of files is slower than reading a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySummary {
    pub path: String,
    /// Depth below the project root; 0 for a top-level directory.
    pub depth: usize,
    /// Selectable files directly in this directory and below it.
    pub selectable_files: usize,
    pub total_bytes: u64,
}

pub fn directory_summaries(inventory: &Inventory) -> Vec<DirectorySummary> {
    let mut totals: std::collections::BTreeMap<String, (usize, u64)> =
        std::collections::BTreeMap::new();

    for entry in inventory.selectable() {
        let segments: Vec<&str> = entry.path.split('/').collect();
        // Attribute the file to every ancestor directory, so `src` reports the
        // whole subtree rather than only its immediate children.
        for depth in 1..segments.len() {
            let directory = segments[..depth].join("/");
            let slot = totals.entry(directory).or_insert((0, 0));
            slot.0 += 1;
            slot.1 += entry.size_bytes;
        }
    }

    let mut summaries: Vec<DirectorySummary> = totals
        .into_iter()
        .map(|(path, (selectable_files, total_bytes))| DirectorySummary {
            depth: path.matches('/').count(),
            path,
            selectable_files,
            total_bytes,
        })
        .collect();
    summaries.sort_by(|a, b| a.path.cmp(&b.path));
    summaries
}

/// Selection state of a directory node in the tree UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriState {
    Unchecked,
    Partial,
    Checked,
}

/// Computes the tri-state for a directory given the selected set.
pub fn directory_state(
    inventory: &Inventory,
    directory: &str,
    selected: &std::collections::HashSet<String>,
) -> TriState {
    let prefix = normalize_dir(directory);
    let mut total = 0usize;
    let mut hits = 0usize;
    for entry in inventory.files.iter().filter(|entry| entry.selectable) {
        if !entry.path.starts_with(&prefix) {
            continue;
        }
        total += 1;
        if selected.contains(&entry.path) {
            hits += 1;
        }
    }
    match (total, hits) {
        (0, _) | (_, 0) => TriState::Unchecked,
        (total, hits) if total == hits => TriState::Checked,
        _ => TriState::Partial,
    }
}

fn normalize_dir(directory: &str) -> String {
    let trimmed = directory.trim_end_matches('/');
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}/")
    }
}
