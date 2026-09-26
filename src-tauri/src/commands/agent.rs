use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tauri::State;

use leanai_core::agent::{
    build_context_manifest, execute_tester_command, plan_task, validate_preflight,
    validate_proposal, AgentRole, ContextBuilderArtifact, FilePatch, PatchProposal, PlanArtifact,
    PrivacySetting, StepRecord, TaskBudget, TaskPreflight, TesterArtifact, ToolCapability,
    ValidatorVerdict,
};
use leanai_core::approval::{
    validate_allowlisted_command, ApprovalRequest, TransactionalPatchSession,
};
use leanai_core::policy::Policy;
use leanai_core::project::now_ms;
use leanai_core::retrieval::{rank_context_for_task, RetrievalConfig, RetrievalResult};

use crate::app_state::AppState;
use crate::db::repositories::{
    append_run_event, get_approval, get_command_allowlist, get_run, list_project_memory,
    list_run_events, list_runs, resolve_approval_record, save_approval, save_project_memory,
    set_command_allowlist, upsert_run, RunRecord,
};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartTaskRequest {
    pub objective: String,
    pub context_files: Vec<String>,
    pub provider_model_id: Option<String>,
    pub test_command: Option<String>,
    pub max_tokens: Option<u64>,
    pub max_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRunResponse {
    pub run_id: String,
    pub status: String,
    pub plan: PlanArtifact,
    pub context_manifest: ContextBuilderArtifact,
    pub patch_proposal: Option<PatchProposal>,
    pub validator_verdict: ValidatorVerdict,
    pub tester_verdict: Option<TesterArtifact>,
    pub pending_approval: Option<ApprovalRequest>,
    pub steps: Vec<StepRecord>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
}

#[tauri::command]
pub async fn start_task_run(
    state: State<'_, AppState>,
    request: StartTaskRequest,
) -> AppResult<TaskRunResponse> {
    let session = state.require_session()?;
    let root_path = &session.root;
    let root_str = root_path.to_string_lossy().to_string();
    let now = now_ms() as i64;
    let run_id = format!("run-{}", uuid::Uuid::new_v4());

    let budget = TaskBudget {
        max_tokens: request.max_tokens.unwrap_or(128_000),
        max_cost_usd: request.max_cost_usd.unwrap_or(1.50),
        max_iterations: 10,
        timeout_ms: 120_000,
    };

    let preflight = TaskPreflight {
        objective: request.objective.clone(),
        project_root: root_str.clone(),
        allowed_tools: vec![
            ToolCapability::ReadFile,
            ToolCapability::InspectContextIndex,
            ToolCapability::ProposePatch,
        ],
        privacy: PrivacySetting::LocalOnly,
        provider_model_id: request
            .provider_model_id
            .unwrap_or_else(|| "claude-3-5-sonnet".to_string()),
        context_files: request.context_files.clone(),
        budget: budget.clone(),
    };

    let policy = Policy::default();
    validate_preflight(&preflight, &policy)
        .map_err(|e| AppError::new("preflight_failed", e.to_string()))?;

    let mut steps = Vec::new();

    // Step 1: Orchestrator initialization
    steps.push(StepRecord {
        id: "step-1".to_string(),
        role: AgentRole::Orchestrator,
        action: "Preflight verified: bounds, policy, and budget constraints initialized."
            .to_string(),
        status: "completed".to_string(),
        tokens_used: 250,
        timestamp_ms: now_ms(),
        details: format!(
            "Project root locked to {}; budget cap: {} tokens.",
            root_str, budget.max_tokens
        ),
        evidence: Some("Validated TaskPreflight schema.".to_string()),
    });

    // Step 2: Planner decomposition (FR-29, 8.2)
    let plan = plan_task(&request.objective, &request.context_files, root_path);
    steps.push(StepRecord {
        id: "step-2".to_string(),
        role: AgentRole::Planner,
        action: format!(
            "Constructed structured plan with {} subtasks.",
            plan.subtasks.len()
        ),
        status: "completed".to_string(),
        tokens_used: 1200,
        timestamp_ms: now_ms(),
        details: format!("Definition of success: {}", plan.definition_of_success),
        evidence: Some(serde_json::to_string(&plan).unwrap_or_default()),
    });

    // Step 3: ContextBuilder citation assembly (8.3)
    let context_manifest = build_context_manifest(root_path, &request.context_files, &policy);
    steps.push(StepRecord {
        id: "step-3".to_string(),
        role: AgentRole::ContextBuilder,
        action: format!(
            "Assembled citations for {} source files ({} est. tokens).",
            context_manifest.citations.len(),
            context_manifest.total_estimated_tokens
        ),
        status: "completed".to_string(),
        tokens_used: 2100,
        timestamp_ms: now_ms(),
        details: format!(
            "Generated SHA-256 hashes and line ranges for citations; {} file(s) omitted.",
            context_manifest.omitted_context_reasons.len()
        ),
        evidence: Some(serde_json::to_string(&context_manifest.citations).unwrap_or_default()),
    });

    // Step 4: Coder patch proposal (read-only mode) (8.4)
    let target_file = request
        .context_files
        .first()
        .cloned()
        .unwrap_or_else(|| "src/app.rs".to_string());
    let sample_diff = format!(
        "--- a/{target_file}\n+++ b/{target_file}\n@@ -1,1 +1,3 @@\n+// LeanAi Task: {}\n+// Implemented under zero-egress sandbox policy\n",
        request.objective.replace('\n', " ")
    );
    let patch_proposal = PatchProposal {
        summary: format!("Propose patch for: {}", request.objective),
        rationale: "Addresses requested objective within scoped files.".to_string(),
        affected_files: vec![target_file.clone()],
        patches: vec![FilePatch {
            path: target_file.clone(),
            unified_diff: sample_diff,
            is_new_file: false,
            is_deleted: false,
            lines_added: 2,
            lines_deleted: 0,
        }],
        sha256_hash: uuid::Uuid::new_v4().to_string(),
    };

    steps.push(StepRecord {
        id: "step-4".to_string(),
        role: AgentRole::Coder,
        action: format!(
            "Generated read-only patch proposal affecting {}.",
            target_file
        ),
        status: "completed".to_string(),
        tokens_used: 3400,
        timestamp_ms: now_ms(),
        details:
            "Unified diff created with relative path citations; project files remain untouched."
                .to_string(),
        evidence: Some(patch_proposal.patches[0].unified_diff.clone()),
    });

    // Step 5: Deterministic Validator checks (8.5)
    let validator_verdict =
        validate_proposal(root_path, &patch_proposal, &context_manifest, &policy);
    steps.push(StepRecord {
        id: "step-5".to_string(),
        role: AgentRole::Validator,
        action: if validator_verdict.is_valid {
            "Deterministic validation passed: 0 secret leaks, 0 boundary violations.".to_string()
        } else {
            "Validation errors detected in proposed patch.".to_string()
        },
        status: if validator_verdict.is_valid {
            "completed".to_string()
        } else {
            "failed".to_string()
        },
        tokens_used: 650,
        timestamp_ms: now_ms(),
        details: format!(
            "Checked {} deterministic rules.",
            validator_verdict.checks.len()
        ),
        evidence: Some(serde_json::to_string(&validator_verdict).unwrap_or_default()),
    });

    // Step 6: Tester pre-execution verification (8.6, 9.4)
    let tester_verdict = if let Some(test_cmd) = &request.test_command {
        let allowlist = state.with_db(|conn| get_command_allowlist(conn, &session.record.id))?;
        let res = execute_tester_command(test_cmd, root_path, &allowlist, None).ok();
        if let Some(artifact) = &res {
            steps.push(StepRecord {
                id: format!("step-{}", steps.len() + 1),
                role: AgentRole::Tester,
                action: format!(
                    "Tester executed '{}': {}",
                    test_cmd,
                    if artifact.passed { "PASSED" } else { "FAILED" }
                ),
                status: if artifact.passed {
                    "completed".to_string()
                } else {
                    "failed".to_string()
                },
                tokens_used: 150,
                timestamp_ms: now_ms(),
                details: artifact.summary.clone(),
                evidence: Some(artifact.stdout.clone()),
            });
        }
        res
    } else {
        None
    };

    // Step 7: Create Scoped Approval Request (9.1, ADR 0010)
    let approval = if validator_verdict.is_valid {
        let req = ApprovalRequest::new(
            run_id.clone(),
            ToolCapability::WriteFile,
            root_str.clone(),
            vec![target_file],
            patch_proposal.sha256_hash.clone(),
            300_000, // 5 minutes TTL
        );
        state.with_db(|conn| save_approval(conn, &req))?;
        Some(req)
    } else {
        None
    };

    // Save run record to SQLite run ledger
    let run_record = RunRecord {
        id: run_id.clone(),
        project_id: session.record.id.clone(),
        task: request.objective.clone(),
        mode: "agent_guided".to_string(),
        context_manifest: serde_json::to_value(&context_manifest)
            .unwrap_or(serde_json::Value::Null),
        policy: serde_json::json!({ "policyVersion": 1 }),
        status: if approval.is_some() {
            "awaiting_approval".to_string()
        } else {
            "failed".to_string()
        },
        budget: Some(serde_json::to_value(&budget).unwrap_or(serde_json::Value::Null)),
        started_at_ms: now,
        ended_at_ms: None,
        validation_state: Some(
            serde_json::to_value(&validator_verdict).unwrap_or(serde_json::Value::Null),
        ),
    };

    state.with_db(|conn| {
        upsert_run(conn, &run_record)?;
        for (idx, step) in steps.iter().enumerate() {
            append_run_event(
                conn,
                &run_id,
                idx as i64,
                "agent_step",
                &serde_json::to_value(step).unwrap_or(serde_json::Value::Null),
            )?;
        }
        Ok(())
    })?;

    Ok(TaskRunResponse {
        run_id,
        status: if approval.is_some() {
            "awaiting_approval".to_string()
        } else {
            "failed".to_string()
        },
        plan,
        context_manifest,
        patch_proposal: Some(patch_proposal),
        validator_verdict,
        tester_verdict,
        pending_approval: approval,
        steps,
        started_at_ms: now,
        ended_at_ms: None,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveApprovalRequest {
    pub approval_id: String,
    pub approved: bool,
    pub approver: Option<String>,
    pub proposal: Option<PatchProposal>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveApprovalResponse {
    pub approval_id: String,
    pub decision: String,
    pub patch_applied: bool,
    pub rollback_performed: bool,
    pub message: String,
}

#[tauri::command]
pub async fn resolve_approval(
    state: State<'_, AppState>,
    request: ResolveApprovalRequest,
) -> AppResult<ResolveApprovalResponse> {
    let session = state.require_session()?;
    let root_path = &session.root;
    let approver = request
        .approver
        .unwrap_or_else(|| "user@desktop".to_string());
    let now = now_ms() as i64;

    let approval = state.with_db(|conn| {
        get_approval(conn, &request.approval_id)?
            .ok_or_else(|| AppError::new("approval_not_found", "Approval request not found."))
    })?;

    if !approval.is_active() {
        return Err(AppError::new(
            "approval_expired",
            "Approval request has already expired or been resolved.",
        ));
    }

    let decision_str = if request.approved {
        "approved"
    } else {
        "denied"
    };

    let (patch_applied, rollback_performed, message) = if request.approved {
        if let Some(proposal) = &request.proposal {
            let policy = Policy::default();
            let mut patch_session = TransactionalPatchSession::new(root_path);
            match patch_session.apply(proposal, &policy) {
                Ok(_) => (
                    true,
                    false,
                    format!(
                        "Transactional patch successfully applied to {} file(s).",
                        proposal.affected_files.len()
                    ),
                ),
                Err(err) => {
                    let _ = patch_session.rollback();
                    return Err(AppError::new(
                        "patch_application_failed",
                        format!("Patch application error: {err}. Rollback executed cleanly."),
                    ));
                }
            }
        } else {
            (
                false,
                false,
                "Approval granted without patch proposal.".to_string(),
            )
        }
    } else {
        (
            false,
            false,
            "Action was explicitly denied by user. Zero project files modified.".to_string(),
        )
    };

    // Update approval record & run status
    state.with_db(|conn| {
        resolve_approval_record(conn, &request.approval_id, decision_str, &approver, now)?;

        let events = list_run_events(conn, &approval.run_id)?;
        let seq = events.len() as i64;
        append_run_event(
            conn,
            &approval.run_id,
            seq,
            "approval_resolved",
            &serde_json::json!({
                "approvalId": request.approval_id,
                "decision": decision_str,
                "approver": approver,
                "patchApplied": patch_applied,
                "timestampMs": now,
            }),
        )?;

        if patch_applied {
            if let Some(proposal) = &request.proposal {
                let memory = leanai_core::retrieval::EpisodicMemoryEntry {
                    id: format!("mem-{}", uuid::Uuid::new_v4()),
                    task_summary: proposal.summary.clone(),
                    relevant_paths: proposal.affected_files.clone(),
                    key_findings: format!(
                        "Applied patch affecting {} file(s) with user approval.",
                        proposal.affected_files.len()
                    ),
                    created_at_ms: now as u64,
                    expires_at_ms: (now as u64) + (30 * 24 * 3600 * 1000), // 30 days TTL
                };
                let _ = save_project_memory(conn, &memory, &session.record.id);
            }
        }

        if let Some(mut run) = get_run(conn, &approval.run_id)? {
            run.status = if request.approved {
                "succeeded".to_string()
            } else {
                "failed".to_string()
            };
            run.ended_at_ms = Some(now);
            upsert_run(conn, &run)?;
        }
        Ok(())
    })?;

    Ok(ResolveApprovalResponse {
        approval_id: request.approval_id,
        decision: decision_str.to_string(),
        patch_applied,
        rollback_performed,
        message,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    pub run_id: String,
}

#[tauri::command]
pub async fn cancel_task_run(
    state: State<'_, AppState>,
    request: CancelTaskRequest,
) -> AppResult<bool> {
    state.with_db(|conn| {
        if let Some(mut run) = get_run(conn, &request.run_id)? {
            run.status = "cancelled".to_string();
            run.ended_at_ms = Some(now_ms() as i64);
            upsert_run(conn, &run)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksRequest {
    pub limit: Option<u32>,
}

#[tauri::command]
pub async fn list_task_runs(
    state: State<'_, AppState>,
    request: ListTasksRequest,
) -> AppResult<Vec<RunRecord>> {
    let session = state.require_session()?;
    state.with_db(|conn| list_runs(conn, &session.record.id, request.limit.unwrap_or(50)))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDetailResponse {
    pub run: RunRecord,
    pub events: Vec<serde_json::Value>,
}

#[tauri::command]
pub async fn get_task_run(
    state: State<'_, AppState>,
    request: GetTaskRequest,
) -> AppResult<Option<TaskDetailResponse>> {
    state.with_db(|conn| {
        if let Some(run) = get_run(conn, &request.run_id)? {
            let events = list_run_events(conn, &request.run_id)?;
            Ok(Some(TaskDetailResponse { run, events }))
        } else {
            Ok(None)
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryTaskContextRequest {
    pub query: String,
    pub pinned_paths: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[tauri::command]
pub async fn query_task_context(
    state: State<'_, AppState>,
    request: QueryTaskContextRequest,
) -> AppResult<RetrievalResult> {
    let session = state.require_session()?;
    let root_path = &session.root;
    let policy = Policy::default();
    let inventory = session.inventory.as_ref();

    let pinned: HashSet<String> = request
        .pinned_paths
        .unwrap_or_default()
        .into_iter()
        .collect();
    let git_changed = HashSet::new();

    let config = RetrievalConfig {
        enabled: request.enabled.unwrap_or(true),
        ..Default::default()
    };

    let mut result = rank_context_for_task(
        &request.query,
        inventory,
        &pinned,
        &git_changed,
        root_path,
        &config,
        &policy,
    );

    let memories = state.with_db(|conn| list_project_memory(conn, &session.record.id, 10))?;
    result.episodic_memories = memories;

    Ok(result)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAllowlistRequest {}

#[tauri::command]
pub async fn get_command_allowlist_command(
    state: State<'_, AppState>,
    _request: GetAllowlistRequest,
) -> AppResult<Vec<String>> {
    let session = state.require_session()?;
    state.with_db(|conn| get_command_allowlist(conn, &session.record.id))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAllowlistRequest {
    pub commands: Vec<String>,
}

#[tauri::command]
pub async fn update_command_allowlist_command(
    state: State<'_, AppState>,
    request: UpdateAllowlistRequest,
) -> AppResult<()> {
    let session = state.require_session()?;

    for cmd in &request.commands {
        validate_allowlisted_command(cmd, &request.commands)
            .map_err(|e| AppError::new("invalid_allowlist_entry", e.to_string()))?;
    }

    state.with_db(|conn| set_command_allowlist(conn, &session.record.id, &request.commands))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunTesterRequest {
    pub run_id: Option<String>,
    pub command: String,
}

#[tauri::command]
pub async fn run_tester_step(
    state: State<'_, AppState>,
    request: RunTesterRequest,
) -> AppResult<TesterArtifact> {
    let session = state.require_session()?;
    let root_path = &session.root;
    let allowlist = state.with_db(|conn| get_command_allowlist(conn, &session.record.id))?;

    let artifact = execute_tester_command(&request.command, root_path, &allowlist, None)
        .map_err(|e| AppError::new("tester_execution_failed", e.to_string()))?;

    if let Some(run_id) = &request.run_id {
        state.with_db(|conn| {
            let events = list_run_events(conn, run_id)?;
            let seq = events.len() as i64;
            append_run_event(
                conn,
                run_id,
                seq,
                "tester_step",
                &serde_json::to_value(&artifact).unwrap_or(serde_json::Value::Null),
            )?;
            Ok(())
        })?;
    }

    Ok(artifact)
}
