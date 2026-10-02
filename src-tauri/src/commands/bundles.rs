use serde::{Deserialize, Serialize};
use tauri::State;

use leanai_core::concat::{self, Bundle, BundleOptions};
use leanai_core::context::{self, ContextDocument};
use leanai_core::manifest::BundleManifest;
use leanai_core::project;
use leanai_core::secrets::{self, SecretReport};
use leanai_core::selection::{
    self, DirectorySummary, ResolvedSelection, SelectionRecipe, SelectionSpec, SuggestedSelection,
    TriState,
};
use leanai_core::tokenizer;

use crate::app_state::AppState;
use crate::commands::context::{ensure_project_context, file_info, ContextFileInfo};
use crate::db::repositories;
use crate::error::{AppError, AppResult};

/// The project map for a bundle that asks for one: the stored index,
/// regenerated first when it is missing, stale or in an older format, so the
/// map always matches the files beside it (ADR 0015).
async fn project_map(
    state: &AppState,
    options: &BundleOptions,
) -> AppResult<Option<(ContextDocument, String)>> {
    if !options.include_project_map {
        return Ok(None);
    }
    let document = ensure_project_context(state).await?;
    let markdown = context::render(&document);
    Ok(Some((document, markdown)))
}

/// The state of PROJECT_CONTEXT.md in the project, for the map shown beside
/// the bundle.
fn map_file(state: &AppState, document: &ContextDocument) -> Option<ContextFileInfo> {
    state
        .session()
        .map(|session| file_info(&session.root, document))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSelectionRequest {
    pub selection: SelectionSpec,
}

#[tauri::command]
pub async fn resolve_selection(
    state: State<'_, AppState>,
    request: ResolveSelectionRequest,
) -> AppResult<ResolvedSelection> {
    let session = state.require_session()?;
    let policy = state.settings().policy;
    Ok(selection::resolve(
        &session.inventory,
        &request.selection,
        &policy,
    )?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryStateRequest {
    pub directories: Vec<String>,
    pub selected: Vec<String>,
}

/// Tri-state for the file tree, computed in Rust so the UI and the bundler
/// agree on what "the whole folder is selected" means.
#[tauri::command]
pub async fn directory_states(
    state: State<'_, AppState>,
    request: DirectoryStateRequest,
) -> AppResult<std::collections::BTreeMap<String, TriState>> {
    let session = state.require_session()?;
    let selected: std::collections::HashSet<String> = request.selected.into_iter().collect();
    Ok(request
        .directories
        .into_iter()
        .map(|directory| {
            let tri = selection::directory_state(&session.inventory, &directory, &selected);
            (directory, tri)
        })
        .collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestSelectionRequest {
    pub recipe: SelectionRecipe,
}

/// Proposes a starting selection so opening a repository does not begin with
/// clicking every checkbox. The rule lives in `leanai-core`, so it cannot
/// propose anything the policy blocks.
#[tauri::command]
pub async fn suggest_selection(
    state: State<'_, AppState>,
    request: SuggestSelectionRequest,
) -> AppResult<SuggestedSelection> {
    let session = state.require_session()?;
    Ok(selection::suggest(&session.inventory, request.recipe))
}

/// Every directory holding at least one selectable file, with its totals, for
/// the folder picker.
#[tauri::command]
pub async fn directory_summaries(state: State<'_, AppState>) -> AppResult<Vec<DirectorySummary>> {
    let session = state.require_session()?;
    Ok(selection::directory_summaries(&session.inventory))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildBundleRequest {
    pub selection: SelectionSpec,
    pub options: BundleOptions,
    /// Cap the returned preview text. The full bundle still hashes and counts
    /// in full; only the transferred preview is trimmed.
    pub preview_limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildBundleResponse {
    pub resolved: ResolvedSelection,
    #[serde(flatten)]
    pub bundle: BundlePreview,
    pub manifest: BundleManifest,
    /// The exact index whose rendering heads the bundle, when included, so the
    /// UI shows the same map that is copied.
    pub project_map: Option<ContextDocument>,
    /// The state of PROJECT_CONTEXT.md in the project, when the map is
    /// included, so the page can alert about an existing file.
    pub project_map_file: Option<ContextFileInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BundlePreview {
    /// The bundle text without the project map, which the UI shows on its own.
    pub preview: String,
    pub preview_truncated: bool,
    /// Tokens the project map adds; 0 when it is not included.
    pub project_map_tokens: usize,
    pub output_hash: String,
    pub estimate: tokenizer::TokenEstimate,
    /// The label the UI must render next to the number.
    pub estimate_label: String,
    pub contributions: Vec<tokenizer::FileContribution>,
    pub truncations: Vec<concat::TruncationWarning>,
    pub skipped: Vec<String>,
    pub byte_len: u64,
    pub file_count: usize,
}

/// Characters of bundle text sent to the UI for display.
///
/// The preview is read by a human in a scroll box, and it crosses the IPC
/// boundary on every rebuild. 400 KB of it was pure transfer cost for text
/// nobody scrolls to; every number shown alongside still covers the full
/// bundle, and the UI says the preview is shortened.
const PREVIEW_LIMIT: usize = 64_000;

fn preview_of(bundle: Bundle, limit: Option<usize>) -> BundlePreview {
    let limit = limit.unwrap_or(PREVIEW_LIMIT);
    let mut preview = bundle.text;
    if let Some((start, end)) = bundle.project_map_range {
        preview.replace_range(start..end, "");
    }
    let truncated = preview.len() > limit;
    if truncated {
        let mut end = limit;
        while end > 0 && !preview.is_char_boundary(end) {
            end -= 1;
        }
        preview.truncate(end);
    }
    BundlePreview {
        preview,
        preview_truncated: truncated,
        project_map_tokens: bundle.project_map_tokens,
        output_hash: bundle.output_hash,
        estimate_label: bundle.estimate.kind.label(),
        estimate: bundle.estimate,
        contributions: bundle.contributions,
        truncations: bundle.truncations,
        skipped: bundle.skipped,
        byte_len: bundle.byte_len,
        file_count: bundle.file_count,
    }
}

/// FR-07/09/10/13: resolve, build and estimate in one round trip so the preview
/// and the number can never disagree.
#[tauri::command]
pub async fn build_bundle(
    state: State<'_, AppState>,
    request: BuildBundleRequest,
) -> AppResult<BuildBundleResponse> {
    // First, because regenerating the map may rescan the project.
    let map = project_map(&state, &request.options).await?;
    let session = state.require_session()?;
    let policy = state.settings().policy;
    let inventory = session.inventory.clone();
    let root = session.root.clone();

    let resolved = selection::resolve(&inventory, &request.selection, &policy)?;
    let options = request.options.clone();
    let build_inventory = inventory.clone();
    let build_resolved = resolved.clone();
    let (map_document, map_text) = map.unzip();
    let project_map_file = map_document
        .as_ref()
        .and_then(|document| map_file(&state, document));

    let bundle = tauri::async_runtime::spawn_blocking(move || {
        concat::build_with_map(
            &root,
            &build_inventory,
            &build_resolved,
            &options,
            map_text.as_deref(),
        )
    })
    .await
    .map_err(|error| AppError::internal(format!("bundle task failed: {error}")))??;

    let manifest = BundleManifest::new(
        &inventory,
        &resolved,
        &request.options,
        &bundle.estimate,
        &bundle.truncations,
        &bundle.skipped,
    );

    Ok(BuildBundleResponse {
        resolved,
        bundle: preview_of(bundle, request.preview_limit),
        manifest,
        project_map: map_document,
        project_map_file,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub selection: SelectionSpec,
    pub options: BundleOptions,
    /// Where the bundle is going. Recorded in the audit log.
    pub destination: ExportDestination,
    /// Absolute path chosen through the OS save dialog. Required for `file`.
    pub target_path: Option<String>,
    /// Set once the user has reviewed the preflight screen.
    pub acknowledged_warnings: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportDestination {
    Clipboard,
    File,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreflight {
    pub file_count: usize,
    /// True when `PROJECT_CONTEXT.md` heads the exported text.
    pub project_map_included: bool,
    pub byte_len: u64,
    pub estimate: tokenizer::TokenEstimate,
    pub estimate_label: String,
    pub included_files: Vec<String>,
    pub secret_report: SecretReport,
    pub requires_confirmation: bool,
    pub disclaimer: &'static str,
    pub destination_note: String,
}

/// Backlog 4.6: the consent screen every export goes through, including local
/// ones, so a future cloud provider inherits a tested flow.
#[tauri::command]
pub async fn export_preflight(
    state: State<'_, AppState>,
    request: ExportRequest,
) -> AppResult<ExportPreflight> {
    let map_text = project_map(&state, &request.options)
        .await?
        .map(|(_, markdown)| markdown);
    let session = state.require_session()?;
    let settings = state.settings();
    let resolved = selection::resolve(&session.inventory, &request.selection, &settings.policy)?;
    let root = session.root.clone();
    let options = request.options.clone();
    let inventory = session.inventory.clone();
    let files = resolved.files.clone();

    let (bundle, report) = tauri::async_runtime::spawn_blocking(move || {
        let bundle =
            concat::build_with_map(&root, &inventory, &resolved, &options, map_text.as_deref())?;
        let mut report = SecretReport::default();
        // The map is generated text, but it leaves too, so it is scanned too.
        if let Some(map) = &map_text {
            report.push(secrets::scan_text(context::DOCUMENT_NAME, map, 20));
        }
        for path in &files {
            match project::resolve_within_root(&root, path)
                .and_then(|absolute| std::fs::read_to_string(absolute).map_err(Into::into))
            {
                Ok(text) => report.push(secrets::scan_text(path, &text, 20)),
                Err(_) => report.skipped.push(path.clone()),
            }
        }
        Ok::<_, leanai_core::CoreError>((bundle, report))
    })
    .await
    .map_err(|error| AppError::internal(format!("preflight task failed: {error}")))??;

    let destination_note = match request.destination {
        ExportDestination::Clipboard => {
            "The bundle is copied to your system clipboard. Other applications on this device can read the clipboard.".to_string()
        }
        ExportDestination::File => format!(
            "The bundle is written to {}. It leaves LeanAI's control once written.",
            request.target_path.as_deref().unwrap_or("a file you choose")
        ),
    };

    Ok(ExportPreflight {
        file_count: bundle.file_count,
        project_map_included: bundle.project_map_range.is_some(),
        byte_len: bundle.byte_len,
        estimate_label: bundle.estimate.kind.label(),
        estimate: bundle.estimate,
        included_files: bundle
            .contributions
            .iter()
            .map(|c| c.path.clone())
            .collect(),
        requires_confirmation: settings.require_export_confirmation
            && report.requires_confirmation(),
        secret_report: report,
        disclaimer: SecretReport::DISCLAIMER,
        destination_note,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResponse {
    /// Present for clipboard exports; the frontend hands this to the clipboard
    /// plugin, which is the only capability allowed to write it.
    pub text: Option<String>,
    pub written_path: Option<String>,
    pub output_hash: String,
    pub bundle_id: Option<String>,
    pub manifest_path: Option<String>,
}

/// FR-11/12: performs the export and records it in the audit log.
#[tauri::command]
pub async fn export_bundle(
    state: State<'_, AppState>,
    request: ExportRequest,
) -> AppResult<ExportResponse> {
    let map_text = project_map(&state, &request.options)
        .await?
        .map(|(_, markdown)| markdown);
    let session = state.require_session()?;
    let settings = state.settings();
    let resolved = selection::resolve(&session.inventory, &request.selection, &settings.policy)?;

    let root = session.root.clone();
    let inventory = session.inventory.clone();
    let options = request.options.clone();
    let build_resolved = resolved.clone();
    let bundle = tauri::async_runtime::spawn_blocking(move || {
        concat::build_with_map(
            &root,
            &inventory,
            &build_resolved,
            &options,
            map_text.as_deref(),
        )
    })
    .await
    .map_err(|error| AppError::internal(format!("bundle task failed: {error}")))??;

    let manifest = BundleManifest::new(
        &session.inventory,
        &resolved,
        &request.options,
        &bundle.estimate,
        &bundle.truncations,
        &bundle.skipped,
    );

    if settings.require_export_confirmation && !request.acknowledged_warnings {
        return Err(AppError::new(
            "confirmation_required",
            "This export has not been confirmed on the preflight screen.",
        )
        .with_recovery("Review the file list and warnings, then confirm the export."));
    }

    let mut written_path = None;
    let mut manifest_path = None;
    if request.destination == ExportDestination::File {
        let target = request.target_path.clone().ok_or_else(|| {
            AppError::new("missing_target", "No destination file was chosen.")
                .with_recovery("Choose where to save the bundle.")
        })?;
        std::fs::write(&target, &bundle.text)?;
        let sidecar = format!("{target}.leanai-manifest.yaml");
        std::fs::write(&sidecar, manifest.to_front_matter())?;
        manifest_path = Some(sidecar);
        written_path = Some(target);
    }

    let estimate_kind = manifest.token_estimate.kind.clone();
    let bundle_id = state.with_db(|connection| {
        let id = repositories::record_bundle(
            connection,
            &session.record.id,
            &session.inventory.source_revision,
            &manifest,
            &bundle.output_hash,
            bundle.estimate.value,
            &estimate_kind,
            bundle.file_count,
            bundle.byte_len,
            Some(&bundle.text),
            settings.bundle_retention,
        )?;
        repositories::record_audit(
            connection,
            &session.record.id,
            "export_bundle",
            match request.destination {
                ExportDestination::Clipboard => "clipboard",
                ExportDestination::File => "file",
            },
            &serde_json::json!({
                "fileCount": bundle.file_count,
                "byteLen": bundle.byte_len,
                "outputHash": bundle.output_hash,
                "estimate": bundle.estimate.value,
                "estimateKind": estimate_kind,
                "includedFiles": resolved.files,
                "projectMapIncluded": bundle.project_map_range.is_some(),
            }),
            "succeeded",
        )?;
        Ok(id)
    })?;

    Ok(ExportResponse {
        text: match request.destination {
            ExportDestination::Clipboard => Some(bundle.text),
            ExportDestination::File => None,
        },
        written_path,
        output_hash: bundle.output_hash,
        bundle_id,
        manifest_path,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePresetRequest {
    pub name: String,
    pub selection: SelectionSpec,
    pub options: BundleOptions,
}

#[tauri::command]
pub async fn save_preset(
    state: State<'_, AppState>,
    request: SavePresetRequest,
) -> AppResult<repositories::PresetRecord> {
    let session = state.require_session()?;
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::new("invalid_name", "A preset needs a name."));
    }
    state.with_db(|connection| {
        repositories::save_preset(
            connection,
            &session.record.id,
            &name,
            &request.selection,
            &request.options,
        )
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetListResponse {
    pub presets: Vec<PresetWithValidation>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetWithValidation {
    #[serde(flatten)]
    pub preset: repositories::PresetRecord,
    /// Re-resolved against the current scan, so a preset saved before a
    /// refactor reports what it can no longer find (backlog 4.1).
    pub validation: Option<ResolvedSelection>,
}

#[tauri::command]
pub async fn list_presets(state: State<'_, AppState>) -> AppResult<PresetListResponse> {
    let session = state.require_session()?;
    let policy = state.settings().policy;
    let presets =
        state.with_db(|connection| repositories::list_presets(connection, &session.record.id))?;

    let presets = presets
        .into_iter()
        .map(|preset| {
            let validation =
                selection::resolve(&session.inventory, &preset.selection, &policy).ok();
            PresetWithValidation { preset, validation }
        })
        .collect();
    Ok(PresetListResponse { presets })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetIdRequest {
    pub preset_id: String,
    pub name: Option<String>,
}

#[tauri::command]
pub async fn rename_preset(state: State<'_, AppState>, request: PresetIdRequest) -> AppResult<()> {
    let name = request
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| AppError::new("invalid_name", "A preset needs a name."))?;
    state.with_db(|connection| repositories::rename_preset(connection, &request.preset_id, name))
}

#[tauri::command]
pub async fn delete_preset(state: State<'_, AppState>, request: PresetIdRequest) -> AppResult<()> {
    state.with_db(|connection| repositories::delete_preset(connection, &request.preset_id))
}

#[tauri::command]
pub async fn bundle_history(
    state: State<'_, AppState>,
) -> AppResult<Vec<repositories::BundleRecord>> {
    let session = state.require_session()?;
    let limit = state.settings().max_history_entries;
    state.with_db(|connection| repositories::list_bundles(connection, &session.record.id, limit))
}

#[tauri::command]
pub async fn clear_bundle_history(state: State<'_, AppState>) -> AppResult<usize> {
    let session = state.require_session()?;
    state.with_db(|connection| repositories::clear_history(connection, &session.record.id))
}

#[tauri::command]
pub async fn audit_log(state: State<'_, AppState>) -> AppResult<Vec<serde_json::Value>> {
    let session = state.require_session()?;
    state.with_db(|connection| repositories::list_audit(connection, &session.record.id, 100))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle(text: &str, map: Option<(usize, usize)>) -> Bundle {
        Bundle {
            text: text.to_string(),
            output_hash: "h".into(),
            estimate: tokenizer::estimate(text),
            contributions: Vec::new(),
            truncations: Vec::new(),
            skipped: Vec::new(),
            byte_len: text.len() as u64,
            file_count: 1,
            project_map_tokens: if map.is_some() { 7 } else { 0 },
            project_map_range: map,
        }
    }

    #[test]
    fn preview_leaves_out_the_map_but_keeps_its_numbers() {
        let text = "# Project Context\n\n---\n\n===== a.ts =====\nx\n";
        let map_end = "# Project Context\n\n---\n\n".len();
        let preview = preview_of(bundle(text, Some((0, map_end))), None);
        assert_eq!(preview.preview, "===== a.ts =====\nx\n");
        assert_eq!(preview.project_map_tokens, 7);
        assert_eq!(
            preview.byte_len,
            text.len() as u64,
            "numbers cover the whole bundle"
        );

        let plain = preview_of(bundle("===== a.ts =====\nx\n", None), Some(5));
        assert!(plain.preview_truncated);
        assert_eq!(plain.preview, "=====");
    }
}
