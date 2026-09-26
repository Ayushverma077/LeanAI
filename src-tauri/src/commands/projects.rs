use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use leanai_core::aiignore::{self, AiIgnorePreview};
use leanai_core::gitinfo::{self, ChangedFile, DiffRequest, GitState};
use leanai_core::inventory::Inventory;
use leanai_core::project;
use leanai_core::walker::{self, ScanOptions};

use crate::app_state::{AppState, ProjectSession};
use crate::commands::{ProgressEvent, SCAN_PROGRESS_EVENT};
use crate::db::repositories;
use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectRequest {
    /// Absolute path chosen by the user through the OS directory picker. The
    /// backend canonicalises it and treats the result as the only approved
    /// root for this session.
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectResponse {
    pub project: repositories::ProjectRecord,
    pub git: GitState,
    /// True when the project already has a `.aiignore`.
    pub has_ai_ignore: bool,
}

/// FR-01: opening a project is the single moment a filesystem scope is granted.
#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    request: OpenProjectRequest,
) -> AppResult<OpenProjectResponse> {
    let root = project::canonical_root(&request.path)?;
    let fingerprint = project::fingerprint(&root);
    let display_name = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let policy_version = state.settings().policy.version;

    let record = state.with_db(|connection| {
        repositories::upsert_project(
            connection,
            &fingerprint,
            &root.to_string_lossy(),
            &display_name,
            policy_version,
        )
    })?;

    state.set_session(ProjectSession {
        record: record.clone(),
        root: root.clone(),
        inventory: Arc::new(empty_inventory(&root, &fingerprint, policy_version)),
    });

    Ok(OpenProjectResponse {
        project: record,
        git: gitinfo::git_state(&root),
        has_ai_ignore: root.join(aiignore::FILE_NAME).exists(),
    })
}

fn empty_inventory(root: &std::path::Path, fingerprint: &str, policy_version: u32) -> Inventory {
    Inventory {
        root: root.to_string_lossy().into_owned(),
        project_fingerprint: fingerprint.to_string(),
        source_revision: String::new(),
        policy_version,
        scanned_at_ms: 0,
        files: Vec::new(),
        issues: Vec::new(),
        stats: Default::default(),
    }
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> AppResult<()> {
    state.cancel_scan();
    state.clear_session();
    Ok(())
}

#[tauri::command]
pub async fn list_projects(
    state: State<'_, AppState>,
) -> AppResult<Vec<repositories::ProjectRecord>> {
    state.with_db(|connection| repositories::list_projects(connection))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub inventory: Inventory,
    /// Rendered exclusion summary so the UI does not recompute policy rules.
    pub class_counts: std::collections::BTreeMap<String, usize>,
    pub selectable_bytes: u64,
}

/// FR-02/06: scans the open project. Heavy work runs on a blocking thread and
/// progress is emitted so the UI never freezes.
#[tauri::command]
pub async fn scan_project(app: AppHandle, state: State<'_, AppState>) -> AppResult<ScanResponse> {
    let session = state.require_session()?;
    let settings = state.settings();
    let cancel = state.begin_scan();
    let root = session.root.clone();

    let options = ScanOptions {
        policy: settings.policy.clone(),
        hash_contents: true,
        progress_every: 250,
    };

    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        walker::scan(&root, &options, &cancel, |progress| {
            let _ = handle.emit(
                SCAN_PROGRESS_EVENT,
                ProgressEvent {
                    phase: "scanning".to_string(),
                    files_seen: progress.files_seen,
                    directories_seen: progress.directories_seen,
                    bytes_seen: progress.bytes_seen,
                },
            );
        })
    })
    .await
    .map_err(|error| AppError::internal(format!("scan task failed: {error}")))?;

    state.end_scan();
    let inventory = result?;

    state.with_db(|connection| {
        repositories::set_scan_revision(connection, &session.record.id, &inventory.source_revision)
    })?;

    let class_counts = inventory.class_counts();
    let selectable_bytes = inventory.selectable_bytes();
    let inventory = Arc::new(inventory);
    state.update_inventory(inventory.clone());

    Ok(ScanResponse {
        inventory: (*inventory).clone(),
        class_counts,
        selectable_bytes,
    })
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.cancel_scan())
}

#[tauri::command]
pub async fn git_status(state: State<'_, AppState>) -> AppResult<GitState> {
    let session = state.require_session()?;
    Ok(gitinfo::git_state(&session.root))
}

/// Backlog 4.3: git-diff selection with an explicit scope and base reference.
#[tauri::command]
pub async fn changed_files(
    state: State<'_, AppState>,
    request: DiffRequest,
) -> AppResult<Vec<ChangedFile>> {
    let session = state.require_session()?;
    Ok(gitinfo::changed_files(&session.root, &request)?)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiIgnoreRequest {
    pub contents: String,
}

/// Backlog 4.2: preview a candidate `.aiignore` without writing it.
#[tauri::command]
pub async fn preview_ai_ignore(
    state: State<'_, AppState>,
    request: AiIgnoreRequest,
) -> AppResult<AiIgnorePreview> {
    let session = state.require_session()?;
    Ok(aiignore::preview(
        &session.root,
        &request.contents,
        &session.inventory,
    )?)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiIgnoreFile {
    pub exists: bool,
    pub contents: String,
    pub template: String,
}

#[tauri::command]
pub async fn read_ai_ignore(state: State<'_, AppState>) -> AppResult<AiIgnoreFile> {
    let session = state.require_session()?;
    let path = session.root.join(aiignore::FILE_NAME);
    let exists = path.exists();
    Ok(AiIgnoreFile {
        contents: if exists {
            std::fs::read_to_string(&path)?
        } else {
            String::new()
        },
        exists,
        template: aiignore::TEMPLATE.to_string(),
    })
}

/// Writes `.aiignore` into the project root. This is the only command that
/// writes inside a user's project, it targets one fixed filename, and the UI
/// requires the preview to be shown first.
#[tauri::command]
pub async fn write_ai_ignore(
    state: State<'_, AppState>,
    request: AiIgnoreRequest,
) -> AppResult<AiIgnoreFile> {
    let session = state.require_session()?;
    let path = project::resolve_within_root(&session.root, aiignore::FILE_NAME)?;
    std::fs::write(&path, &request.contents)?;
    state.with_db(|connection| {
        repositories::record_audit(
            connection,
            &session.record.id,
            "write_ai_ignore",
            "project_write",
            &serde_json::json!({ "path": aiignore::FILE_NAME, "bytes": request.contents.len() }),
            "succeeded",
        )
        .map(|_| ())
    })?;
    Ok(AiIgnoreFile {
        exists: true,
        contents: request.contents,
        template: aiignore::TEMPLATE.to_string(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectIdRequest {
    pub project_id: String,
}

/// Backlog 4.7: removing a project explains exactly what was deleted.
#[tauri::command]
pub async fn forget_project(
    state: State<'_, AppState>,
    request: ProjectIdRequest,
) -> AppResult<serde_json::Value> {
    if let Some(session) = state.session() {
        if session.record.id == request.project_id {
            state.clear_session();
        }
    }
    state.with_db(|connection| repositories::delete_project(connection, &request.project_id))
}
