use serde::{Deserialize, Serialize};

use crate::catalog::PriceCatalog;
use crate::error::{CoreError, Result};

/// Task classifications matching the routing policy in Instructions.md §9.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskClass {
    FileInventory,
    ContextDocumentation,
    MultiFilePlanning,
    CodeChangeProposal,
    MechanicalValidation,
}

/// User-visible routing policy configuration (ADR 0008).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingPolicy {
    pub low_cost_model_id: String,
    pub strong_model_id: String,
    pub auto_escalate_on_context_exceeded: bool,
    pub max_budget_usd: Option<f64>,
    pub require_local_only: bool,
}

impl Default for RoutingPolicy {
    fn default() -> Self {
        Self {
            low_cost_model_id: "openai/gpt-4o-mini".to_string(),
            strong_model_id: "openai/gpt-4o".to_string(),
            auto_escalate_on_context_exceeded: true,
            max_budget_usd: None,
            require_local_only: false,
        }
    }
}

/// Rationale for why a particular model was chosen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingReason {
    TaskDefaultLowCost,
    TaskRequiresStrongReasoning,
    EscalatedContextExceededLowCostCap,
    LocalOnlyEnforced,
}

/// The auditable routing decision produced for a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutingDecision {
    pub task_class: TaskClass,
    pub selected_model_id: String,
    pub selected_provider: String,
    pub reason: RoutingReason,
    pub explanation: String,
    pub estimated_cost_usd: Option<f64>,
}

/// Evaluates routing policy deterministically against task requirements and context size.
pub fn route_task(
    task_class: TaskClass,
    context_tokens: usize,
    policy: &RoutingPolicy,
    catalog: &PriceCatalog,
) -> Result<RoutingDecision> {
    if policy.require_local_only {
        let local_entry = catalog
            .entries
            .iter()
            .find(|e| e.provider == "local")
            .ok_or_else(|| {
                CoreError::InvalidModel("No local model available in catalog".to_string())
            })?;

        if context_tokens > local_entry.context_cap {
            return Err(CoreError::LimitExceeded(format!(
                "Context size ({} tokens) exceeds local model capacity ({} tokens)",
                context_tokens, local_entry.context_cap
            )));
        }

        return Ok(RoutingDecision {
            task_class,
            selected_model_id: local_entry.model_id.clone(),
            selected_provider: local_entry.provider.clone(),
            reason: RoutingReason::LocalOnlyEnforced,
            explanation: "Policy requires local-only inference; selected local GGUF model."
                .to_string(),
            estimated_cost_usd: Some(0.0),
        });
    }

    let default_strong = match task_class {
        TaskClass::MultiFilePlanning | TaskClass::CodeChangeProposal => true,
        TaskClass::FileInventory
        | TaskClass::ContextDocumentation
        | TaskClass::MechanicalValidation => false,
    };

    let (chosen_model_id, reason, explanation) = if default_strong {
        (
            policy.strong_model_id.clone(),
            RoutingReason::TaskRequiresStrongReasoning,
            "Complex task requires strong reasoning model tier per policy.".to_string(),
        )
    } else {
        let low_entry = catalog.get(&policy.low_cost_model_id).ok_or_else(|| {
            CoreError::InvalidModel(format!(
                "Configured low-cost model `{}` not in catalog",
                policy.low_cost_model_id
            ))
        })?;

        if context_tokens > low_entry.context_cap {
            if policy.auto_escalate_on_context_exceeded {
                (
                    policy.strong_model_id.clone(),
                    RoutingReason::EscalatedContextExceededLowCostCap,
                    format!(
                        "Context ({} tokens) exceeds low-cost tier cap ({} tokens); escalated to strong tier.",
                        context_tokens, low_entry.context_cap
                    ),
                )
            } else {
                return Err(CoreError::LimitExceeded(format!(
                    "Context ({} tokens) exceeds low-cost tier limit ({} tokens)",
                    context_tokens, low_entry.context_cap
                )));
            }
        } else {
            (
                policy.low_cost_model_id.clone(),
                RoutingReason::TaskDefaultLowCost,
                "Standard task routed to default low-cost tier.".to_string(),
            )
        }
    };

    let entry = catalog.get(&chosen_model_id).ok_or_else(|| {
        CoreError::InvalidModel(format!("Selected model `{chosen_model_id}` not in catalog"))
    })?;

    if context_tokens > entry.context_cap {
        return Err(CoreError::LimitExceeded(format!(
            "Context ({} tokens) exceeds selected model context cap ({} tokens)",
            context_tokens, entry.context_cap
        )));
    }

    let estimated_cost = catalog.estimate_cost(&chosen_model_id, context_tokens, 500, 0);

    if let (Some(max_budget), Some(cost)) = (policy.max_budget_usd, estimated_cost) {
        if cost > max_budget {
            return Err(CoreError::BudgetExceeded(format!(
                "Estimated task cost (${cost:.4}) exceeds maximum configured budget (${max_budget:.4})"
            )));
        }
    }

    Ok(RoutingDecision {
        task_class,
        selected_model_id: entry.model_id.clone(),
        selected_provider: entry.provider.clone(),
        reason,
        explanation,
        estimated_cost_usd: estimated_cost,
    })
}
