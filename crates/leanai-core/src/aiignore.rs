use std::path::Path;

use ignore::gitignore::GitignoreBuilder;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::inventory::Inventory;

/// LeanAI's own ignore file. Uses gitignore syntax and is layered *above*
/// `.gitignore` but *below* the built-in safety policy, so it can hide extra
/// files but can never re-expose a credential-sensitive path.
pub const FILE_NAME: &str = ".aiignore";

pub const TEMPLATE: &str = "\
# .aiignore - extra exclusions for LeanAI bundles.
#
# Syntax matches .gitignore. These rules apply on top of your .gitignore;
# they cannot re-include files blocked by LeanAI's safety policy
# (credentials, binaries, oversized files).
#
# Examples:
# docs/generated/
# *.snap
# !docs/generated/api.md
";

/// The effect a candidate `.aiignore` would have, shown before it is written
/// to disk (backlog 4.2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiIgnorePreview {
    /// Files currently selectable that the rules would exclude.
    pub newly_excluded: Vec<String>,
    /// Rule lines that matched nothing. Usually a typo.
    pub unmatched_rules: Vec<String>,
    /// Rule lines the parser rejected.
    pub invalid_rules: Vec<String>,
    pub selectable_before: usize,
    pub selectable_after: usize,
}

/// Evaluates candidate `.aiignore` content against a scanned inventory without
/// writing anything.
pub fn preview(root: &Path, contents: &str, inventory: &Inventory) -> Result<AiIgnorePreview> {
    let mut builder = GitignoreBuilder::new(root);
    let mut invalid_rules = Vec::new();
    let mut rules = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match builder.add_line(None, trimmed) {
            Ok(_) => rules.push(trimmed.to_string()),
            Err(error) => invalid_rules.push(format!("{trimmed} — {error}")),
        }
    }

    let matcher = builder
        .build()
        .map_err(|error| crate::error::CoreError::Io(error.to_string()))?;

    let mut preview = AiIgnorePreview {
        invalid_rules,
        selectable_before: inventory.selectable().count(),
        ..Default::default()
    };
    let mut matched_rules = vec![false; rules.len()];

    for entry in inventory.selectable() {
        let absolute = root.join(&entry.path);
        if matcher
            .matched_path_or_any_parents(&absolute, false)
            .is_ignore()
        {
            preview.newly_excluded.push(entry.path.clone());
            // Attribute the match to the first rule that hits this path on its
            // own, so "unmatched rules" stays honest for simple rule sets.
            for (index, rule) in rules.iter().enumerate() {
                if matched_rules[index] {
                    continue;
                }
                if rule_matches(root, rule, &entry.path) {
                    matched_rules[index] = true;
                }
            }
        }
    }

    preview.unmatched_rules = rules
        .into_iter()
        .zip(matched_rules)
        .filter(|(rule, matched)| !matched && !rule.starts_with('!'))
        .map(|(rule, _)| rule)
        .collect();
    preview.selectable_after = preview.selectable_before - preview.newly_excluded.len();
    Ok(preview)
}

fn rule_matches(root: &Path, rule: &str, relative: &str) -> bool {
    let mut builder = GitignoreBuilder::new(root);
    if builder.add_line(None, rule).is_err() {
        return false;
    }
    let Ok(matcher) = builder.build() else {
        return false;
    };
    matcher
        .matched_path_or_any_parents(root.join(relative), false)
        .is_ignore()
}
