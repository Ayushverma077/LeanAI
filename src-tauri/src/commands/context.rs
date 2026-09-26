use serde::{Deserialize, Serialize};
use tauri::State;

use leanai_core::context::{self, ChangeImpact, ContextDocument, GenerateOptions};
use leanai_core::project;

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
}

fn respond(document: ContextDocument) -> ContextResponse {
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
    }
}

/// FR-17: generates the deterministic context index for the open project.
#[tauri::command]
pub async fn generate_context(state: State<'_, AppState>) -> AppResult<ContextResponse> {
    let session = state.require_session()?;
    if session.inventory.files.is_empty() {
        return Err(
            AppError::new("no_scan", "This project has not been scanned yet.")
                .with_recovery("Scan the project, then generate its context index."),
        );
    }

    let root = session.root.clone();
    let inventory = session.inventory.clone();
    let document = tauri::async_runtime::spawn_blocking(move || {
        context::generate(&root, &inventory, &GenerateOptions::default())
    })
    .await
    .map_err(|error| AppError::internal(format!("context task failed: {error}")))??;

    state.with_db(|connection| {
        repositories::save_context(connection, &session.record.id, &document)
    })?;
    Ok(respond(document))
}

/// Loads the stored context and re-checks it against the current scan, so a
/// user always sees freshness rather than a snapshot's claim of it (FR-19).
#[tauri::command]
pub async fn load_context(state: State<'_, AppState>) -> AppResult<Option<ContextResponse>> {
    let session = state.require_session()?;
    let stored =
        state.with_db(|connection| repositories::latest_context(connection, &session.record.id))?;
    let Some(mut document) = stored else {
        return Ok(None);
    };
    if !session.inventory.files.is_empty() {
        context::apply_change_impact(&mut document, &session.inventory);
    }
    Ok(Some(respond(document)))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveContextRequest {
    /// Absolute destination chosen through the OS save dialog.
    pub target_path: String,
}

#[tauri::command]
pub async fn save_context_document(
    state: State<'_, AppState>,
    request: SaveContextRequest,
) -> AppResult<String> {
    let session = state.require_session()?;
    let stored =
        state.with_db(|connection| repositories::latest_context(connection, &session.record.id))?;
    let mut document = stored.ok_or_else(|| {
        AppError::new("no_context", "This project has no context index yet.")
            .with_recovery("Generate the context index first.")
    })?;
    context::apply_change_impact(&mut document, &session.inventory);
    std::fs::write(&request.target_path, context::render(&document))?;

    state.with_db(|connection| {
        repositories::record_audit(
            connection,
            &session.record.id,
            "save_context_document",
            "file",
            &serde_json::json!({
                "targetPath": request.target_path,
                "revision": document.source_revision,
                "freshness": format!("{:?}", document.freshness()).to_lowercase(),
            }),
            "succeeded",
        )
        .map(|_| ())
    })?;
    Ok(request.target_path)
}
