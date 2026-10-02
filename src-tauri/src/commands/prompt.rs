//! One-box prompt execution: route locally, give the chosen model the minimum
//! context, let it request what it needs, validate, and apply safely.
//!
//! Flow: analyse prompt → context budget → minimum sufficient model →
//! minimal initial context → model-driven file reads/searches (validated,
//! bounded) → structured edits → validation → apply gate → approval or
//! transactional apply → PROJECT_CONTEXT refresh.
//!
//! LeanAI never pre-selects source files for the model. The model sees only
//! the PROJECT_CONTEXT sections the budget allows and asks for everything
//! else by path, range or search.

use std::collections::{BTreeSet, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{ipc::Channel, State};

use leanai_core::agent::{
    build_context_manifest, scan_untrusted_text_for_injection, validate_proposal, PatchProposal,
    ToolCapability, ValidatorVerdict,
};
use leanai_core::apply_gate::{self, ApplyDecision, ApplyLevel, GateInput};
use leanai_core::approval::{ApprovalRequest, TransactionalPatchSession};
use leanai_core::catalog::{CatalogEntry, PriceCatalog};
use leanai_core::classify::FileClass;
use leanai_core::inventory::Inventory;
use leanai_core::llm_protocol::{self, FileRequest, ModelAction};
use leanai_core::policy::Policy;
use leanai_core::project::{now_ms, resolve_within_root};
use leanai_core::routing::{
    analyze_prompt, plan_context_budget, select_model, ContextBudget, ContextLevel, ModelSelection,
    ProjectVocabulary, PromptAnalysis, RoutingPolicy,
};
use leanai_core::secrets::{scan_text, Confidence};
use leanai_core::tokenizer;

use crate::app_state::AppState;
use crate::commands::context::{ensure_project_context, refresh_project_context};
use crate::db::repositories::{append_run_event, save_approval, upsert_run, RunRecord};
use crate::error::{AppError, AppResult};
use crate::llm_client::{self, ChatMessage, Endpoint};

/// Hard ceiling on model calls per run, whatever the budget says.
const MAX_CALLS: u32 = 16;
/// Consecutive invalid replies or edits tolerated before escalating.
const FAILURES_BEFORE_ESCALATION: u32 = 2;
/// Search results returned to the model per search.
const MAX_SEARCH_MATCHES: usize = 40;
/// Files larger than this are not searched.
const MAX_SEARCH_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPromptRequest {
    pub prompt: String,
    #[serde(default)]
    pub history: Vec<ConversationMessage>,
    /// Allow low-risk validated changes to be applied without review. The
    /// apply gate still holds back anything it ranks above low risk.
    #[serde(default)]
    pub auto_apply: bool,
    #[serde(default)]
    pub require_local_only: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConversationRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConversationMessage {
    pub role: ConversationRole,
    pub content: String,
}

/// One file or range the model received (or was refused).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalRecord {
    pub path: String,
    pub from_line: usize,
    pub to_line: usize,
    pub tokens: usize,
    /// Set when the request was refused, with the reason given to the model.
    pub refused: Option<String>,
    pub redacted_lines: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceStep {
    pub kind: String,
    pub detail: String,
    pub tokens: usize,
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Escalation {
    pub from_model_id: String,
    pub to_model_id: String,
    pub reason: String,
}

/// Measured token flow for one run. Context numbers are local cl100k
/// estimates of what LeanAI sent; billed numbers come from the provider.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenMetrics {
    /// Selectable project text, estimated at 4 bytes per token.
    pub repository_tokens_estimate: usize,
    pub initial_context_tokens: usize,
    pub retrieved_tokens: usize,
    pub context_tokens_sent: usize,
    /// Repository tokens that were never sent. Measured, not targeted.
    pub tokens_avoided: usize,
    pub billed_input_tokens: usize,
    pub billed_output_tokens: usize,
    pub llm_calls: u32,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRunResponse {
    pub run_id: String,
    /// `completed`, `awaiting_approval` or `applied`.
    pub status: String,
    pub answer: Option<String>,
    pub summary: Option<String>,
    pub analysis: PromptAnalysis,
    pub budget: ContextBudget,
    pub selection: ModelSelection,
    pub final_model_id: String,
    pub final_model_name: String,
    pub escalations: Vec<Escalation>,
    pub retrievals: Vec<RetrievalRecord>,
    pub searches: Vec<String>,
    pub patch_proposal: Option<PatchProposal>,
    pub validator_verdict: Option<ValidatorVerdict>,
    /// How much review the change needs and why; set for every proposal.
    pub apply_decision: Option<ApplyDecision>,
    pub pending_approval: Option<ApprovalRequest>,
    pub files_changed: Vec<String>,
    pub context_refreshed: bool,
    pub metrics: TokenMetrics,
    pub trace: Vec<TraceStep>,
    pub warnings: Vec<String>,
}

enum Outcome {
    Answer(String),
    Proposal(PatchProposal, ValidatorVerdict),
}

/// Mutable state of one run, kept so a failed run is still recorded with
/// everything that happened before the failure.
struct RunLedger {
    progress: Option<Channel<TraceStep>>,
    trace: Vec<TraceStep>,
    retrievals: Vec<RetrievalRecord>,
    searches: Vec<String>,
    escalations: Vec<Escalation>,
    warnings: Vec<String>,
    metrics: TokenMetrics,
    /// Files read that contained text resembling instructions to the model.
    injection_warnings: usize,
    /// Models whose server rejected the reply schema this run. They are not
    /// sent it again, so each rejection costs one extra request at most.
    schema_unsupported: BTreeSet<String>,
}

impl RunLedger {
    fn step(&mut self, kind: &str, detail: impl Into<String>, tokens: usize, model: Option<&str>) {
        let step = TraceStep {
            kind: kind.to_string(),
            detail: detail.into(),
            tokens,
            model_id: model.map(str::to_string),
        };
        if let Some(progress) = &self.progress {
            let _ = progress.send(step.clone());
        }
        self.trace.push(step);
    }
}

#[tauri::command]
pub async fn run_prompt(
    state: State<'_, AppState>,
    request: RunPromptRequest,
    on_progress: Channel<TraceStep>,
) -> AppResult<PromptRunResponse> {
    run_prompt_reporting(&state, request, Some(on_progress)).await
}

/// The command body, callable without a Tauri runtime (integration tests).
pub async fn run_prompt_with(
    state: &AppState,
    request: RunPromptRequest,
) -> AppResult<PromptRunResponse> {
    run_prompt_reporting(state, request, None).await
}

async fn run_prompt_reporting(
    state: &AppState,
    request: RunPromptRequest,
    progress: Option<Channel<TraceStep>>,
) -> AppResult<PromptRunResponse> {
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::new("empty_prompt", "Enter a request first."));
    }
    // Bound conversation input independently of provider context limits.
    if request.history.len() > 12
        || request
            .history
            .iter()
            .map(|m| m.content.len())
            .sum::<usize>()
            > 48_000
    {
        return Err(AppError::new(
            "history_too_large",
            "Start a new task to continue.",
        ));
    }
    let history: Vec<ChatMessage> = request
        .history
        .iter()
        .map(|message| ChatMessage {
            role: match message.role {
                ConversationRole::User => "user",
                ConversationRole::Assistant => "assistant",
            },
            content: message.content.clone(),
        })
        .collect();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    let started = now_ms() as i64;
    let session = state.session();
    let catalog = crate::commands::models::effective_catalog(state)?;

    // --- 1. Local routing (zero provider tokens) ---------------------------
    let vocabulary = session
        .as_ref()
        .map(|s| ProjectVocabulary::from_paths(s.inventory.selectable().map(|f| f.path.as_str())));
    let routing_prompt = request
        .history
        .iter()
        .filter(|message| matches!(message.role, ConversationRole::User))
        .map(|message| message.content.as_str())
        .chain(std::iter::once(prompt.as_str()))
        .collect::<Vec<_>>()
        .join("\n");
    let mut analysis = analyze_prompt(&prompt, vocabulary.as_ref());
    if !history.is_empty() {
        let previous = analyze_prompt(&routing_prompt, vocabulary.as_ref());
        // Earlier requests inform the context budget, never authorization to edit.
        analysis.context_requirement = analysis
            .context_requirement
            .max(previous.context_requirement);
        analysis.complexity = analysis.complexity.max(previous.complexity);
    }
    let budget = plan_context_budget(&analysis);
    if budget.allow_retrieval && session.is_none() {
        return Err(AppError::new(
            "no_project",
            "This request needs a project, but none is open.",
        )
        .with_recovery("Open a project folder, then send the request again."));
    }

    let (available, local_port) = available_models(state, &catalog);
    let policy = RoutingPolicy {
        require_local_only: request.require_local_only,
        ..RoutingPolicy::default()
    };
    let prompt_tokens = tokenizer::estimate(&prompt).value
        + history
            .iter()
            .map(|message| tokenizer::estimate(&message.content).value)
            .sum::<usize>();
    let selection = select_model(
        &analysis,
        &budget,
        prompt_tokens,
        &catalog,
        &available,
        &policy,
    )
    .map_err(|e| {
        let message = match e {
            leanai_core::CoreError::InvalidModel(message) => message,
            other => other.to_string(),
        };
        AppError::new("no_model_available", message)
            .with_recovery("Open Models to add an API key or start a local model.")
    })?;

    let mut ledger = RunLedger {
        progress,
        trace: Vec::new(),
        retrievals: Vec::new(),
        searches: Vec::new(),
        escalations: Vec::new(),
        warnings: Vec::new(),
        metrics: TokenMetrics {
            repository_tokens_estimate: session
                .as_ref()
                .map(|s| (s.inventory.selectable_bytes() / 4) as usize)
                .unwrap_or(0),
            ..Default::default()
        },
        injection_warnings: 0,
        schema_unsupported: BTreeSet::new(),
    };
    ledger.step(
        "route",
        format!(
            "{:?} · complexity {:.1} · context {:.1} · {:?} context · {} ({} tier, minimum {})",
            analysis.intent,
            analysis.complexity,
            analysis.context_requirement,
            budget.level,
            selection.display_name,
            selection.tier.label(),
            selection.minimum_tier.label(),
        ),
        0,
        None,
    );
    if !selection.capability_shortfall.is_empty() {
        ledger.warnings.push(format!(
            "No available model fully meets this request ({}). Connect a stronger model for better results.",
            selection.capability_shortfall.join(", ")
        ));
    }

    let result = execute(
        state,
        &prompt,
        &history,
        &budget,
        &selection,
        &catalog,
        local_port,
        &mut ledger,
    )
    .await;
    let metrics = &mut ledger.metrics;
    metrics.context_tokens_sent = metrics.initial_context_tokens + metrics.retrieved_tokens;
    metrics.tokens_avoided = metrics
        .repository_tokens_estimate
        .saturating_sub(metrics.context_tokens_sent);

    // --- Outcome: answer, approval, or transactional apply -----------------
    let mut final_model = selection.model_id.clone();
    if let Some(last) = ledger.escalations.last() {
        final_model = last.to_model_id.clone();
    }
    let final_model_name = catalog
        .get(&final_model)
        .map(|e| e.display_name.clone())
        .unwrap_or_else(|| final_model.clone());

    let outcome = match result {
        Ok(outcome) => outcome,
        Err(error) => {
            ledger.step("failed", error.message.clone(), 0, None);
            record_run(
                state, &run_id, &prompt, started, "failed", &ledger, &analysis, &budget,
                &selection, None, None,
            );
            return Err(error);
        }
    };
    if state.session().as_ref().map(|session| &session.root)
        != session.as_ref().map(|session| &session.root)
    {
        return Err(AppError::new(
            "project_changed",
            "The open project changed while this task was running.",
        )
        .with_recovery("Reopen the original project and send the request again."));
    }

    let mut response = PromptRunResponse {
        run_id: run_id.clone(),
        status: "completed".to_string(),
        answer: None,
        summary: None,
        analysis: analysis.clone(),
        budget: budget.clone(),
        selection: selection.clone(),
        final_model_id: final_model,
        final_model_name,
        escalations: Vec::new(),
        retrievals: Vec::new(),
        searches: Vec::new(),
        patch_proposal: None,
        validator_verdict: None,
        apply_decision: None,
        pending_approval: None,
        files_changed: Vec::new(),
        context_refreshed: false,
        metrics: TokenMetrics::default(),
        trace: Vec::new(),
        warnings: Vec::new(),
    };

    match outcome {
        Outcome::Answer(text) => {
            response.answer = Some(text);
        }
        Outcome::Proposal(proposal, verdict) => {
            let session = state.require_session()?;
            response.summary = Some(proposal.summary.clone());
            let decision = apply_gate::decide(&GateInput {
                auto_apply_enabled: request.auto_apply,
                analysis: &analysis,
                proposal: &proposal,
                verdict: &verdict,
                corrections: ledger
                    .trace
                    .iter()
                    .filter(|s| {
                        matches!(
                            s.kind.as_str(),
                            "invalid_reply"
                                | "edit_rejected"
                                | "validation_failed"
                                | "insufficient"
                        )
                    })
                    .count(),
                injection_warnings: ledger.injection_warnings,
            });
            ledger.step(
                "apply_gate",
                match decision.reasons.first() {
                    None if decision.level == ApplyLevel::AutoApply => {
                        "Low risk; applying automatically.".to_string()
                    }
                    None => "Low risk; automatic apply is off, so waiting for review.".to_string(),
                    Some(_) => format!(
                        "{}: {}",
                        match decision.level {
                            ApplyLevel::CarefulReview => "Careful review",
                            _ => "Review",
                        },
                        decision
                            .reasons
                            .iter()
                            .map(|r| r.text.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                },
                0,
                None,
            );
            if decision.level == ApplyLevel::AutoApply {
                apply_proposal(&session.root, &proposal)?;
                response.status = "applied".to_string();
                response.files_changed = proposal.affected_files.clone();
                ledger.step(
                    "apply",
                    format!(
                        "Applied transactionally to {} file(s).",
                        proposal.affected_files.len()
                    ),
                    0,
                    None,
                );
                match refresh_project_context(state).await {
                    Ok(_) => {
                        response.context_refreshed = true;
                        ledger.step(
                            "context_refresh",
                            "Rescanned and regenerated PROJECT_CONTEXT.md.",
                            0,
                            None,
                        );
                    }
                    Err(error) => ledger.warnings.push(format!(
                        "Files were updated, but the project context could not be refreshed: {}",
                        error.message
                    )),
                }
            } else {
                let approval = ApprovalRequest::new(
                    run_id.clone(),
                    ToolCapability::WriteFile,
                    session.root.to_string_lossy().to_string(),
                    proposal.affected_files.clone(),
                    proposal.sha256_hash.clone(),
                    30 * 60 * 1000,
                );
                response.status = "awaiting_approval".to_string();
                response.pending_approval = Some(approval);
            }
            response.patch_proposal = Some(proposal);
            response.validator_verdict = Some(verdict);
            response.apply_decision = Some(decision);
        }
    }

    let status = match response.status.as_str() {
        "awaiting_approval" => "awaiting_approval",
        _ => "succeeded",
    };
    record_run(
        state,
        &run_id,
        &prompt,
        started,
        status,
        &ledger,
        &analysis,
        &budget,
        &selection,
        response.validator_verdict.as_ref(),
        response.apply_decision.as_ref(),
    );
    // The approval references the run row, so it is stored after it.
    if let Some(approval) = &response.pending_approval {
        state.with_db(|conn| save_approval(conn, approval))?;
    }

    response.escalations = ledger.escalations;
    response.retrievals = ledger.retrievals;
    response.searches = ledger.searches;
    response.metrics = ledger.metrics;
    response.trace = ledger.trace;
    response.warnings = ledger.warnings;
    Ok(response)
}

/// Applies a validated proposal with the existing transactional session:
/// every file is backed up first and restored if any write fails.
pub(crate) fn apply_proposal(root: &Path, proposal: &PatchProposal) -> AppResult<()> {
    let mut session = TransactionalPatchSession::new(root);
    if let Err(error) = session.apply(proposal, &Policy::default()) {
        let _ = session.rollback();
        return Err(AppError::new(
            "patch_application_failed",
            format!("The change could not be applied ({error}). All files were restored."),
        ));
    }
    Ok(())
}

/// Catalog models the user can call right now.
fn available_models(state: &AppState, catalog: &PriceCatalog) -> (Vec<String>, Option<u16>) {
    let configured = |provider: &str| state.keychain.is_configured(provider).unwrap_or(false);
    let anthropic = configured("anthropic");
    let openai = configured("openai");
    let local_port = match state.sidecar.status() {
        crate::sidecar_manager::SidecarStatus::Ready { port, .. } => Some(port),
        _ => None,
    };
    let ids = catalog
        .entries
        .iter()
        .filter(|e| match e.provider.as_str() {
            "anthropic" => anthropic,
            "openai" => openai,
            "local" => local_port.is_some(),
            // Reachability is only known by calling; an unreachable server
            // triggers escalation to the next model.
            leanai_core::catalog::SELF_HOSTED_PROVIDER => true,
            _ => false,
        })
        .map(|e| e.model_id.clone())
        .collect();
    (ids, local_port)
}

fn endpoint_for(
    state: &AppState,
    entry: &CatalogEntry,
    local_port: Option<u16>,
) -> AppResult<Endpoint> {
    let key = |provider: &str| -> AppResult<String> {
        state.keychain.read(provider)?.ok_or_else(|| {
            AppError::new(
                "provider_not_configured",
                format!("No API key is stored for {provider}."),
            )
            .with_recovery("Add the key on the Models page.")
        })
    };
    match entry.provider.as_str() {
        "anthropic" => Ok(Endpoint::Anthropic {
            api_key: key("anthropic")?,
        }),
        "openai" => Ok(Endpoint::OpenAi {
            api_key: key("openai")?,
        }),
        "local" => local_port
            .map(|port| Endpoint::Local { port })
            .ok_or_else(|| AppError::new("local_model_stopped", "The local model is not running.")),
        leanai_core::catalog::SELF_HOSTED_PROVIDER => {
            crate::commands::models::self_hosted_endpoint(state, &entry.model_id)
        }
        other => Err(AppError::new(
            "unknown_provider",
            format!("Unknown provider `{other}`."),
        )),
    }
}

/// The model loop. Returns the final answer or a validated proposal.
#[allow(clippy::too_many_arguments)]
async fn execute(
    state: &AppState,
    prompt: &str,
    history: &[ChatMessage],
    budget: &ContextBudget,
    selection: &ModelSelection,
    catalog: &PriceCatalog,
    local_port: Option<u16>,
    ledger: &mut RunLedger,
) -> AppResult<Outcome> {
    let mut model = catalog
        .get(&selection.model_id)
        .cloned()
        .ok_or_else(|| AppError::internal("selected model is not in the catalog"))?;
    let mut endpoint = endpoint_for(state, &model, local_port)?;
    let mut chain: VecDeque<String> = selection.escalation_chain.iter().cloned().collect();

    // --- Direct mode: no repository involved at all ------------------------
    if budget.level == ContextLevel::None {
        let mut messages = history.to_vec();
        messages.push(ChatMessage {
            role: "user",
            content: prompt.to_string(),
        });
        loop {
            let reply = call(
                &endpoint,
                &model,
                llm_protocol::direct_system_prompt(),
                &messages,
                budget.max_output_tokens,
                None,
                ledger,
            )
            .await;
            match reply {
                Ok((reply, truncated)) if !reply.trim().is_empty() => {
                    if truncated {
                        ledger.warnings.push(
                            "The answer reached the output limit and may be cut short.".to_string(),
                        );
                    }
                    return Ok(Outcome::Answer(reply));
                }
                Ok(_) => escalate(
                    &mut chain,
                    &mut model,
                    &mut endpoint,
                    state,
                    catalog,
                    local_port,
                    ledger,
                    "empty reply",
                )?,
                Err(error) if escalates_on(&error) => escalate(
                    &mut chain,
                    &mut model,
                    &mut endpoint,
                    state,
                    catalog,
                    local_port,
                    ledger,
                    &error.message,
                )?,
                Err(error) => return Err(error),
            }
        }
    }

    // --- Agent mode: minimal context, model-driven retrieval ---------------
    let session = state.require_session()?;
    let document = ensure_project_context(state).await?;
    // The context refresh may have rescanned; always use the latest inventory.
    let refreshed = state.require_session()?;
    if refreshed.root != session.root {
        return Err(AppError::new(
            "project_changed",
            "The open project changed while preparing context.",
        ));
    }
    let session = refreshed;
    let (index, index_tokens) = leanai_core::context::render_sections(
        &document,
        &budget.sections,
        budget.initial_context_tokens,
    );
    ledger.metrics.initial_context_tokens = index_tokens;
    ledger.step(
        "initial_context",
        format!(
            "Sent PROJECT_CONTEXT sections: {}.",
            budget.sections.join(", ")
        ),
        index_tokens,
        None,
    );

    let root = session.root.clone();
    let inventory = session.inventory.clone();
    let system = llm_protocol::agent_system_prompt(budget.max_rounds, budget.max_retrieved_tokens);
    let schema = llm_protocol::action_schema();
    let mut messages = history.to_vec();
    messages.push(ChatMessage {
        role: "user",
        content: format!(
            "# Project index: {}\n\n{index}\n# Request\n\n{prompt}",
            session.record.display_name
        ),
    });
    let mut read_paths: HashSet<String> = HashSet::new();
    let mut rounds = 0u32;
    let mut failures = 0u32;
    let mut calls = 0u32;

    loop {
        calls += 1;
        if calls > MAX_CALLS {
            return Err(AppError::new(
                "turn_limit_reached",
                "The model did not finish within the turn limit. No files were changed.",
            )
            .with_recovery("Try a more specific request."));
        }
        let max_output = budget
            .max_output_tokens
            .max(if model.provider == "anthropic" {
                8_000
            } else {
                0
            });
        let reply = match call(
            &endpoint,
            &model,
            &system,
            &messages,
            max_output,
            Some(&schema),
            ledger,
        )
        .await
        {
            // A cut-off reply cannot be a complete action; it fails parsing below.
            Ok((reply, truncated)) => {
                if truncated {
                    String::new()
                } else {
                    reply
                }
            }
            Err(error) if escalates_on(&error) => {
                escalate(
                    &mut chain,
                    &mut model,
                    &mut endpoint,
                    state,
                    catalog,
                    local_port,
                    ledger,
                    &error.message,
                )?;
                continue;
            }
            Err(error) => return Err(error),
        };

        let action = match llm_protocol::parse_action(&reply) {
            Ok(action) => action,
            Err(error) => {
                failures += 1;
                ledger.step("invalid_reply", error.to_string(), 0, Some(&model.model_id));
                messages.push(ChatMessage {
                    role: "assistant",
                    content: reply,
                });
                messages.push(ChatMessage {
                    role: "user",
                    content: format!("Your reply was rejected: {error}. Reply with exactly one JSON action object."),
                });
                if failures >= FAILURES_BEFORE_ESCALATION {
                    escalate(
                        &mut chain,
                        &mut model,
                        &mut endpoint,
                        state,
                        catalog,
                        local_port,
                        ledger,
                        "repeated invalid structured output",
                    )?;
                    failures = 0;
                }
                continue;
            }
        };

        let retrieval_left = budget
            .max_retrieved_tokens
            .saturating_sub(ledger.metrics.retrieved_tokens);
        let exhausted = rounds >= budget.max_rounds;
        let feedback = match action {
            ModelAction::Answer { text } => {
                ledger.step("answer", "Model answered.", 0, Some(&model.model_id));
                return Ok(Outcome::Answer(text));
            }
            ModelAction::Insufficient { reason } => {
                ledger.step("insufficient", reason.clone(), 0, Some(&model.model_id));
                escalate(
                    &mut chain,
                    &mut model,
                    &mut endpoint,
                    state,
                    catalog,
                    local_port,
                    ledger,
                    &format!("model reported: {reason}"),
                )?;
                failures = 0;
                // The stronger model continues the same conversation.
                format!("A more capable model is taking over. Continue the task. (Previous model said: {reason})")
            }
            ModelAction::ReadFiles { .. } | ModelAction::Search { .. } if exhausted => {
                failures += 1;
                "Retrieval budget exhausted. Reply now with an edit or answer action using what you have.".to_string()
            }
            ModelAction::ReadFiles { files, reason } => {
                rounds += 1;
                let mut blocks = Vec::new();
                for file in files {
                    let left = budget
                        .max_retrieved_tokens
                        .saturating_sub(ledger.metrics.retrieved_tokens);
                    let (block, record) = read_for_model(
                        &root,
                        &inventory,
                        &file,
                        left,
                        budget.max_file_tokens,
                        // Redact likely secrets whenever text leaves this
                        // machine, including to self-hosted servers.
                        model.provider != "local",
                    );
                    if record.refused.is_none() {
                        read_paths.insert(record.path.clone());
                        ledger.metrics.retrieved_tokens += record.tokens;
                    }
                    ledger.step(
                        "read_file",
                        match &record.refused {
                            Some(why) => format!("{} refused: {why}", record.path),
                            None => format!(
                                "{} lines {}-{}",
                                record.path, record.from_line, record.to_line
                            ),
                        },
                        record.tokens,
                        Some(&model.model_id),
                    );
                    for warning in scan_untrusted_text_for_injection(&block) {
                        ledger.injection_warnings += 1;
                        ledger
                            .warnings
                            .push(format!("{}: {warning} (treated as data)", record.path));
                    }
                    ledger.retrievals.push(record);
                    blocks.push(block);
                }
                if let Some(reason) = reason.filter(|r| !r.trim().is_empty()) {
                    ledger.step("model_reason", reason, 0, Some(&model.model_id));
                }
                format!(
                    "{}\n\n(Retrieval budget left: about {} tokens, {} turns.)",
                    blocks.join("\n\n"),
                    budget
                        .max_retrieved_tokens
                        .saturating_sub(ledger.metrics.retrieved_tokens),
                    budget.max_rounds.saturating_sub(rounds)
                )
            }
            ModelAction::Search { query, path_prefix } => {
                rounds += 1;
                let (text, matches) = search_project(
                    root.clone(),
                    inventory.clone(),
                    query.clone(),
                    path_prefix.clone(),
                )
                .await;
                let mut text = text;
                let mut tokens = tokenizer::estimate(&text).value;
                if tokens > retrieval_left {
                    text = format!("Search results withheld: retrieval budget exhausted ({retrieval_left} tokens left).");
                    tokens = 0;
                }
                ledger.metrics.retrieved_tokens += tokens;
                ledger.searches.push(query.clone());
                ledger.step(
                    "search",
                    format!("\"{query}\" → {matches} match(es)"),
                    tokens,
                    Some(&model.model_id),
                );
                text
            }
            ModelAction::Edit { summary, edits } => {
                match llm_protocol::build_proposal(&root, &summary, &edits, &read_paths) {
                    Err(error) => {
                        failures += 1;
                        ledger.step("edit_rejected", error.to_string(), 0, Some(&model.model_id));
                        format!("Your edit was rejected: {error}. Fix it and reply with a corrected edit action (read the file again if needed).")
                    }
                    Ok(proposal) => {
                        let cited: Vec<String> = read_paths.iter().cloned().collect();
                        let manifest = build_context_manifest(&root, &cited, &Policy::default());
                        let verdict =
                            validate_proposal(&root, &proposal, &manifest, &Policy::default());
                        if verdict.is_valid {
                            ledger.step(
                                "validated",
                                format!(
                                    "{} file(s) changed; {} checks passed.",
                                    proposal.affected_files.len(),
                                    verdict.checks.iter().filter(|c| c.passed).count()
                                ),
                                0,
                                Some(&model.model_id),
                            );
                            return Ok(Outcome::Proposal(proposal, verdict));
                        }
                        failures += 1;
                        ledger.step(
                            "validation_failed",
                            verdict.errors.join("; "),
                            0,
                            Some(&model.model_id),
                        );
                        format!(
                            "Validation failed: {}. Produce a corrected edit.",
                            verdict.errors.join("; ")
                        )
                    }
                }
            }
        };

        messages.push(ChatMessage {
            role: "assistant",
            content: reply,
        });
        messages.push(ChatMessage {
            role: "user",
            content: feedback,
        });
        if failures >= FAILURES_BEFORE_ESCALATION {
            escalate(
                &mut chain,
                &mut model,
                &mut endpoint,
                state,
                catalog,
                local_port,
                ledger,
                "repeated invalid edits",
            )?;
            failures = 0;
        }
    }
}

/// Provider failures that are concrete evidence to move to the next model:
/// the model is not offered, or its server cannot be reached.
fn escalates_on(error: &AppError) -> bool {
    matches!(
        error.code.as_str(),
        "provider_model_unavailable"
            | "provider_unreachable"
            | "provider_unavailable"
            | "provider_request_too_large"
    )
}

/// One model call, tallied into the ledger. Returns the text and whether the
/// provider cut it at the output limit. `schema` constrains the reply where
/// the provider supports it.
async fn call(
    endpoint: &Endpoint,
    model: &CatalogEntry,
    system: &str,
    messages: &[ChatMessage],
    max_tokens: usize,
    schema: Option<&serde_json::Value>,
    ledger: &mut RunLedger,
) -> AppResult<(String, bool)> {
    let schema = schema.filter(|_| !ledger.schema_unsupported.contains(&model.model_id));
    ledger.step(
        "model_started",
        format!("Waiting for {}…", model.display_name),
        0,
        Some(&model.model_id),
    );
    let reply = llm_client::complete(
        endpoint,
        model.api_model_name(),
        system,
        messages,
        max_tokens,
        schema,
    )
    .await?;
    if reply.schema_dropped && ledger.schema_unsupported.insert(model.model_id.clone()) {
        ledger.warnings.push(format!(
            "{} does not support schema-constrained replies; LeanAI fell back to instructions only, so invalid replies are possible.",
            model.display_name
        ));
    }
    ledger.metrics.llm_calls += 1;
    ledger.metrics.billed_input_tokens += reply.input_tokens;
    ledger.metrics.billed_output_tokens += reply.output_tokens;
    let cost = PriceCatalog::default_catalog()
        .estimate_cost(&model.model_id, reply.input_tokens, reply.output_tokens, 0)
        .unwrap_or(0.0);
    ledger.metrics.estimated_cost_usd =
        ((ledger.metrics.estimated_cost_usd + cost) * 1e6).round() / 1e6;
    ledger.step(
        "llm_call",
        format!(
            "{} · {} in / {} out{}",
            model.display_name,
            reply.input_tokens,
            reply.output_tokens,
            if reply.truncated {
                " · hit output limit"
            } else {
                ""
            }
        ) + if schema.is_some() && !reply.schema_dropped {
            " · schema-constrained"
        } else {
            ""
        },
        reply.input_tokens + reply.output_tokens,
        Some(&model.model_id),
    );
    Ok((reply.text, reply.truncated))
}

/// Moves to the next stronger model, or fails when none is left. Escalation
/// only ever happens on concrete evidence passed in as `reason`.
#[allow(clippy::too_many_arguments)]
fn escalate(
    chain: &mut VecDeque<String>,
    model: &mut CatalogEntry,
    endpoint: &mut Endpoint,
    state: &AppState,
    catalog: &PriceCatalog,
    local_port: Option<u16>,
    ledger: &mut RunLedger,
    reason: &str,
) -> AppResult<()> {
    let Some(next_id) = chain.pop_front() else {
        return Err(AppError::new(
            "model_insufficient",
            format!("{} could not complete this request ({reason}), and no stronger model is available. No files were changed.", model.display_name),
        )
        .with_recovery("Connect a more capable provider on the Models page, or make the request more specific."));
    };
    let next = catalog
        .get(&next_id)
        .cloned()
        .ok_or_else(|| AppError::internal("escalation model missing from catalog"))?;
    *endpoint = endpoint_for(state, &next, local_port)?;
    ledger.escalations.push(Escalation {
        from_model_id: model.model_id.clone(),
        to_model_id: next.model_id.clone(),
        reason: reason.to_string(),
    });
    ledger.step(
        "escalate",
        format!("{} → {}: {reason}", model.display_name, next.display_name),
        0,
        None,
    );
    *model = next;
    Ok(())
}

/// Reads one model-requested file or range after validating it. Returns the
/// block to send and a record; refusals are explained to the model.
fn read_for_model(
    root: &Path,
    inventory: &Inventory,
    request: &FileRequest,
    budget_left: usize,
    max_file_tokens: usize,
    leaves_machine: bool,
) -> (String, RetrievalRecord) {
    let path = request
        .path
        .trim()
        .trim_start_matches("./")
        .replace('\\', "/");
    let refuse = |reason: String| {
        (
            format!("<file path=\"{path}\">REFUSED: {reason}</file>"),
            RetrievalRecord {
                path: path.clone(),
                from_line: 0,
                to_line: 0,
                tokens: 0,
                refused: Some(reason),
                redacted_lines: 0,
            },
        )
    };

    let absolute: PathBuf = match resolve_within_root(root, &path) {
        Ok(absolute) => absolute,
        Err(_) => return refuse("path is outside the project".to_string()),
    };
    let Some(entry) = inventory.get(&path) else {
        return refuse("not in the project index (ignored, generated or does not exist); use search to find files".to_string());
    };
    if !entry.selectable || entry.class != FileClass::SourceText {
        let why = entry
            .exclusion
            .as_ref()
            .map(|e| e.reason.clone())
            .unwrap_or_else(|| format!("{} files are not shared", entry.class.label()));
        return refuse(why);
    }
    if budget_left < 200 {
        return refuse("retrieval budget exhausted".to_string());
    }
    let text = match std::fs::read_to_string(&absolute) {
        Ok(text) => text,
        Err(_) => return refuse("could not be read as UTF-8 text".to_string()),
    };

    let lines: Vec<&str> = text.lines().collect();
    let total = lines.len();
    let from = request.start_line.unwrap_or(1).max(1).min(total.max(1));
    let to = request
        .end_line
        .unwrap_or(total)
        .clamp(from, total.max(from));
    let cap = max_file_tokens.min(budget_left);

    let mut body = String::new();
    let mut used = 0usize;
    let mut last = from.saturating_sub(1);
    let mut redacted = 0usize;
    let sensitive_lines: HashSet<usize> = if leaves_machine {
        scan_text(&path, &text, 200)
            .into_iter()
            .filter(|f| matches!(f.confidence, Confidence::High | Confidence::Medium))
            .map(|f| f.line)
            .collect()
    } else {
        HashSet::new()
    };
    for (index, line) in lines.iter().enumerate().take(to).skip(from - 1) {
        let line_no = index + 1;
        let shown = if sensitive_lines.contains(&line_no) {
            redacted += 1;
            "[line redacted by LeanAI: possible credential]"
        } else {
            line
        };
        let cost = tokenizer::estimate(shown).value + 1;
        if used + cost > cap && line_no > from {
            break;
        }
        body.push_str(shown);
        body.push('\n');
        used += cost;
        last = line_no;
    }
    let note = if last < to {
        format!(
            "\n(truncated at line {last} of {total}; request startLine {} to continue)",
            last + 1
        )
    } else {
        String::new()
    };
    let block = format!(
        "<file path=\"{path}\" lines=\"{from}-{last}\" total_lines=\"{total}\">\n{body}</file>{note}"
    );
    let tokens = tokenizer::estimate(&block).value;
    (
        block,
        RetrievalRecord {
            path,
            from_line: from,
            to_line: last,
            tokens,
            refused: None,
            redacted_lines: redacted,
        },
    )
}

/// Case-insensitive literal search over shareable project text, bounded by
/// match count and file size. Runs off the async runtime.
async fn search_project(
    root: PathBuf,
    inventory: Arc<Inventory>,
    query: String,
    prefix: Option<String>,
) -> (String, usize) {
    tauri::async_runtime::spawn_blocking(move || {
        let needle = query.to_lowercase();
        let prefix = prefix
            .map(|p| p.trim().trim_start_matches("./").to_string())
            .unwrap_or_default();
        let mut out = Vec::new();
        'files: for entry in inventory.selectable() {
            if entry.class != FileClass::SourceText
                || entry.size_bytes > MAX_SEARCH_FILE_BYTES
                || !entry.path.starts_with(&prefix)
            {
                continue;
            }
            let Ok(absolute) = resolve_within_root(&root, &entry.path) else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(absolute) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    let snippet: String = line.trim().chars().take(200).collect();
                    out.push(format!("{}:{}: {snippet}", entry.path, index + 1));
                    if out.len() >= MAX_SEARCH_MATCHES {
                        break 'files;
                    }
                }
            }
        }
        let count = out.len();
        let text = if out.is_empty() {
            format!("<search query=\"{query}\">no matches</search>")
        } else {
            format!(
                "<search query=\"{query}\" matches=\"{count}\"{}>\n{}\n</search>",
                if count >= MAX_SEARCH_MATCHES {
                    " truncated=\"true\""
                } else {
                    ""
                },
                out.join("\n")
            )
        };
        (text, count)
    })
    .await
    .unwrap_or_else(|_| ("<search>failed</search>".to_string(), 0))
}

