use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::concat::{BundleOptions, TruncationWarning};
use crate::inventory::Inventory;
use crate::selection::ResolvedSelection;
use crate::tokenizer::{EstimateKind, TokenEstimate};

pub const BUNDLE_FORMAT: &str = "leanai.bundle/v1";

/// The companion manifest for a saved or exported bundle (Instructions §8.2).
///
/// Contains no absolute paths and no file contents, so it can be attached to a
/// support ticket without leaking the user's machine layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleManifest {
    pub format: String,
    pub project_fingerprint: String,
    pub source_revision: String,
    pub policy_version: u32,
    pub created_at_ms: u64,
    pub selection: SelectionManifest,
    pub options: BundleOptions,
    pub token_estimate: TokenEstimateManifest,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionManifest {
    pub included_files: Vec<String>,
    pub excluded_files: Vec<String>,
    /// Path → reason. Only excluded paths the user asked for are listed; the
    /// full exclusion set lives in the inventory.
    pub exclusion_reasons: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenEstimateManifest {
    pub value: usize,
    /// `openai_family_estimate` | `provider_exact` | `unavailable`
    pub kind: String,
    pub tokenizer_or_provider: String,
}

impl From<&TokenEstimate> for TokenEstimateManifest {
    fn from(estimate: &TokenEstimate) -> Self {
        let (kind, source) = match &estimate.kind {
            EstimateKind::OpenAiFamilyEstimate { tokenizer } => {
                ("openai_family_estimate", tokenizer.clone())
            }
            EstimateKind::ProviderExact { provider, model } => {
                ("provider_exact", format!("{provider}/{model}"))
            }
            EstimateKind::Unavailable { reason } => ("unavailable", reason.clone()),
        };
        Self {
            value: estimate.value,
            kind: kind.to_string(),
            tokenizer_or_provider: source,
        }
    }
}

impl BundleManifest {
    pub fn new(
        inventory: &Inventory,
        selection: &ResolvedSelection,
        options: &BundleOptions,
        estimate: &TokenEstimate,
        truncations: &[TruncationWarning],
        skipped: &[String],
    ) -> Self {
        let mut exclusion_reasons = BTreeMap::new();
        let mut excluded_files = Vec::new();
        for rejected in &selection.rejected {
            exclusion_reasons.insert(rejected.path.clone(), rejected.reason.clone());
            excluded_files.push(rejected.path.clone());
        }

        let mut warnings = Vec::new();
        for truncation in truncations {
            warnings.push(format!(
                "{} was truncated from {} to {} bytes",
                truncation.path, truncation.original_bytes, truncation.included_bytes
            ));
        }
        for path in skipped {
            warnings.push(format!("{path} was skipped: unreadable at build time"));
        }
        for path in &selection.missing {
            warnings.push(format!("{path} is in the selection but no longer exists"));
        }
        if inventory.stats.truncated {
            warnings.push("the scan hit the file limit and is incomplete".to_string());
        }
        if !matches!(estimate.kind, EstimateKind::ProviderExact { .. }) {
            warnings.push(
                "token value is a local OpenAI-family estimate, not a provider count".to_string(),
            );
        }

        Self {
            format: BUNDLE_FORMAT.to_string(),
            project_fingerprint: inventory.project_fingerprint.clone(),
            source_revision: inventory.source_revision.clone(),
            policy_version: inventory.policy_version,
            created_at_ms: crate::project::now_ms(),
            selection: SelectionManifest {
                included_files: selection.files.clone(),
                excluded_files,
                exclusion_reasons,
            },
            options: options.clone(),
            token_estimate: TokenEstimateManifest::from(estimate),
            warnings,
        }
    }

    /// Deterministic YAML front matter matching the schema in Instructions §8.2.
    pub fn to_front_matter(&self) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("format: {}\n", self.format));
        out.push_str(&format!(
            "project_fingerprint: {}\n",
            self.project_fingerprint
        ));
        out.push_str(&format!("source_revision: {}\n", self.source_revision));
        out.push_str(&format!("policy_version: {}\n", self.policy_version));
        out.push_str("selection:\n  included_files:\n");
        for path in &self.selection.included_files {
            out.push_str(&format!("    - {}\n", yaml_scalar(path)));
        }
        out.push_str("  excluded_files:\n");
        for path in &self.selection.excluded_files {
            out.push_str(&format!("    - {}\n", yaml_scalar(path)));
        }
        out.push_str("  exclusion_reasons:\n");
        for (path, reason) in &self.selection.exclusion_reasons {
            out.push_str(&format!(
                "    {}: {}\n",
                yaml_scalar(path),
                yaml_scalar(reason)
            ));
        }
        out.push_str("options:\n");
        out.push_str(&format!("  headers: {}\n", self.options.headers));
        out.push_str(&format!("  line_numbers: {}\n", self.options.line_numbers));
        out.push_str(&format!("  code_fences: {}\n", self.options.code_fences));
        out.push_str(&format!(
            "  file_size_annotations: {}\n",
            self.options.file_size_annotations
        ));
        out.push_str(&format!("  include_tree: {}\n", self.options.include_tree));
        out.push_str(&format!(
            "  normalize_line_endings: {}\n",
            self.options.normalize_line_endings
        ));
        out.push_str("token_estimate:\n");
        out.push_str(&format!("  value: {}\n", self.token_estimate.value));
        out.push_str(&format!("  kind: {}\n", self.token_estimate.kind));
        out.push_str(&format!(
            "  tokenizer_or_provider: {}\n",
            yaml_scalar(&self.token_estimate.tokenizer_or_provider)
        ));
        out.push_str("warnings:\n");
        for warning in &self.warnings {
            out.push_str(&format!("  - {}\n", yaml_scalar(warning)));
        }
        out.push_str("---\n");
        out
    }
}

/// Quotes a YAML scalar when it could otherwise be misparsed.
fn yaml_scalar(value: &str) -> String {
    let needs_quotes = value.is_empty()
        || value.contains([
            ':', '#', '\n', '"', '\'', '{', '}', '[', ']', ',', '&', '*', '?', '|', '>', '%', '@',
            '`',
        ])
        || value.starts_with(' ')
        || value.ends_with(' ')
        || value.starts_with('-');
    if needs_quotes {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value.to_string()
    }
}
