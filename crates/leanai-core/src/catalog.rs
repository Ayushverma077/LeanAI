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
    pub fn default_catalog() -> Self {
        Self {
            version: "2026-09-06.1".to_string(),
            updated_at_ms: 1788675700000,
            update_source: "official_provider_pricing".to_string(),
            entries: vec![
                CatalogEntry {
                    model_id: "openai/gpt-4o-mini".to_string(),
                    display_name: "GPT-4o mini".to_string(),
                    provider: "openai".to_string(),
                    context_cap: 128_000,
                    pricing: ModelPrice {
                        input_usd_per_1m: 0.15,
                        output_usd_per_1m: 0.60,
                        cached_input_usd_per_1m: Some(0.075),
                        currency: "USD".to_string(),
                    },
                    capabilities: CapabilityProfile {
                        context_cap: 128_000,
                        streaming: true,
                        tool_calling: true,
                        structured_output: true,
                        vision: true,
                        exact_token_counting: false,
                        cached_token_policy: CachedTokenPolicy::PromptPrefix,
                    },
                },
                CatalogEntry {
                    model_id: "openai/gpt-4o".to_string(),
                    display_name: "GPT-4o".to_string(),
                    provider: "openai".to_string(),
                    context_cap: 128_000,
                    pricing: ModelPrice {
                        input_usd_per_1m: 2.50,
                        output_usd_per_1m: 10.00,
                        cached_input_usd_per_1m: Some(1.25),
                        currency: "USD".to_string(),
                    },
                    capabilities: CapabilityProfile {
                        context_cap: 128_000,
                        streaming: true,
                        tool_calling: true,
                        structured_output: true,
                        vision: true,
                        exact_token_counting: false,
                        cached_token_policy: CachedTokenPolicy::PromptPrefix,
                    },
                },
                CatalogEntry {
                    model_id: "anthropic/claude-3-5-haiku".to_string(),
                    display_name: "Claude 3.5 Haiku".to_string(),
                    provider: "anthropic".to_string(),
                    context_cap: 200_000,
                    pricing: ModelPrice {
                        input_usd_per_1m: 0.80,
                        output_usd_per_1m: 4.00,
                        cached_input_usd_per_1m: Some(0.08),
                        currency: "USD".to_string(),
                    },
                    capabilities: CapabilityProfile {
                        context_cap: 200_000,
                        streaming: true,
                        tool_calling: true,
                        structured_output: true,
                        vision: true,
                        exact_token_counting: false,
                        cached_token_policy: CachedTokenPolicy::PromptPrefix,
                    },
                },
                CatalogEntry {
                    model_id: "anthropic/claude-3-5-sonnet".to_string(),
                    display_name: "Claude 3.5 Sonnet".to_string(),
                    provider: "anthropic".to_string(),
                    context_cap: 200_000,
                    pricing: ModelPrice {
                        input_usd_per_1m: 3.00,
                        output_usd_per_1m: 15.00,
                        cached_input_usd_per_1m: Some(0.30),
                        currency: "USD".to_string(),
                    },
                    capabilities: CapabilityProfile {
                        context_cap: 200_000,
                        streaming: true,
                        tool_calling: true,
                        structured_output: true,
                        vision: true,
                        exact_token_counting: false,
                        cached_token_policy: CachedTokenPolicy::PromptPrefix,
                    },
                },
                CatalogEntry {
                    model_id: "local/qwen2.5-coder-7b".to_string(),
                    display_name: "Qwen 2.5 Coder 7B (GGUF)".to_string(),
                    provider: "local".to_string(),
                    context_cap: 32_768,
                    pricing: ModelPrice {
                        input_usd_per_1m: 0.0,
                        output_usd_per_1m: 0.0,
                        cached_input_usd_per_1m: Some(0.0),
                        currency: "USD".to_string(),
                    },
                    capabilities: CapabilityProfile {
                        context_cap: 32_768,
                        streaming: true,
                        tool_calling: false,
                        structured_output: true,
                        vision: false,
                        exact_token_counting: false,
                        cached_token_policy: CachedTokenPolicy::None,
                    },
                },
            ],
        }
    }
}
