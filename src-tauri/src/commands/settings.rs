use tauri::State;

use crate::app_state::{AppState, Settings};
use crate::error::AppResult;

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    Ok(state.settings())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> AppResult<Settings> {
    state.save_settings(settings)
}

/// The running policy, described by the backend so the UI never keeps its own
/// copy of the rules it displays.
#[tauri::command]
pub async fn describe_policy() -> AppResult<serde_json::Value> {
    Ok(leanai_core::policy_description())
}

/// Restores defaults. Projects, presets and history are untouched.
#[tauri::command]
pub async fn reset_settings(state: State<'_, AppState>) -> AppResult<Settings> {
    state.save_settings(Settings::default())
}
