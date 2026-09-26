use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::tokenizer::EstimateKind;

/// Declares caching policy supported by the model or provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CachedTokenPolicy {
    None,
    PromptPrefix,
    Automatic,
}

/// Capability profile for a specific model (ADR 0008).
///
/// Providers declare this profile honestly. LeanAI never assumes feature parity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProfile {
    pub context_cap: usize,
    pub streaming: bool,
    pub tool_calling: bool,
    pub structured_output: bool,
    pub vision: bool,
    pub exact_token_counting: bool,
    pub cached_token_policy: CachedTokenPolicy,
}

impl Default for CapabilityProfile {
    fn default() -> Self {
        Self {
            context_cap: 8192,
            streaming: true,
            tool_calling: false,
            structured_output: false,
            vision: false,
            exact_token_counting: false,
            cached_token_policy: CachedTokenPolicy::None,
        }
    }
}

/// Provider category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    LocalSidecar,
    CloudOpenAi,
    CloudAnthropic,
    Mock,
}

/// Operational status of a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Registered,
    Ready,
    Active,
    Error,
}

/// Metadata descriptor for a known model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDescriptor {
    pub id: String,
    pub name: String,
    pub provider_kind: ProviderKind,
    pub version: String,
    pub capabilities: CapabilityProfile,
    pub status: ModelStatus,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionRequest {
    pub prompt: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<usize>,
    pub requires_tool_calling: bool,
    pub requires_structured_output: bool,
    pub requires_vision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionResponse {
    pub text: String,
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub cached_tokens: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCountResult {
    pub count: usize,
    pub estimate_kind: EstimateKind,
}

/// Validates that a model's profile satisfies the requirements of a request.
/// Fails BEFORE any network or process call (ADR 0008).
pub fn validate_request_capabilities(
    profile: &CapabilityProfile,
    request: &CompletionRequest,
    estimated_prompt_tokens: usize,
) -> Result<()> {
    if request.requires_tool_calling && !profile.tool_calling {
        return Err(CoreError::MissingCapability("tool_calling".to_string()));
    }
    if request.requires_structured_output && !profile.structured_output {
        return Err(CoreError::MissingCapability(
            "structured_output".to_string(),
        ));
    }
    if request.requires_vision && !profile.vision {
        return Err(CoreError::MissingCapability("vision".to_string()));
    }
    if estimated_prompt_tokens > profile.context_cap {
        return Err(CoreError::LimitExceeded(format!(
            "Prompt size ({} tokens) exceeds model context cap ({} tokens)",
            estimated_prompt_tokens, profile.context_cap
        )));
    }
    Ok(())
}

/// Trait defining a model provider boundary.
pub trait Provider: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn kind(&self) -> ProviderKind;
    fn capability_profile(&self) -> &CapabilityProfile;
    fn count_tokens(&self, text: &str) -> Result<TokenCountResult>;
    fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse>;
}

/// Mock provider for deterministic tests and offline contract assertions.
#[derive(Debug, Clone)]
pub struct MockProvider {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub profile: CapabilityProfile,
}

impl MockProvider {
    pub fn new(id: impl Into<String>, name: impl Into<String>, profile: CapabilityProfile) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            kind: ProviderKind::Mock,
            profile,
        }
    }
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn kind(&self) -> ProviderKind {
        self.kind
    }
    fn capability_profile(&self) -> &CapabilityProfile {
        &self.profile
    }
    fn count_tokens(&self, text: &str) -> Result<TokenCountResult> {
        if self.profile.exact_token_counting {
            Ok(TokenCountResult {
                count: text.split_whitespace().count(),
                estimate_kind: EstimateKind::ProviderExact {
                    provider: self.id.clone(),
                    model: self.name.clone(),
                },
            })
        } else {
            let bpe_estimate = crate::tokenizer::estimate(text);
            Ok(TokenCountResult {
                count: bpe_estimate.value,
                estimate_kind: bpe_estimate.kind,
            })
        }
    }
    fn complete(&self, request: &CompletionRequest) -> Result<CompletionResponse> {
        let est = crate::tokenizer::estimate(&request.prompt).value;
        validate_request_capabilities(&self.profile, request, est)?;
        Ok(CompletionResponse {
            text: format!("Mock completion for: {}", request.prompt),
            prompt_tokens: est,
            completion_tokens: 12,
            cached_tokens: if self.profile.cached_token_policy != CachedTokenPolicy::None {
                Some(10)
            } else {
                None
            },
        })
    }
}
