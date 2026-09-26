//! LeanAI Desktop — Tauri application layer.
//!
//! This crate owns process, database and IPC concerns. All product policy
//! (what may be scanned, selected, bundled or read) lives in `leanai-core`, so
//! the rules are testable without a running desktop app and cannot be bypassed
//! by adding a command here.

pub mod app_state;
pub mod commands;
pub mod db;
pub mod error;
pub mod keychain;
pub mod sidecar_manager;

#[cfg(target_os = "macos")]
mod macos_dock {
    use std::ffi::{c_char, c_void};

    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }

    pub fn set_dock_icon(icon_bytes: &[u8]) {
        unsafe {
            let ns_data_class = objc_getClass(c"NSData".as_ptr());
            let ns_image_class = objc_getClass(c"NSImage".as_ptr());
            let ns_app_class = objc_getClass(c"NSApplication".as_ptr());
            if ns_data_class.is_null() || ns_image_class.is_null() || ns_app_class.is_null() {
                return;
            }

            let sel_data_with_bytes = sel_registerName(c"dataWithBytes:length:".as_ptr());
            let sel_alloc = sel_registerName(c"alloc".as_ptr());
            let sel_init_with_data = sel_registerName(c"initWithData:".as_ptr());
            let sel_shared_app = sel_registerName(c"sharedApplication".as_ptr());
            let sel_set_icon = sel_registerName(c"setApplicationIconImage:".as_ptr());

            let msg_send_data: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *const u8,
                usize,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let data = msg_send_data(
                ns_data_class,
                sel_data_with_bytes,
                icon_bytes.as_ptr(),
                icon_bytes.len(),
            );
            if data.is_null() {
                return;
            }

            let msg_send_no_args: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            let raw_image = msg_send_no_args(ns_image_class, sel_alloc);
            if raw_image.is_null() {
                return;
            }

            let msg_send_one_arg: unsafe extern "C" fn(
                *mut c_void,
                *mut c_void,
                *mut c_void,
            ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
            let image = msg_send_one_arg(raw_image, sel_init_with_data, data);
            if image.is_null() {
                return;
            }

            let app = msg_send_no_args(ns_app_class, sel_shared_app);
            if !app.is_null() {
                let msg_send_set_icon: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
                    std::mem::transmute(objc_msgSend as *const ());
                msg_send_set_icon(app, sel_set_icon, image);
            }
        }
    }
}

use app_state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let state = AppState::initialize(app_data_dir).map_err(|error| {
                std::io::Error::other(format!("{}: {}", error.code, error.message))
            })?;
            app.manage(state);

            #[cfg(target_os = "macos")]
            {
                const ICON_BYTES: &[u8] = include_bytes!("../icons/icon.png");
                macos_dock::set_dock_icon(ICON_BYTES);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::projects::open_project,
            commands::projects::close_project,
            commands::projects::list_projects,
            commands::projects::scan_project,
            commands::projects::cancel_scan,
            commands::projects::git_status,
            commands::projects::changed_files,
            commands::projects::preview_ai_ignore,
            commands::projects::read_ai_ignore,
            commands::projects::write_ai_ignore,
            commands::projects::forget_project,
            commands::bundles::resolve_selection,
            commands::bundles::suggest_selection,
            commands::bundles::directory_summaries,
            commands::bundles::directory_states,
            commands::bundles::build_bundle,
            commands::bundles::export_preflight,
            commands::bundles::export_bundle,
            commands::bundles::save_preset,
            commands::bundles::list_presets,
            commands::bundles::rename_preset,
            commands::bundles::delete_preset,
            commands::bundles::bundle_history,
            commands::bundles::clear_bundle_history,
            commands::bundles::audit_log,
            commands::context::generate_context,
            commands::context::load_context,
            commands::context::context_change_impact,
            commands::context::fetch_source,
            commands::context::save_context_document,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::describe_policy,
            commands::settings::reset_settings,
            commands::diagnostics::diagnostics,
            commands::models::list_models,
            commands::models::register_local_model,
            commands::models::unregister_model,
            commands::models::start_local_model,
            commands::models::stop_local_model,
            commands::models::local_model_status,
            commands::models::list_providers,
            commands::models::configure_provider_credential,
            commands::models::disconnect_provider,
            commands::models::check_provider_status,
            commands::models::get_model_catalog,
            commands::models::route_task,
            commands::models::estimate_provider_tokens,
            commands::agent::start_task_run,
            commands::agent::resolve_approval,
            commands::agent::cancel_task_run,
            commands::agent::list_task_runs,
            commands::agent::get_task_run,
            commands::agent::query_task_context,
            commands::agent::get_command_allowlist_command,
            commands::agent::update_command_allowlist_command,
            commands::agent::run_tester_step,
            commands::git::get_git_auth_status,
            commands::git::configure_github_token,
            commands::git::disconnect_github,
            commands::git::test_github_ssh,
            commands::git::git_remote_status,
            commands::git::git_push_branch,
            commands::git::clone_remote_repository,
            commands::git::list_github_repositories,
        ])
        .run(tauri::generate_context!())
        .expect("error while running LeanAI Desktop");
}
