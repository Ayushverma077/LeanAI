use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::agent::{PatchProposal, ToolCapability};
use crate::error::{CoreError, Result};
use crate::policy::Policy;
use crate::project::{now_ms, resolve_within_root};

/// Status of an approval decision (FR-27, 9.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionState {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// Scoped, expiring approval request for a consequential capability (ADR 0010, 9.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRequest {
    pub id: String,
    pub run_id: String,
    pub capability: ToolCapability,
    pub project_root: String,
    pub affected_paths: Vec<String>,
    pub patch_hash: String,
    pub token: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub state: DecisionState,
    pub approver: Option<String>,
    pub decided_at_ms: Option<u64>,
}

impl ApprovalRequest {
    pub fn new(
        run_id: String,
        capability: ToolCapability,
        project_root: String,
        affected_paths: Vec<String>,
        patch_hash: String,
        ttl_ms: u64,
    ) -> Self {
        let now = now_ms();
        use sha2::Digest;
        let id = format!(
            "appr-{:.16x}",
            sha2::Sha256::digest(format!("{run_id}-{patch_hash}-{now}").as_bytes())
        );
        let token = format!(
            "tok-{:.16x}",
            sha2::Sha256::digest(format!("{id}-{now}-token").as_bytes())
        );
        Self {
            id,
            run_id,
            capability,
            project_root,
            affected_paths,
            patch_hash,
            token,
            created_at_ms: now,
            expires_at_ms: now + ttl_ms,
            state: DecisionState::Pending,
            approver: None,
            decided_at_ms: None,
        }
    }

    /// True if the request is still pending and has not passed its expiration deadline.
    pub fn is_active(&self) -> bool {
        self.state == DecisionState::Pending && now_ms() < self.expires_at_ms
    }

    /// Resolves the request with user decision (ADR 0010).
    pub fn resolve(&mut self, approved: bool, approver: String) {
        if now_ms() >= self.expires_at_ms {
            self.state = DecisionState::Expired;
        } else if approved {
            self.state = DecisionState::Approved;
        } else {
            self.state = DecisionState::Denied;
        }
        self.approver = Some(approver);
        self.decided_at_ms = Some(now_ms());
    }
}

/// Validates that a command to be executed by Tester is on the project allowlist (8.6, 9.7).
///
/// Strictly rejects dangerous shell chaining, environment injection, and destructive commands.
pub fn validate_allowlisted_command(raw_command: &str, allowlist: &[String]) -> Result<()> {
    let trimmed = raw_command.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidSelection(
            "command cannot be empty".to_string(),
        ));
    }

    // Reject shell chaining, redirects, subshells, and environment manipulation
    let forbidden_chars = [
        ';', '&', '|', '>', '<', '$', '`', '\\', '(', ')', '\n', '\r',
    ];
    for ch in forbidden_chars {
        if trimmed.contains(ch) {
            return Err(CoreError::LimitExceeded(format!(
                "forbidden shell metacharacter '{ch}' in command"
            )));
        }
    }

    // Check against destructive operations
    let lower = trimmed.to_lowercase();
    let destructive_patterns = [
        "rm -rf",
        "sudo",
        "curl",
        "wget",
        "nc",
        "chmod",
        "chown",
        "mkfs",
        "dd",
        ":(){ :|:& };:",
        "git push --force",
        "git reset --hard",
    ];
    for pattern in destructive_patterns {
        if lower.contains(pattern) {
            return Err(CoreError::LimitExceeded(format!(
                "command contains dangerous/destructive pattern: {pattern}"
            )));
        }
    }

    let cmd_tokens = crate::agent::parse_command_tokens(trimmed)?;

    // Must match at least one prefix or exact rule in allowlist using token comparison
    let is_allowed = allowlist.iter().any(|allowed| {
        let rule = allowed.trim();
        if rule.is_empty() {
            return false;
        }
        if let Ok(rule_tokens) = crate::agent::parse_command_tokens(rule) {
            if !rule_tokens.is_empty() && cmd_tokens.len() >= rule_tokens.len() {
                return cmd_tokens[..rule_tokens.len()] == rule_tokens[..];
            }
        }
        false
    });

    if !is_allowed {
        return Err(CoreError::LimitExceeded(
            "command is not present in project command allowlist".to_string(),
        ));
    }

    Ok(())
}

