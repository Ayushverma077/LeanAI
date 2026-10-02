use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use leanai_core::concat::BundleOptions;
use leanai_core::inventory::Inventory;
use leanai_core::policy::Policy;
use leanai_core::walker::CancelToken;

use crate::db::migrations;
use crate::db::repositories::{self, BundleRetention, ProjectRecord};
use crate::error::{AppError, AppResult};

/// User-controlled settings. Stored in SQLite as one JSON row so adding a
/// setting does not need a migration; the shape is versioned by `schema`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub schema: u32,
    pub policy: Policy,
    pub default_bundle_options: BundleOptions,
    pub bundle_retention: BundleRetention,
    /// Keep at most this many bundle history rows per project.
    pub max_history_entries: u32,
    /// Telemetry is off until the user opts in, and never carries source text,
    /// bundle text or credentials (backlog 0.6).
    pub telemetry_opt_in: bool,
    /// Require a typed confirmation before any export that leaves the device.
    pub require_export_confirmation: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema: 1,
            policy: Policy::default(),
            default_bundle_options: BundleOptions::default(),
            bundle_retention: BundleRetention::default(),
            max_history_entries: 50,
            telemetry_opt_in: false,
            require_export_confirmation: true,
        }
    }
}

pub const SETTINGS_KEY: &str = "settings.v1";

/// The project the user currently has open, with its last scan.
#[derive(Clone)]
pub struct ProjectSession {
    pub record: ProjectRecord,
    pub root: PathBuf,
    pub inventory: Arc<Inventory>,
}

/// The clone currently in progress, so the UI can cancel it and the app can
/// stop it on exit.
///
/// Without this a clone outlived the app: when `tauri dev` restarted, the
/// running `git clone` was re-parented to launchd and kept downloading, and
/// one report of a slow clone turned out to be two 1.1 GB clones splitting the
/// same connection.
#[derive(Default)]
pub struct CloneJob {
    child: Mutex<Option<std::process::Child>>,
    cancelled: std::sync::atomic::AtomicBool,
}