#[allow(clippy::too_many_arguments)]
fn record_run(
    state: &AppState,
    run_id: &str,
    prompt: &str,
    started: i64,
    status: &str,
    ledger: &RunLedger,
    analysis: &PromptAnalysis,
    budget: &ContextBudget,
    selection: &ModelSelection,
    verdict: Option<&ValidatorVerdict>,
    decision: Option<&ApplyDecision>,
) {
    let Some(session) = state.session() else {
        return;
    };
    let record = RunRecord {
        id: run_id.to_string(),
        project_id: session.record.id.clone(),
        task: prompt.to_string(),
        mode: "prompt".to_string(),
        context_manifest: serde_json::json!({
            "budget": budget,
            "retrievals": ledger.retrievals,
            "searches": ledger.searches,
            "metrics": ledger.metrics,
        }),
        // The apply decision is kept with the run so its reasons can later be
        // compared with what the user actually approved or discarded.
        policy: serde_json::json!({
            "analysis": analysis,
            "selection": selection,
            "escalations": ledger.escalations,
            "applyDecision": decision,
        }),
        status: status.to_string(),
        budget: serde_json::to_value(budget).ok(),
        started_at_ms: started,
        ended_at_ms: (status != "awaiting_approval").then(|| now_ms() as i64),
        validation_state: verdict.and_then(|v| serde_json::to_value(v).ok()),
    };
    // Recording is best effort: a ledger failure must not hide the result.
    let _ = state.with_db(|conn| {
        upsert_run(conn, &record)?;
        for (index, step) in ledger.trace.iter().enumerate() {
            append_run_event(
                conn,
                run_id,
                index as i64,
                "prompt_step",
                &serde_json::to_value(step).unwrap_or(serde_json::Value::Null),
            )?;
        }
        Ok(())
    });
}
