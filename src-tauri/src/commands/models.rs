//! Local model runtime and cloud provider commands (Phases 6 and 7).

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;

use leanai_core::catalog::{CatalogEntry, ModelTier, PriceCatalog, SELF_HOSTED_PROVIDER};
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
pub async fn get_model_catalog(state: State<'_, AppState>) -> AppResult<PriceCatalog> {
    effective_catalog(&state)
}

// ---------------------------------------------------------------------------
// Self-hosted models: user-run OpenAI-compatible servers (Ollama, vLLM,
// LM Studio, llama.cpp) on this or another machine. Stored under their own
// settings key; API keys, when a server needs one, live in the keychain.
// ---------------------------------------------------------------------------

const SELF_HOSTED_KEY: &str = "self_hosted_models.v1";

/// A user-registered model behind an OpenAI-compatible API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfHostedModel {
    /// Stable slug; the catalog ID is `custom/{id}`.
    pub id: String,
    pub display_name: String,
    /// What the user entered, e.g. `http://192.168.1.20:11434`.
    pub base_url: String,
    /// Model name the server expects, e.g. `qwen3:14b`.
    pub model: String,
    /// How capable the user judges it; drives routing.
    pub tier: ModelTier,
    pub context_cap: usize,
    #[serde(default)]
    pub has_api_key: bool,
}

impl SelfHostedModel {
    fn keychain_id(&self) -> String {
        format!("{SELF_HOSTED_PROVIDER}-{}", self.id)
    }

    fn catalog_entry(&self) -> CatalogEntry {
        CatalogEntry::self_hosted(
            &self.id,
            &self.display_name,
            &self.model,
            self.context_cap,
            self.tier,
        )
    }
}

pub fn load_self_hosted(state: &AppState) -> AppResult<Vec<SelfHostedModel>> {
    let raw = state.with_db(|conn| repositories::get_setting(conn, SELF_HOSTED_KEY))?;
    Ok(raw
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default())
}

pub fn store_self_hosted(state: &AppState, models: &[SelfHostedModel]) -> AppResult<()> {
    let json = serde_json::to_string(models).map_err(|e| AppError::internal(e.to_string()))?;
    state.with_db(|conn| repositories::set_setting(conn, SELF_HOSTED_KEY, &json))
}

/// Built-in catalog plus the user's self-hosted models. This is the catalog
/// the prompt router chooses from.
pub(crate) fn effective_catalog(state: &AppState) -> AppResult<PriceCatalog> {
    let mut catalog = PriceCatalog::default_catalog();
    catalog.entries.extend(
        load_self_hosted(state)?
            .iter()
            .map(SelfHostedModel::catalog_entry),
    );
    Ok(catalog)
}

