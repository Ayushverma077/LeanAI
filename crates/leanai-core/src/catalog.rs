use serde::{Deserialize, Serialize};

use crate::provider::{CachedTokenPolicy, CapabilityProfile};

/// Token pricing for a model (USD per 1,000,000 tokens).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPrice {
    #[serde(alias = "inputUsdPer1M")]
    pub input_usd_per_1m: f64,
    #[serde(alias = "outputUsdPer1M")]
    pub output_usd_per_1m: f64,
    #[serde(alias = "cachedInputUsdPer1M")]
    pub cached_input_usd_per_1m: Option<f64>,
    pub currency: String,
}

/// Capability tier used by the prompt router. Ordered: a higher tier is more
/// capable and, as a rule, more expensive.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    #[default]
    Fast,
    Balanced,
    Powerful,
}

impl ModelTier {
    pub fn label(&self) -> &'static str {
        match self {
            ModelTier::Fast => "fast",
            ModelTier::Balanced => "balanced",
            ModelTier::Powerful => "powerful",
        }
    }
}

/// Relative strengths on a 0-10 scale. The router compares a prompt's
/// required strengths against these, so two prompts with the same complexity
/// can still need different models (a story needs creativity, a migration
/// needs coding and reasoning).
///
/// These are tunable defaults, not benchmarks: they live here, in one place,
/// so re-rating a model never means touching routing logic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStrengths {
    pub coding: f64,
    pub reasoning: f64,
    pub creativity: f64,
    pub long_context: f64,
    pub instruction_following: f64,
}

impl ModelStrengths {
    pub const fn new(
        coding: f64,
        reasoning: f64,
        creativity: f64,
        long_context: f64,
        instruction_following: f64,
    ) -> Self {
        Self {
            coding,
            reasoning,
            creativity,
            long_context,
            instruction_following,
        }
    }

    /// Dimensions where `self` (a model) falls short of `required`.
    pub fn shortfalls(&self, required: &ModelStrengths) -> Vec<&'static str> {
        let pairs = [
            ("coding", self.coding, required.coding),
            ("reasoning", self.reasoning, required.reasoning),
            ("creativity", self.creativity, required.creativity),
            ("long-context", self.long_context, required.long_context),
            (
                "instruction-following",
                self.instruction_following,
                required.instruction_following,
            ),
        ];
        pairs
            .into_iter()
            .filter(|(_, have, need)| have + 1e-9 < *need)
            .map(|(name, _, _)| name)
            .collect()
    }

    /// Default strengths for a model known only by its tier, such as a
    /// self-hosted model the user registered. Deliberately conservative: the
    /// router may escalate, but should not over-trust an unknown model.
    pub fn for_tier(tier: ModelTier) -> Self {
        match tier {
            ModelTier::Fast => Self::new(5.0, 4.0, 5.0, 5.0, 6.0),
            ModelTier::Balanced => Self::new(7.0, 7.0, 7.0, 7.0, 8.0),
            ModelTier::Powerful => Self::new(9.0, 9.0, 8.0, 9.0, 9.0),
        }
    }

    pub fn total(&self) -> f64 {
        self.coding
            + self.reasoning
            + self.creativity
            + self.long_context
            + self.instruction_following
    }
}

/// Catalog entry describing pricing and capabilities for a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub model_id: String,
    pub display_name: String,
    pub provider: String,
    pub context_cap: usize,
    pub pricing: ModelPrice,
    pub capabilities: CapabilityProfile,
    #[serde(default)]
    pub tier: ModelTier,
    #[serde(default)]
    pub strengths: ModelStrengths,
    /// The model name sent to the provider API, when it differs from the part
    /// of `model_id` after the `provider/` prefix.
    #[serde(default)]
    pub api_model: Option<String>,
}

/// Provider name for models the user hosts themselves behind an
/// OpenAI-compatible API (Ollama, vLLM, LM Studio, llama.cpp server, ...).
pub const SELF_HOSTED_PROVIDER: &str = "custom";

impl CatalogEntry {
    /// A self-hosted model. It costs nothing per token, so the router prefers
    /// it whenever its tier and strengths are sufficient.
    pub fn self_hosted(
        id: &str,
        display_name: &str,
        api_model: &str,
        context_cap: usize,
        tier: ModelTier,
    ) -> Self {
        let mut entry = entry(
            &format!("{SELF_HOSTED_PROVIDER}/{id}"),
            display_name,
            SELF_HOSTED_PROVIDER,
            context_cap,
            (0.0, 0.0, Some(0.0)),
            tier,
            ModelStrengths::for_tier(tier),
            false,
        );
        entry.api_model = Some(api_model.to_string());
        entry
    }

    /// True for models that run on hardware the user controls.
    pub fn is_private(&self) -> bool {
        self.provider == "local" || self.provider == SELF_HOSTED_PROVIDER
    }

    /// The identifier the provider's API expects.
    pub fn api_model_name(&self) -> &str {
        if let Some(name) = &self.api_model {
            return name;
        }
        self.model_id
            .split_once('/')
            .map(|(_, name)| name)
            .unwrap_or(&self.model_id)
    }
}

