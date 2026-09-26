use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tiktoken_rs::{cl100k_base, CoreBPE};

/// What produced a token number. This label travels with every estimate all the
/// way to the UI, because an OpenAI-family BPE count is *not* an Anthropic,
/// Gemini or GGUF count (scope decision in Instructions.md §2, FR-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EstimateKind {
    /// Local `cl100k_base` count. Accurate for OpenAI models; indicative only
    /// for every other family.
    OpenAiFamilyEstimate { tokenizer: String },
    /// Returned by a provider's own count endpoint, with consent.
    ProviderExact { provider: String, model: String },
    /// No estimate could be produced.
    Unavailable { reason: String },
}

impl EstimateKind {
    pub fn local() -> Self {
        EstimateKind::OpenAiFamilyEstimate {
            tokenizer: "cl100k_base".to_string(),
        }
    }

    /// Short label rendered next to every number in the UI.
    pub fn label(&self) -> String {
        match self {
            EstimateKind::OpenAiFamilyEstimate { tokenizer } => {
                format!("estimate · {tokenizer} · OpenAI-family")
            }
            EstimateKind::ProviderExact { provider, model } => {
                format!("exact · {provider}/{model}")
            }
            EstimateKind::Unavailable { .. } => "unavailable".to_string(),
        }
    }

    pub fn is_exact(&self) -> bool {
        matches!(self, EstimateKind::ProviderExact { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenEstimate {
    pub value: usize,
    #[serde(flatten)]
    pub kind: EstimateKind,
}

impl TokenEstimate {
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            value: 0,
            kind: EstimateKind::Unavailable {
                reason: reason.into(),
            },
        }
    }
}

static BPE: Lazy<Option<CoreBPE>> = Lazy::new(|| cl100k_base().ok());

/// Counts tokens locally with `cl100k_base`.
pub fn estimate(text: &str) -> TokenEstimate {
    match BPE.as_ref() {
        Some(bpe) => TokenEstimate {
            value: bpe.encode_ordinary(text).len(),
            kind: EstimateKind::local(),
        },
        None => TokenEstimate::unavailable("the local tokenizer could not be initialised"),
    }
}

/// Per-file contribution, used by the bundle preview to show what dominates a
/// context budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContribution {
    pub path: String,
    pub bytes: u64,
    pub tokens: usize,
    /// Share of the whole bundle, 0.0-1.0.
    pub share: f64,
}

pub fn contributions(parts: &[(String, u64, usize)], total_tokens: usize) -> Vec<FileContribution> {
    parts
        .iter()
        .map(|(path, bytes, tokens)| FileContribution {
            path: path.clone(),
            bytes: *bytes,
            tokens: *tokens,
            share: if total_tokens == 0 {
                0.0
            } else {
                *tokens as f64 / total_tokens as f64
            },
        })
        .collect()
}