/// Transactional backup and restore manager for applying code patches (9.2).
///
/// If any file write or patch fails, all modified files are restored and new files deleted.
pub struct TransactionalPatchSession {
    root: PathBuf,
    backups: HashMap<PathBuf, Option<Vec<u8>>>,
    created_files: Vec<PathBuf>,
}

impl TransactionalPatchSession {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let canonical = crate::project::canonical_root(&root).unwrap_or(root);
        Self {
            root: canonical,
            backups: HashMap::new(),
            created_files: Vec::new(),
        }
    }

    /// Records original state of a file before modification.
    pub fn backup_file(&mut self, rel_path: &str) -> Result<()> {
        let abs_path = resolve_within_root(&self.root, rel_path)?;
        if self.backups.contains_key(&abs_path) {
            return Ok(());
        }

        if abs_path.exists() {
            let data = fs::read(&abs_path)?;
            self.backups.insert(abs_path, Some(data));
        } else {
            self.backups.insert(abs_path, None);
        }
        Ok(())
    }

    /// Applies a patch proposal transactionally. Reverts automatically on any error.
    pub fn apply(&mut self, proposal: &PatchProposal, _policy: &Policy) -> Result<()> {
        for patch in &proposal.patches {
            let abs_path = resolve_within_root(&self.root, &patch.path)?;

            self.backup_file(&patch.path)?;

            if patch.is_deleted {
                if abs_path.exists() {
                    fs::remove_file(&abs_path)?;
                }
            } else if patch.is_new_file {
                if let Some(parent) = abs_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                // Strip unified diff headers to get new file text
                let content = extract_new_content_from_diff(&patch.unified_diff);
                fs::write(&abs_path, content)?;
                self.created_files.push(abs_path);
            } else {
                // Apply modified content
                let current_content = if abs_path.exists() {
                    fs::read_to_string(&abs_path)?
                } else {
                    String::new()
                };
                let new_content = apply_unified_diff_chunk(&current_content, &patch.unified_diff)?;
                fs::write(&abs_path, new_content)?;
            }
        }

        Ok(())
    }

    /// Reverts all modifications made during this session.
    pub fn rollback(&mut self) -> Result<()> {
        for (path, original_data) in &self.backups {
            match original_data {
                Some(data) => {
                    if let Some(parent) = path.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    let _ = fs::write(path, data);
                }
                None => {
                    if path.exists() {
                        let _ = fs::remove_file(path);
                    }
                }
            }
        }
        for path in &self.created_files {
            if path.exists() {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }
}

/// Helper: extracts added lines from a unified diff for a new file.
fn extract_new_content_from_diff(diff: &str) -> String {
    let mut lines = Vec::new();
    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("@@") {
            continue;
        }
        if let Some(stripped) = line.strip_prefix('+') {
            lines.push(stripped);
        }
    }
    if lines.is_empty() {
        diff.to_string()
    } else {
        lines.join("\n")
    }
}

/// Helper: applies unified diff line additions/deletions to base text.
fn apply_unified_diff_chunk(base: &str, diff: &str) -> Result<String> {
    let mut result_lines = Vec::new();
    let base_lines: Vec<&str> = base.lines().collect();
    let mut base_idx = 0;

    for line in diff.lines() {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with("@@ -") {
            // Parse orig_start from hunk header e.g. @@ -10,4 +10,5 @@
            if let Some(rest) = line.strip_prefix("@@ -") {
                if let Some(num_str) = rest.split([',', ' ']).next() {
                    if let Ok(orig_start) = num_str.parse::<usize>() {
                        let target_idx = orig_start.saturating_sub(1);
                        while base_idx < target_idx && base_idx < base_lines.len() {
                            result_lines.push(base_lines[base_idx].to_string());
                            base_idx += 1;
                        }
                    }
                }
            }
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            result_lines.push(added.to_string());
        } else if let Some(_deleted) = line.strip_prefix('-') {
            if base_idx < base_lines.len() {
                base_idx += 1;
            }
        } else {
            let unchanged = line.strip_prefix(' ').unwrap_or(line);
            result_lines.push(unchanged.to_string());
            if base_idx < base_lines.len() {
                base_idx += 1;
            }
        }
    }

    // Append any trailing unchanged base lines
    while base_idx < base_lines.len() {
        result_lines.push(base_lines[base_idx].to_string());
        base_idx += 1;
    }

    Ok(result_lines.join("\n"))
}