impl CloneJob {
    pub fn attach(&self, child: std::process::Child) {
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }
    }

    /// Waits for the attached process to exit, and detaches it.
    ///
    /// Only called once git's output has closed, i.e. once it has exited, so
    /// the process is never out of `cancel`'s reach while it is running.
    pub fn wait(&self) -> std::io::Result<std::process::ExitStatus> {
        let child = self
            .child
            .lock()
            .map_err(|_| std::io::Error::other("the clone lock was poisoned"))?
            .take();
        match child {
            Some(mut child) => child.wait(),
            None => Err(std::io::Error::other("no clone process is attached")),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Stops the clone and everything git started for it. Returns false when
    /// no process was running.
    ///
    /// On Unix this is SIGTERM to git's whole process group (it runs in its
    /// own), rather than SIGKILL, so git's own handler deletes the
    /// half-written directory. Windows has no equivalent, so there the process
    /// tree is terminated and the caller removes the folder.
    pub fn cancel(&self) -> bool {
        self.cancelled
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let Ok(mut guard) = self.child.lock() else {
            return false;
        };
        let Some(child) = guard.as_mut() else {
            return false;
        };
        let pid = child.id();

        #[cfg(unix)]
        {
            let signalled = std::process::Command::new("kill")
                .args(["-TERM", &format!("-{pid}")])
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
            if !signalled {
                let _ = child.kill();
            }
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
            let _ = child.kill();
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = pid;
            let _ = child.kill();
        }
        true
    }
}

/// Process-wide state.
///
/// Every field is guarded independently and no lock is ever held across an
/// `await`: heavy work runs on a blocking thread with cloned inputs, and the
/// result is written back in a short critical section (backlog 1.3).
pub struct AppState {
    db: Mutex<Connection>,
    session: Mutex<Option<ProjectSession>>,
    scan_cancel: Mutex<Option<CancelToken>>,
    clone_job: Mutex<Option<Arc<CloneJob>>>,
    settings: Mutex<Settings>,
    pub database_path: PathBuf,
    pub schema_version: u32,
    pub sidecar: Arc<crate::sidecar_manager::SidecarManager>,
    pub keychain: Arc<crate::keychain::KeychainStore>,
    /// Held while the project map is generated and written, so two builds
    /// (for example opening a project while the Context page refreshes) never
    /// race on PROJECT_CONTEXT.md. Async, because it is held across awaits.
    pub context_gate: tokio::sync::Mutex<()>,
}

impl AppState {
    /// Opens (or creates) the local database and applies migrations.
    pub fn initialize(app_data_dir: PathBuf) -> AppResult<Self> {
        std::fs::create_dir_all(&app_data_dir)?;
        let database_path = app_data_dir.join("leanai.sqlite3");
        let mut connection = Connection::open(&database_path)?;
        let schema_version = migrations::migrate(&mut connection)?;

        let settings = repositories::get_setting(&connection, SETTINGS_KEY)?
            .and_then(|raw| serde_json::from_str::<Settings>(&raw).ok())
            .unwrap_or_default();

        Ok(Self {
            db: Mutex::new(connection),
            session: Mutex::new(None),
            scan_cancel: Mutex::new(None),
            clone_job: Mutex::new(None),
            settings: Mutex::new(settings),
            database_path,
            schema_version,
            sidecar: Arc::new(crate::sidecar_manager::SidecarManager::new()),
            keychain: Arc::new(crate::keychain::KeychainStore::new()),
            context_gate: tokio::sync::Mutex::new(()),
        })
    }

    /// Runs `work` with the database connection. The closure must not block on
    /// anything other than SQLite.
    pub fn with_db<T>(&self, work: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self
            .db
            .lock()
            .map_err(|_| AppError::internal("the local database lock was poisoned"))?;
        work(&mut guard)
    }

    pub fn settings(&self) -> Settings {
        self.settings
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    pub fn save_settings(&self, settings: Settings) -> AppResult<Settings> {
        let serialized =
            serde_json::to_string(&settings).map_err(|e| AppError::internal(e.to_string()))?;
        self.with_db(|connection| {
            repositories::set_setting(connection, SETTINGS_KEY, &serialized)
        })?;
        if let Ok(mut guard) = self.settings.lock() {
            *guard = settings.clone();
        }
        Ok(settings)
    }

    pub fn session(&self) -> Option<ProjectSession> {
        self.session.lock().ok().and_then(|guard| guard.clone())
    }

    /// Returns the open project, or a recoverable error telling the user to
    /// open one. Commands call this instead of unwrapping.
    pub fn require_session(&self) -> AppResult<ProjectSession> {
        self.session().ok_or_else(|| {
            AppError::new("no_project", "No project is open.")
                .with_recovery("Choose a project directory to get started.")
        })
    }

    pub fn set_session(&self, session: ProjectSession) {
        if let Ok(mut guard) = self.session.lock() {
            *guard = Some(session);
        }
    }

    pub fn clear_session(&self) {
        if let Ok(mut guard) = self.session.lock() {
            *guard = None;
        }
    }

    pub fn update_inventory(&self, inventory: Arc<Inventory>) {
        if let Ok(mut guard) = self.session.lock() {
            if let Some(session) = guard.as_mut() {
                session.inventory = inventory;
            }
        }
    }

    /// Registers a cancellation token for the running scan, cancelling any
    /// scan that is still in flight.
    pub fn begin_scan(&self) -> CancelToken {
        let token = CancelToken::new();
        if let Ok(mut guard) = self.scan_cancel.lock() {
            if let Some(previous) = guard.take() {
                previous.cancel();
            }
            *guard = Some(token.clone());
        }
        token
    }

    pub fn cancel_scan(&self) -> bool {
        if let Ok(mut guard) = self.scan_cancel.lock() {
            if let Some(token) = guard.take() {
                token.cancel();
                return true;
            }
        }
        false
    }

    pub fn end_scan(&self) {
        if let Ok(mut guard) = self.scan_cancel.lock() {
            *guard = None;
        }
    }

    /// Registers a new clone. Only one runs at a time: two large downloads
    /// split the connection and both crawl.
    pub fn begin_clone(&self) -> AppResult<Arc<CloneJob>> {
        let mut guard = self
            .clone_job
            .lock()
            .map_err(|_| AppError::internal("the clone lock was poisoned"))?;
        if guard.is_some() {
            return Err(AppError::new(
                "clone_in_progress",
                "A repository is already being cloned.",
            )
            .with_recovery("Wait for it to finish, or stop it first."));
        }
        let job = Arc::new(CloneJob::default());
        *guard = Some(job.clone());
        Ok(job)
    }

    pub fn end_clone(&self) {
        if let Ok(mut guard) = self.clone_job.lock() {
            *guard = None;
        }
    }

    /// Cancels the clone in progress. Returns false when nothing was running.
    pub fn cancel_clone(&self) -> bool {
        let job = self.clone_job.lock().ok().and_then(|guard| guard.clone());
        job.is_some_and(|job| job.cancel())
    }
}
