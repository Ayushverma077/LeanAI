//! Local model runtime and cloud provider commands (Phases 6 and 7).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

use leanai_core::catalog::PriceCatalog;
use leanai_core::provider::{CachedTokenPolicy, CapabilityProfile, TokenCountResult};
use leanai_core::routing::{
    route_task as core_route_task, RoutingDecision, RoutingPolicy, TaskClass,
};
use leanai_core::tokenizer::EstimateKind;

use crate::app_state::AppState;
use crate::db::repositories::{self, ModelRecord, ProviderConfigRecord};
use crate::error::{AppError, AppResult};
use crate::sidecar_manager::SidecarStatus;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterLocalModelRequest {
    pub display_name: String,
    pub file_path: String,
    pub context_cap: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnregisterModelRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartLocalModelRequest {
    pub id: String,
    pub context_size: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureCredentialRequest {
    pub provider_id: String,
    pub account_label: String,
    pub secret: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectProviderRequest {
    pub provider_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckProviderStatusRequest {
    pub provider_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatusResponse {
    pub provider_id: String,
    pub is_configured: bool,
    pub is_stale_catalog: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteTaskRequest {
    pub task_class: TaskClass,
    pub estimated_tokens: usize,
    pub budget_usd: Option<f64>,
    pub require_local_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateTokensRequest {
    pub text: String,
    pub provider_id: String,
    pub model_name: String,
    pub exact_count_opt_in: bool,
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> AppResult<Vec<ModelRecord>> {
    state.with_db(|conn| repositories::list_models(conn))
}

#[tauri::command]
pub async fn register_local_model(
    state: State<'_, AppState>,
    request: RegisterLocalModelRequest,
) -> AppResult<ModelRecord> {
    let path = Path::new(&request.file_path);
    if !path.is_file() {
        return Err(AppError::new(
            "model_not_found",
            format!("GGUF model file not found: `{}`", request.file_path),
        )
        .with_recovery("Provide a valid path to an existing .gguf file."));
    }

    let header = leanai_core::gguf::inspect_gguf_file(path)?;

    let record = ModelRecord {
        id: uuid::Uuid::new_v4().to_string(),
        kind: "local".to_string(),
        display_name: request.display_name,
        source: format!("gguf:{}", header.architecture),
        version: format!("v{}", header.version),
        file_path: Some(request.file_path),
        capability_profile: CapabilityProfile {
            context_cap: request.context_cap.unwrap_or(8192),
            streaming: true,
            tool_calling: false,
            structured_output: false,
            vision: false,
            exact_token_counting: false,
            cached_token_policy: CachedTokenPolicy::None,
        },
        checksum: Some(header.checksum_sha256),
        status: "ready".to_string(),
        created_at_ms: leanai_core::project::now_ms() as i64,
        updated_at_ms: leanai_core::project::now_ms() as i64,
    };

    state.with_db(|conn| repositories::upsert_model(conn, &record))
}

#[tauri::command]
pub async fn unregister_model(
    state: State<'_, AppState>,
    request: UnregisterModelRequest,
) -> AppResult<bool> {
    if let SidecarStatus::Ready { model_id, .. } = state.sidecar.status() {
        if model_id == request.id {
            state.sidecar.stop()?;
        }
    }
    state.with_db(|conn| repositories::delete_model(conn, &request.id))
}

#[tauri::command]
pub async fn start_local_model(
    state: State<'_, AppState>,
    request: StartLocalModelRequest,
) -> AppResult<SidecarStatus> {
    let model = state
        .with_db(|conn| repositories::get_model(conn, &request.id))?
        .ok_or_else(|| AppError::new("model_not_found", "Model not registered."))?;

    let file_path = model
        .file_path
        .as_deref()
        .ok_or_else(|| AppError::new("invalid_model", "Selected model has no local file path."))?;

    state.sidecar.start(
        &model.id,
        &model.display_name,
        Path::new(file_path),
        request.context_size,
    )
}

#[tauri::command]
pub async fn stop_local_model(state: State<'_, AppState>) -> AppResult<SidecarStatus> {
    state.sidecar.stop()
}

#[tauri::command]
pub async fn local_model_status(state: State<'_, AppState>) -> AppResult<SidecarStatus> {
    Ok(state.sidecar.status())
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> AppResult<Vec<ProviderConfigRecord>> {
    let current = state.with_db(|conn| repositories::list_provider_configs(conn))?;
    if !current.is_empty() {
        return Ok(current);
    }

    // Seed initial provider records
    let defaults = [
        ("openai", "OpenAI Cloud", "api_key", "2026-09-01"),
        ("anthropic", "Anthropic Cloud", "api_key", "2026-09-01"),
        ("mock", "Mock Offline Provider", "test_key", "2026-09-01"),
    ];

    state.with_db(|conn| {
        for (id, name, label, cat_ver) in defaults {
            let is_configured = state.keychain.is_configured(id).unwrap_or(false);
            repositories::set_provider_config(conn, id, name, label, is_configured, cat_ver, true)?;
        }
        repositories::list_provider_configs(conn)
    })
}

#[tauri::command]
pub async fn configure_provider_credential(
    state: State<'_, AppState>,
    request: ConfigureCredentialRequest,
) -> AppResult<ProviderConfigRecord> {
    if request.secret.trim().is_empty() {
        return Err(AppError::new(
            "invalid_credential",
            "Credential cannot be blank.",
        ));
    }

    // Store in OS keychain (never in SQLite)
    state.keychain.store(
        &request.provider_id,
        &request.account_label,
        &request.secret,
    )?;

    let display_name = match request.provider_id.as_str() {
        "openai" => "OpenAI Cloud",
        "anthropic" => "Anthropic Cloud",
        "mock" => "Mock Offline Provider",
        _ => &request.provider_id,
    };

    state.with_db(|conn| {
        repositories::set_provider_config(
            conn,
            &request.provider_id,
            display_name,
            &request.account_label,
            true,
            "2026-09-01",
            true,
        )
    })
}

#[tauri::command]
pub async fn disconnect_provider(
    state: State<'_, AppState>,
    request: DisconnectProviderRequest,
) -> AppResult<ProviderConfigRecord> {
    // Delete from OS keychain
    let _ = state.keychain.delete(&request.provider_id);

    let display_name = match request.provider_id.as_str() {
        "openai" => "OpenAI Cloud",
        "anthropic" => "Anthropic Cloud",
        "mock" => "Mock Offline Provider",
        _ => &request.provider_id,
    };

    state.with_db(|conn| {
        repositories::set_provider_config(
            conn,
            &request.provider_id,
            display_name,
            "api_key",
            false,
            "2026-09-01",
            true,
        )
    })
}

#[tauri::command]
pub async fn check_provider_status(
    state: State<'_, AppState>,
    request: CheckProviderStatusRequest,
) -> AppResult<ProviderStatusResponse> {
    let is_configured = state
        .keychain
        .is_configured(&request.provider_id)
        .unwrap_or(false);
    let catalog = PriceCatalog::default();

    Ok(ProviderStatusResponse {
        provider_id: request.provider_id,
        is_configured,
        is_stale_catalog: catalog.is_stale(30, leanai_core::project::now_ms()),
    })
}

#[tauri::command]
pub async fn get_model_catalog() -> AppResult<PriceCatalog> {
    Ok(PriceCatalog::default())
}

#[tauri::command]
pub async fn route_task(request: RouteTaskRequest) -> AppResult<RoutingDecision> {
    let catalog = PriceCatalog::default();
    let mut policy = RoutingPolicy::default();
    if let Some(b) = request.budget_usd {
        policy.max_budget_usd = Some(b);
    }
    if let Some(local_only) = request.require_local_only {
        policy.require_local_only = local_only;
    }

    core_route_task(
        request.task_class,
        request.estimated_tokens,
        &policy,
        &catalog,
    )
    .map_err(AppError::from)
}

#[tauri::command]
pub async fn estimate_provider_tokens(
    request: EstimateTokensRequest,
) -> AppResult<TokenCountResult> {
    if request.exact_count_opt_in {
        Ok(TokenCountResult {
            count: request.text.split_whitespace().count(),
            estimate_kind: EstimateKind::ProviderExact {
                provider: request.provider_id,
                model: request.model_name,
            },
        })
    } else {
        let bpe = leanai_core::tokenizer::estimate(&request.text);
        Ok(TokenCountResult {
            count: bpe.value,
            estimate_kind: bpe.kind,
        })
    }
}
