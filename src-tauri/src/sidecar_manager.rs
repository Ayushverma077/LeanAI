//! Managed local inference sidecar (`llama-server`) runtime (ADR 0009).
//!
//! Enforces:
//! - Loopback binding only (`127.0.0.1`)
//! - Ephemeral port allocation to avoid collisions
//! - Explicit lifecycle state machine: Stopped -> Starting -> Ready -> Stopping / Error
//! - GGUF preflight inspection before process spawn
//! - Guaranteed process termination on stop and on Drop (no orphan processes)

use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// Lifecycle state machine for the local model runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SidecarStatus {
    Stopped,
    Starting {
        port: u16,
        #[serde(rename = "modelId")]
        model_id: String,
    },
    Ready {
        port: u16,
        pid: u32,
        #[serde(rename = "modelId")]
        model_id: String,
        #[serde(rename = "displayName")]
        display_name: String,
    },
    Stopping,
    Error {
        message: String,
    },
}

/// Manages the `llama-server` sidecar process.
pub struct SidecarManager {
    status: Mutex<SidecarStatus>,
    child: Mutex<Option<Child>>,
    binary_override: Mutex<Option<PathBuf>>,
    mock_healthy: Mutex<bool>,
}

impl Default for SidecarManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SidecarManager {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(SidecarStatus::Stopped),
            child: Mutex::new(None),
            binary_override: Mutex::new(None),
            mock_healthy: Mutex::new(false),
        }
    }

    /// Sets an explicit executable path (useful for testing or custom installs).
    pub fn set_binary_override(&self, path: Option<PathBuf>) {
        if let Ok(mut guard) = self.binary_override.lock() {
            *guard = path;
        }
    }

    /// Enables mock health verification for sandbox or CI testing where loopback network binds are restricted.
    pub fn set_mock_healthy(&self, mock: bool) {
        if let Ok(mut guard) = self.mock_healthy.lock() {
            *guard = mock;
        }
    }

    /// Returns the current lifecycle state.
    pub fn status(&self) -> SidecarStatus {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or(SidecarStatus::Error {
                message: "Sidecar status lock poisoned".to_string(),
            })
    }

    /// Resolves the sidecar binary path.
    pub fn resolve_binary(&self) -> Option<PathBuf> {
        if let Ok(guard) = self.binary_override.lock() {
            if let Some(p) = guard.as_ref() {
                if p.exists() {
                    return Some(p.clone());
                }
            }
        }

        if let Ok(env_path) = std::env::var("LLAMA_SERVER_PATH") {
            let p = PathBuf::from(env_path);
            if p.exists() {
                return Some(p);
            }
        }

        // Check if `llama-server` is in PATH
        which("llama-server")
    }

    /// Preflights and starts a local model sidecar.
    pub fn start(
        &self,
        model_id: &str,
        display_name: &str,
        model_path: &Path,
        context_size: Option<usize>,
    ) -> AppResult<SidecarStatus> {
        // 1. Preflight: Model path validation
        if !model_path.is_file() {
            let err = AppError::new(
                "model_not_found",
                format!("Model file `{}` does not exist.", model_path.display()),
            )
            .with_recovery("Verify that the GGUF file exists on disk.");
            self.set_error(&err.message);
            return Err(err);
        }

        // 2. Preflight: Inspect GGUF header to verify format integrity
        leanai_core::gguf::inspect_gguf_file(model_path).map_err(|e| {
            let app_err = AppError::from(e);
            self.set_error(&app_err.message);
            app_err
        })?;

        // 3. Stop any existing sidecar first
        self.stop()?;

        // 4. Resolve binary
        let binary = match self.resolve_binary() {
            Some(b) => b,
            None => {
                let err = AppError::new(
                    "sidecar_binary_missing",
                    "`llama-server` binary was not found on this system.",
                )
                .with_recovery(
                    "Install llama-server or set the LLAMA_SERVER_PATH environment variable.",
                );
                self.set_error(&err.message);
                return Err(err);
            }
        };

        // 5. Allocate ephemeral port strictly on loopback (127.0.0.1)
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| {
            let err = AppError::new(
                "port_bind_failed",
                format!("Failed to allocate ephemeral loopback port: {e}"),
            );
            self.set_error(&err.message);
            err
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| AppError::new("io", e.to_string()))?
            .port();
        drop(listener); // Free the port for llama-server to bind

        // Update status to Starting
        if let Ok(mut guard) = self.status.lock() {
            *guard = SidecarStatus::Starting {
                port,
                model_id: model_id.to_string(),
            };
        }

        // 6. Spawn process with strictly loopback arguments
        let mut cmd = Command::new(&binary);
        cmd.arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("-m")
            .arg(model_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        if let Some(ctx) = context_size {
            cmd.arg("-c").arg(ctx.to_string());
        }

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err = AppError::new(
                    "sidecar_spawn_failed",
                    format!("Failed to spawn llama-server: {e}"),
                );
                self.set_error(&err.message);
                return Err(err);
            }
        };

        let pid = child.id();
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }

        // 7. Health check: Wait for loopback listener while monitoring child process
        let mut exit_status = None;
        let healthy = if self.mock_healthy.lock().map(|g| *g).unwrap_or(false) {
            true
        } else {
            let start = Instant::now();
            let timeout = Duration::from_secs(4);
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            let mut is_ok = false;
            while start.elapsed() < timeout {
                if let Ok(mut guard) = self.child.lock() {
                    if let Some(child_ref) = guard.as_mut() {
                        if let Ok(Some(st)) = child_ref.try_wait() {
                            exit_status = Some(st);
                            break;
                        }
                    }
                }
                if TcpStream::connect_timeout(&addr, Duration::from_millis(50)).is_ok() {
                    is_ok = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            is_ok
        };

        if !healthy {
            let _ = self.stop();
            let msg = if let Some(st) = exit_status {
                format!("llama-server sidecar exited prematurely with status: {st}")
            } else {
                "llama-server sidecar failed health check after startup.".to_string()
            };
            let err = AppError::new("sidecar_health_failed", msg)
                .with_recovery("Check system resources or model compatibility.");
            self.set_error(&err.message);
            return Err(err);
        }

        let ready_status = SidecarStatus::Ready {
            port,
            pid,
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
        };

        if let Ok(mut guard) = self.status.lock() {
            *guard = ready_status.clone();
        }

        Ok(ready_status)
    }

    /// Stops the sidecar process and reclaims resources.
    pub fn stop(&self) -> AppResult<SidecarStatus> {
        if let Ok(mut guard) = self.status.lock() {
            *guard = SidecarStatus::Stopping;
        }

        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        let stopped = SidecarStatus::Stopped;
        if let Ok(mut guard) = self.status.lock() {
            *guard = stopped.clone();
        }

        Ok(stopped)
    }

    fn set_error(&self, message: &str) {
        if let Ok(mut guard) = self.status.lock() {
            *guard = SidecarStatus::Error {
                message: message.to_string(),
            };
        }
    }
}

impl Drop for SidecarManager {
    fn drop(&mut self) {
        // Guaranteed termination: never leave an orphan process running (ADR 0009).
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Helper to find executable in PATH.
fn which(name: &str) -> Option<PathBuf> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}
