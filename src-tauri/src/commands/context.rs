use serde::{Deserialize, Serialize};
use tauri::State;

use std::path::Path;
use std::sync::Arc;

use leanai_core::context::{self, ChangeImpact, ContextDocument, GenerateOptions};
use leanai_core::context_file::{self, FileState, SaveOutcome};
use leanai_core::project;
use leanai_core::walker::{self, CancelToken, ScanOptions};

use crate::app_state::AppState;
use crate::db::repositories;
use crate::error::{AppError, AppResult};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextResponse {
    pub document: ContextDocument,
    pub markdown: String,
    pub freshness: String,
    pub stale_section_keys: Vec<String>,
    /// The state of PROJECT_CONTEXT.md in the project, with an alert when
    /// LeanAI left an existing file alone (ADR 0016).
    pub file: ContextFileInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextFileInfo {
    pub state: FileState,
    /// Set when the file in the project is not LeanAI's current map.
    pub alert: Option<&'static str>,
    /// Whether the user may replace the file with LeanAI's map.
    pub can_replace: bool,
}

/// How PROJECT_CONTEXT.md in `root` compares with `document`, the map LeanAI
/// last produced.
pub(crate) fn file_info(root: &Path, document: &ContextDocument) -> ContextFileInfo {
    let state = context_file::file_state(root, Some(&document.content_hash))
        .unwrap_or(FileState::Unreadable);
    ContextFileInfo {
        state,
        alert: state.alert(),
        can_replace: state.replaceable(),
    }
}

/// SHA-256 of the text LeanAI last wrote to PROJECT_CONTEXT.md, from the
/// stored index, so an unchanged LeanAI file can be updated without asking.
fn last_written(state: &AppState, project_id: &str) -> AppResult<Option<String>> {
    Ok(state
        .with_db(|connection| repositories::latest_context(connection, project_id))?
        .map(|document| document.content_hash))
}

fn respond(document: ContextDocument, root: &Path) -> ContextResponse {
    let file = file_info(root, &document);
    let markdown = context::render(&document);
    let freshness = format!("{:?}", document.freshness()).to_lowercase();
    let stale_section_keys = document
        .stale_sections()
        .into_iter()
        .map(|section| section.key.clone())
        .collect();
    ContextResponse {
        document,
        markdown,
        freshness,
        stale_section_keys,
        file,
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateContextRequest {
    /// The user chose to replace a PROJECT_CONTEXT.md that LeanAI did not
    /// write, or that was edited since.
    #[serde(default)]
    pub replace_existing: bool,
}

/// FR-17: generates the deterministic context index for the open project.
///
/// The index is always stored for the app. The project file is written only
/// when that cannot destroy anything, or when `replace_existing` says the user
/// chose to replace it; otherwise the response carries an alert.
#[tauri::command]
pub async fn generate_context(
    state: State<'_, AppState>,
    request: Option<GenerateContextRequest>,
) -> AppResult<ContextResponse> {
    let replace = request.unwrap_or_default().replace_existing;
    let _gate = state.context_gate.lock().await;
    let session = state.require_session()?;
    if session.inventory.files.is_empty() {
        return Err(
            AppError::new("no_scan", "This project has not been scanned yet.")
                .with_recovery("Scan the project, then generate its context index."),
        );
    }

    let previous = last_written(&state, &session.record.id)?;
    let root = session.root.clone();
    let inventory = session.inventory.clone();
    let document = tauri::async_runtime::spawn_blocking(move || {
        let document = context::generate(&root, &inventory, &GenerateOptions::default())?;
        // A kept file is reported through `file`, not as an error.
        let _: SaveOutcome = context_file::save_to_project(
            &root,
            &context::render(&document),
            previous.as_deref(),
            replace,
        )?;
        Ok::<_, leanai_core::CoreError>(document)
    })
    .await
    .map_err(|error| AppError::internal(format!("context task failed: {error}")))??;

    state.with_db(|connection| {
        repositories::save_context(connection, &session.record.id, &document)
    })?;
    Ok(respond(document, &session.root))
}

/// Rescans the open project and regenerates PROJECT_CONTEXT.md (file and
/// database copy). Called after LeanAI changes project files, so the index
/// never describes code that is no longer there.
///
/// Writing the file is best effort and never replaces a file LeanAI did not
/// write, or one edited since; the Context page alerts about those. The
/// stored index is updated either way.
pub(crate) async fn refresh_project_context(state: &AppState) -> AppResult<ContextDocument> {
    let _gate = state.context_gate.lock().await;
    refresh_unlocked(state).await
}

/// [`refresh_project_context`] for callers already holding the context gate.
async fn refresh_unlocked(state: &AppState) -> AppResult<ContextDocument> {
    let session = state.require_session()?;
    let previous = last_written(state, &session.record.id)?;
    let policy = state.settings().policy;
    let root = session.root.clone();
    let (inventory, document) = tauri::async_runtime::spawn_blocking(move || {
        let options = ScanOptions {
            policy,
            hash_contents: true,
            progress_every: 1_000,
        };
        let inventory = walker::scan(&root, &options, &CancelToken::new(), |_| {})?;
        let document = context::generate(&root, &inventory, &GenerateOptions::default())?;
        let _ = context_file::save_to_project(
            &root,
            &context::render(&document),
            previous.as_deref(),
            false,
        );
        Ok::<_, leanai_core::CoreError>((inventory, document))
    })
    .await
    .map_err(|error| AppError::internal(format!("context refresh failed: {error}")))??;

    state.with_db(|connection| {
        repositories::set_scan_revision(
            connection,
            &session.record.id,
            &inventory.source_revision,
        )?;
        repositories::save_context(connection, &session.record.id, &document)
    })?;
    state.update_inventory(Arc::new(inventory));
    Ok(document)
}

/// The stored context for the open project, generated (and saved) first when
/// the project has none yet, or when the stored copy predates the last scan.
pub(crate) async fn ensure_project_context(state: &AppState) -> AppResult<ContextDocument> {
    let _gate = state.context_gate.lock().await;
    let session = state.require_session()?;
    let stored =
        state.with_db(|connection| repositories::latest_context(connection, &session.record.id))?;
    match stored {
        // An index in an older format is rebuilt rather than reused.
        Some(document) if document.schema_version != context::SCHEMA_VERSION => {
            refresh_unlocked(state).await
        }
        Some(mut document) if !session.inventory.files.is_empty() => {
            context::apply_change_impact(&mut document, &session.inventory);
            if document.stale_sections().is_empty() {
                return Ok(document);
            }
            refresh_unlocked(state).await
        }
        Some(document) => Ok(document),
        None => refresh_unlocked(state).await,
    }
}

/// Loads the stored context and re-checks it against the current scan, so a
/// user always sees freshness rather than a snapshot's claim of it (FR-19).
#[tauri::command]
pub async fn load_context(state: State<'_, AppState>) -> AppResult<Option<ContextResponse>> {
    let session = state.require_session()?;
    let stored =
        state.with_db(|connection| repositories::latest_context(connection, &session.record.id))?;
    // An index in an older format is not shown; the page offers to generate
    // a new one instead.
    let Some(mut document) = stored.filter(|d| d.schema_version == context::SCHEMA_VERSION) else {
        return Ok(None);
    };
    if !session.inventory.files.is_empty() {
        context::apply_change_impact(&mut document, &session.inventory);
    }
    Ok(Some(respond(document, &session.root)))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeImpactResponse {
    pub impact: ChangeImpact,
    pub freshness: String,
}

/// FR-19: reports which sections a rescan invalidated.
#[tauri::command]
pub async fn context_change_impact(
    state: State<'_, AppState>,
) -> AppResult<Option<ChangeImpactResponse>> {
    let session = state.require_session()?;
    let stored =
        state.with_db(|connection| repositories::latest_context(connection, &session.record.id))?;
    let Some(mut document) = stored else {
        return Ok(None);
    };
    let impact = context::apply_change_impact(&mut document, &session.inventory);
    Ok(Some(ChangeImpactResponse {
        freshness: format!("{:?}", document.freshness()).to_lowercase(),
        impact,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOnDemandRequest {
    /// Project-relative path. Rejected if it escapes the approved root.
    pub path: String,
    /// Optional 1-based inclusive line range.
    pub from_line: Option<usize>,
    pub to_line: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOnDemandResponse {
    pub path: String,
    pub content: String,
    pub content_hash: String,
    /// True when the file changed since the last scan.
    pub changed_since_scan: bool,
    pub from_line: usize,
    pub to_line: usize,
    pub total_lines: usize,
}

/// FR-20 / §8.3: fetch the current bytes behind a context claim.
///
/// This is what keeps the index honest: a summary never has to be trusted,
/// because its source is one call away and the response says whether the file
/// changed since the scan the claim was made against.
#[tauri::command]
pub async fn fetch_source(
    state: State<'_, AppState>,
    request: SourceOnDemandRequest,
) -> AppResult<SourceOnDemandResponse> {
    let session = state.require_session()?;
    let absolute = project::resolve_within_root(&session.root, &request.path)?;

    let entry = session.inventory.get(&request.path);
    if let Some(entry) = entry {
        if !entry.selectable && entry.class == leanai_core::classify::FileClass::CredentialSensitive
        {
            return Err(AppError::new(
                "blocked_path",
                "That path is excluded as credential-sensitive and is not read on demand.",
            ));
        }
    }

    let text = std::fs::read_to_string(&absolute)?;
    let lines: Vec<&str> = text.lines().collect();
    let total_lines = lines.len();
    let from = request.from_line.unwrap_or(1).max(1);
    let to = request.to_line.unwrap_or(total_lines).min(total_lines);
    let slice = if from > total_lines || from > to {
        String::new()
    } else {
        lines[from - 1..to].join("\n")
    };

    let content_hash = project::sha256_hex(text.as_bytes());
    let changed_since_scan = entry
        .and_then(|entry| entry.content_hash.as_ref())
        .is_some_and(|scanned| scanned != &content_hash);

    Ok(SourceOnDemandResponse {
        path: request.path,
        content: slice,
        content_hash,
        changed_since_scan,
        from_line: from,
        to_line: to,
        total_lines,
    })
}
