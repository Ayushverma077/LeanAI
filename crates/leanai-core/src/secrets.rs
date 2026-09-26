use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Confidence in a secret finding. Nothing here is proof: this scanner is a
/// review aid, never a security boundary (NFR 7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Matches a documented provider token format.
    High,
    /// Matches a generic assignment pattern that is often a real credential.
    Medium,
    /// Weak signal; frequently a false positive.
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretFinding {
    pub path: String,
    pub line: usize,
    pub rule: String,
    pub confidence: Confidence,
    /// A redacted excerpt. The matched secret is never stored or displayed in
    /// full, and never written to history or telemetry.
    pub redacted_excerpt: String,
    /// Set when the user has dismissed this finding as a false positive.
    pub dismissed: bool,
}

struct Rule {
    name: &'static str,
    confidence: Confidence,
    pattern: Regex,
}

static RULES: Lazy<Vec<Rule>> = Lazy::new(|| {
    let definitions: &[(&str, Confidence, &str)] = &[
        (
            "aws-access-key-id",
            Confidence::High,
            r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
        ),
        (
            "github-token",
            Confidence::High,
            r"\bgh[pousr]_[A-Za-z0-9]{36,}\b",
        ),
        (
            "slack-token",
            Confidence::High,
            r"\bxox[abprs]-[A-Za-z0-9-]{10,}\b",
        ),
        (
            "stripe-key",
            Confidence::High,
            r"\b[sr]k_(?:live|test)_[A-Za-z0-9]{16,}\b",
        ),
        (
            "google-api-key",
            Confidence::High,
            r"\bAIza[0-9A-Za-z_\-]{35}\b",
        ),
        (
            "openai-key",
            Confidence::High,
            r"\bsk-(?:proj-)?[A-Za-z0-9_\-]{20,}\b",
        ),
        (
            "anthropic-key",
            Confidence::High,
            r"\bsk-ant-[A-Za-z0-9_\-]{20,}\b",
        ),
        (
            "private-key-block",
            Confidence::High,
            r"-----BEGIN (?:RSA |EC |OPENSSH |PGP |DSA )?PRIVATE KEY-----",
        ),
        (
            "jwt",
            Confidence::Medium,
            r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b",
        ),
        (
            "connection-string",
            Confidence::Medium,
            r"(?i)\b(?:postgres|postgresql|mysql|mongodb(?:\+srv)?|redis|amqp)://[^\s:@/]+:[^\s:@/]+@",
        ),
        (
            "assigned-credential",
            Confidence::Medium,
            r#"(?i)\b(?:api[_-]?key|secret|password|passwd|token|access[_-]?key|client[_-]?secret)\b\s*[:=]\s*['"]?[A-Za-z0-9_\-/+]{16,}['"]?"#,
        ),
        (
            "bearer-header",
            Confidence::Low,
            r"(?i)\bauthorization\s*[:=]\s*['\x22]?bearer\s+[A-Za-z0-9._\-]{16,}",
        ),
        (
            "high-entropy-assignment",
            Confidence::Low,
            r#"(?i)\b\w*(?:key|secret|token)\w*\s*[:=]\s*['"][A-Za-z0-9+/=]{32,}['"]"#,
        ),
    ];
    definitions
        .iter()
        .filter_map(|(name, confidence, pattern)| {
            Regex::new(pattern).ok().map(|pattern| Rule {
                name,
                confidence: *confidence,
                pattern,
            })
        })
        .collect()
});

/// Scans one file's text. Returns at most `max_findings` results per file so a
/// generated fixture cannot flood the review screen.
pub fn scan_text(path: &str, text: &str, max_findings: usize) -> Vec<SecretFinding> {
    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.len() > 4_096 {
            continue; // Minified or data lines: too noisy to be useful.
        }
        for rule in RULES.iter() {
            if let Some(matched) = rule.pattern.find(line) {
                findings.push(SecretFinding {
                    path: path.to_string(),
                    line: index + 1,
                    rule: rule.name.to_string(),
                    confidence: rule.confidence,
                    redacted_excerpt: redact(line, matched.start(), matched.end()),
                    dismissed: false,
                });
                if findings.len() >= max_findings {
                    return findings;
                }
            }
        }
    }
    findings
}

/// Replaces the matched span with a length-preserving marker and trims the
/// surrounding context, so the finding is reviewable without revealing the
/// value.
fn redact(line: &str, start: usize, end: usize) -> String {
    let prefix: String = line[..start]
        .chars()
        .rev()
        .take(24)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let suffix: String = line[end..].chars().take(12).collect();
    let matched = &line[start..end];
    let head: String = matched.chars().take(4).collect();
    format!(
        "{}{}…[redacted {} chars]{}",
        prefix.trim_start(),
        head,
        matched.chars().count().saturating_sub(4),
        suffix
    )
}

/// Aggregated view used by the export preflight screen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretReport {
    pub findings: Vec<SecretFinding>,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    /// Files that were not scanned (too large, unreadable).
    pub skipped: Vec<String>,
}

impl SecretReport {
    pub fn push(&mut self, findings: Vec<SecretFinding>) {
        for finding in findings {
            match finding.confidence {
                Confidence::High => self.high += 1,
                Confidence::Medium => self.medium += 1,
                Confidence::Low => self.low += 1,
            }
            self.findings.push(finding);
        }
    }

    /// A blocking result requires an explicit, high-friction confirmation
    /// before export. It is *not* a guarantee that the remainder is clean.
    pub fn requires_confirmation(&self) -> bool {
        self.high > 0 || self.medium > 0
    }

    pub const DISCLAIMER: &'static str = "Heuristic scan. It can miss real secrets and flag harmless strings. Review the selected files yourself before sending them anywhere.";
}