/// Versioned, timestamped model and pricing catalog (ADR 0008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceCatalog {
    pub version: String,
    pub updated_at_ms: u64,
    pub update_source: String,
    pub entries: Vec<CatalogEntry>,
}

impl Default for PriceCatalog {
    fn default() -> Self {
        Self::default_catalog()
    }
}

impl PriceCatalog {
    /// Checks whether the pricing catalog is older than `max_age_days`.
    pub fn is_stale(&self, max_age_days: u64, now_ms: u64) -> bool {
        let max_age_ms = max_age_days * 24 * 60 * 60 * 1000;
        now_ms.saturating_sub(self.updated_at_ms) > max_age_ms
    }

    /// Finds a catalog entry by model ID.
    pub fn get(&self, model_id: &str) -> Option<&CatalogEntry> {
        self.entries.iter().find(|e| e.model_id == model_id)
    }

    /// Calculates estimated cost in USD based on token counts.
    pub fn estimate_cost(
        &self,
        model_id: &str,
        prompt_tokens: usize,
        completion_tokens: usize,
        cached_tokens: usize,
    ) -> Option<f64> {
        let entry = self.get(model_id)?;
        let non_cached_prompt = prompt_tokens.saturating_sub(cached_tokens);
        let input_cost = (non_cached_prompt as f64 / 1_000_000.0) * entry.pricing.input_usd_per_1m;
        let output_cost =
            (completion_tokens as f64 / 1_000_000.0) * entry.pricing.output_usd_per_1m;
        let cached_cost = entry
            .pricing
            .cached_input_usd_per_1m
            .map(|rate| (cached_tokens as f64 / 1_000_000.0) * rate)
            .unwrap_or(0.0);
        Some(input_cost + output_cost + cached_cost)
    }

    /// Default bundled official catalog baseline.
    ///
    /// This is the single place provider model names live. Routing logic
    /// only ever sees tiers, strengths, prices and context caps.
    pub fn default_catalog() -> Self {
        Self {
            version: "2026-09-22.1".to_string(),
            updated_at_ms: 1790058100000,
            update_source: "official_provider_pricing".to_string(),
            entries: vec![
                entry(
                    "openai/gpt-4o-mini",
                    "GPT-4o mini",
                    "openai",
                    128_000,
                    (0.15, 0.60, Some(0.075)),
                    ModelTier::Fast,
                    ModelStrengths::new(5.0, 4.0, 6.0, 5.0, 6.0),
                    true,
                ),
                entry(
                    "openai/gpt-4o",
                    "GPT-4o",
                    "openai",
                    128_000,
                    (2.50, 10.00, Some(1.25)),
                    ModelTier::Balanced,
                    ModelStrengths::new(7.0, 7.0, 8.0, 6.0, 8.0),
                    true,
                ),
                entry(
                    "anthropic/claude-haiku-4-5",
                    "Claude Haiku 4.5",
                    "anthropic",
                    200_000,
                    (1.00, 5.00, Some(0.10)),
                    ModelTier::Fast,
                    ModelStrengths::new(6.0, 5.0, 6.0, 6.0, 7.0),
                    true,
                ),
                entry(
                    "anthropic/claude-sonnet-5",
                    "Claude Sonnet 5",
                    "anthropic",
                    1_000_000,
                    (2.00, 10.00, Some(0.20)),
                    ModelTier::Balanced,
                    ModelStrengths::new(9.0, 8.0, 8.0, 9.0, 9.0),
                    true,
                ),
                entry(
                    "anthropic/claude-opus-5",
                    "Claude Opus 5",
                    "anthropic",
                    1_000_000,
                    (5.00, 25.00, Some(0.50)),
                    ModelTier::Powerful,
                    ModelStrengths::new(10.0, 10.0, 9.0, 10.0, 10.0),
                    true,
                ),
                entry(
                    "local/qwen2.5-coder-7b",
                    "Qwen 2.5 Coder 7B (GGUF)",
                    "local",
                    32_768,
                    (0.0, 0.0, Some(0.0)),
                    ModelTier::Fast,
                    ModelStrengths::new(4.0, 3.0, 3.0, 3.0, 4.0),
                    false,
                ),
            ],
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn entry(
    model_id: &str,
    display_name: &str,
    provider: &str,
    context_cap: usize,
    (input, output, cached): (f64, f64, Option<f64>),
    tier: ModelTier,
    strengths: ModelStrengths,
    cloud: bool,
) -> CatalogEntry {
    CatalogEntry {
        model_id: model_id.to_string(),
        display_name: display_name.to_string(),
        provider: provider.to_string(),
        context_cap,
        pricing: ModelPrice {
            input_usd_per_1m: input,
            output_usd_per_1m: output,
            cached_input_usd_per_1m: cached,
            currency: "USD".to_string(),
        },
        capabilities: CapabilityProfile {
            context_cap,
            streaming: true,
            tool_calling: cloud,
            structured_output: true,
            vision: cloud,
            exact_token_counting: false,
            cached_token_policy: if cloud {
                CachedTokenPolicy::PromptPrefix
            } else {
                CachedTokenPolicy::None
            },
        },
        tier,
        strengths,
        api_model: None,
    }
}