/// Endpoint for a self-hosted catalog entry (`custom/{id}`).
pub(crate) fn self_hosted_endpoint(
    state: &AppState,
    model_id: &str,
) -> AppResult<crate::llm_client::Endpoint> {
    let id = model_id
        .strip_prefix(&format!("{SELF_HOSTED_PROVIDER}/"))
        .unwrap_or(model_id);
    let model = load_self_hosted(state)?
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(|| AppError::new("model_not_found", "That self-hosted model was removed."))?;
    let api_key = if model.has_api_key {
        state.keychain.read(&model.keychain_id())?
    } else {
        None
    };
    Ok(crate::llm_client::Endpoint::SelfHosted {
        url: crate::llm_client::chat_completions_url(&model.base_url)?,
        api_key,
    })
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out
        .trim_end_matches('-')
        .chars()
        .take(32)
        .collect::<String>();
    if out.is_empty() {
        "model".to_string()
    } else {
        out
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSelfHostedModelRequest {
    /// Present when editing an existing entry.
    pub id: Option<String>,
    pub display_name: String,
    pub base_url: String,
    pub model: String,
    pub tier: ModelTier,
    pub context_cap: Option<usize>,
    /// New key to store. `None` keeps the current one.
    pub api_key: Option<String>,
    #[serde(default)]
    pub clear_api_key: bool,
}

#[tauri::command]
pub async fn list_self_hosted_models(
    state: State<'_, AppState>,
) -> AppResult<Vec<SelfHostedModel>> {
    load_self_hosted(&state)
}

#[tauri::command]
pub async fn save_self_hosted_model(
    state: State<'_, AppState>,
    request: SaveSelfHostedModelRequest,
) -> AppResult<SelfHostedModel> {
    let display_name = request.display_name.trim().to_string();
    let model_name = request.model.trim().to_string();
    if display_name.is_empty() || model_name.is_empty() {
        return Err(AppError::new(
            "invalid_model",
            "Enter a display name and the model name the server expects.",
        ));
    }
    // Validates the address before anything is stored.
    crate::llm_client::chat_completions_url(&request.base_url)?;
    let context_cap = request
        .context_cap
        .unwrap_or(32_768)
        .clamp(2_048, 2_000_000);

    let mut models = load_self_hosted(&state)?;
    let id = match &request.id {
        Some(id) if models.iter().any(|m| &m.id == id) => id.clone(),
        Some(_) => {
            return Err(AppError::new(
                "model_not_found",
                "That model no longer exists.",
            ))
        }
        None => {
            let base = slug(&display_name);
            let mut candidate = base.clone();
            let mut n = 2;
            while models.iter().any(|m| m.id == candidate) {
                candidate = format!("{base}-{n}");
                n += 1;
            }
            candidate
        }
    };
    let previous_key = models
        .iter()
        .find(|m| m.id == id)
        .is_some_and(|m| m.has_api_key);
    let mut model = SelfHostedModel {
        id,
        display_name,
        base_url: request.base_url.trim().to_string(),
        model: model_name,
        tier: request.tier,
        context_cap,
        has_api_key: previous_key,
    };

    match request.api_key.as_deref().map(str::trim) {
        Some(key) if !key.is_empty() => {
            state.keychain.store(&model.keychain_id(), "api_key", key)?;
            model.has_api_key = true;
        }
        _ if request.clear_api_key => {
            let _ = state.keychain.delete(&model.keychain_id());
            model.has_api_key = false;
        }
        _ => {}
    }

    models.retain(|m| m.id != model.id);
    models.push(model.clone());
    store_self_hosted(&state, &models)?;
    Ok(model)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfHostedModelIdRequest {
    pub id: String,
}

#[tauri::command]
pub async fn delete_self_hosted_model(
    state: State<'_, AppState>,
    request: SelfHostedModelIdRequest,
) -> AppResult<bool> {
    let mut models = load_self_hosted(&state)?;
    let Some(model) = models.iter().find(|m| m.id == request.id).cloned() else {
        return Ok(false);
    };
    if model.has_api_key {
        let _ = state.keychain.delete(&model.keychain_id());
    }
    models.retain(|m| m.id != request.id);
    store_self_hosted(&state, &models)?;
    Ok(true)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelfHostedTestResult {
    pub latency_ms: u64,
    pub reply: String,
}

/// Sends a tiny request so the user knows the address, model name and key
/// work before a real task depends on them.
#[tauri::command]
pub async fn test_self_hosted_model(
    state: State<'_, AppState>,
    request: SelfHostedModelIdRequest,
) -> AppResult<SelfHostedTestResult> {
    let model = load_self_hosted(&state)?
        .into_iter()
        .find(|m| m.id == request.id)
        .ok_or_else(|| AppError::new("model_not_found", "That self-hosted model was removed."))?;
    let endpoint = self_hosted_endpoint(&state, &model.id)?;
    let started = std::time::Instant::now();
    let reply = crate::llm_client::complete(
        &endpoint,
        &model.model,
        "Reply with the single word OK.",
        &[crate::llm_client::ChatMessage {
            role: "user",
            content: "Connection test.".to_string(),
        }],
        512,
        None,
    )
    .await?;
    Ok(SelfHostedTestResult {
        latency_ms: started.elapsed().as_millis() as u64,
        reply: reply.text.chars().take(200).collect(),
    })
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
