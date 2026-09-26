use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::db::migrations;
use crate::error::AppResult;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub app_version: &'static str,
    pub schema_version: u32,
    pub latest_schema_version: u32,
    pub policy_version: u32,
    pub platform: &'static str,
    pub arch: &'static str,
    /// Whether a project is open. The path itself is deliberately excluded.
    pub project_open: bool,
    pub telemetry_opt_in: bool,
    pub capabilities: Vec<&'static str>,
    pub notice: &'static str,
}

/// Backlog 11.3: a redacted diagnostic snapshot.
///
/// Contains versions, capability state and counts. It deliberately contains no
/// file paths, no source text and no credentials.
#[tauri::command]
pub async fn diagnostics(state: State<'_, AppState>) -> AppResult<Diagnostics> {
    let settings = state.settings();
    Ok(Diagnostics {
        app_version: env!("CARGO_PKG_VERSION"),
        schema_version: state.schema_version,
        latest_schema_version: migrations::LATEST_VERSION,
        policy_version: settings.policy.version,
        platform: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        project_open: state.session().is_some(),
        telemetry_opt_in: settings.telemetry_opt_in,
        capabilities: vec![
            "filesystem: LeanAI commands only, inside the approved project root",
            "network: none",
            "shell: none",
            "clipboard: write-only, on explicit export",
        ],
        notice: "This snapshot contains no file paths, source text or credentials.",
    })
}
