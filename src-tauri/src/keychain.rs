//! OS secure storage abstraction (ADR 0007).
//!
//! Credentials live in the platform keychain under `dev.leanai.desktop.{provider}`.
//! SQLite stores only a non-secret reference (`is_configured: bool`).
//! A credential is read into memory only at the moment of request and is never
//! written to SQLite, logs, or error strings.

use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::{Arc, Mutex};

use crate::error::{AppError, AppResult};

pub const SERVICE_PREFIX: &str = "dev.leanai.desktop";

/// Secure credential store abstraction.
#[derive(Clone, Default)]
pub struct KeychainStore {
    mock_store: Option<Arc<Mutex<HashMap<String, String>>>>,
}

impl KeychainStore {
    /// Creates a keychain store. In test environments or when `LEANAI_MOCK_KEYCHAIN`
    /// is set, an in-memory store is used to prevent interactive prompts or headless CI failures.
    pub fn new() -> Self {
        if std::env::var("LEANAI_MOCK_KEYCHAIN").is_ok() || cfg!(test) {
            Self::mock()
        } else {
            Self { mock_store: None }
        }
    }

    /// Creates an explicit in-memory store for unit and integration testing.
    pub fn mock() -> Self {
        Self {
            mock_store: Some(Arc::new(Mutex::new(HashMap::new()))),
        }
    }

    fn service_name(provider_id: &str) -> String {
        format!("{}.{}", SERVICE_PREFIX, provider_id)
    }

    /// Stores a secret for the given provider in the OS keychain.
    pub fn store(&self, provider_id: &str, account_label: &str, secret: &str) -> AppResult<()> {
        if let Some(store) = &self.mock_store {
            let mut guard = store.lock().unwrap();
            guard.insert(Self::service_name(provider_id), secret.to_string());
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            let service = Self::service_name(provider_id);
            let status = Command::new("/usr/bin/security")
                .args([
                    "add-generic-password",
                    "-a",
                    account_label,
                    "-s",
                    &service,
                    "-w",
                    secret,
                    "-U",
                ])
                .status()
                .map_err(|e| {
                    AppError::new(
                        "keychain_error",
                        format!("Failed to invoke macOS keychain: {e}"),
                    )
                })?;

            if !status.success() {
                return Err(AppError::new(
                    "keychain_error",
                    "OS secure credential storage rejected the request.",
                )
                .with_recovery("Ensure the macOS login keychain is unlocked and accessible."));
            }
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (provider_id, account_label, secret);
            Err(AppError::new(
                "keychain_unavailable",
                "OS secure credential storage is unavailable on this platform.",
            ))
        }
    }

    /// Reads the secret for the given provider.
    pub fn read(&self, provider_id: &str) -> AppResult<Option<String>> {
        if let Some(store) = &self.mock_store {
            let guard = store.lock().unwrap();
            return Ok(guard.get(&Self::service_name(provider_id)).cloned());
        }

        #[cfg(target_os = "macos")]
        {
            let service = Self::service_name(provider_id);
            let output = Command::new("/usr/bin/security")
                .args(["find-generic-password", "-s", &service, "-w"])
                .output()
                .map_err(|e| {
                    AppError::new(
                        "keychain_error",
                        format!("Failed to query macOS keychain: {e}"),
                    )
                })?;

            if !output.status.success() {
                return Ok(None);
            }
            let secret = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if secret.is_empty() {
                Ok(None)
            } else {
                Ok(Some(secret))
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = provider_id;
            Err(AppError::new(
                "keychain_unavailable",
                "OS secure credential storage is unavailable on this platform.",
            ))
        }
    }

    /// Deletes the secret for the given provider from the keychain.
    pub fn delete(&self, provider_id: &str) -> AppResult<bool> {
        if let Some(store) = &self.mock_store {
            let mut guard = store.lock().unwrap();
            return Ok(guard.remove(&Self::service_name(provider_id)).is_some());
        }

        #[cfg(target_os = "macos")]
        {
            let service = Self::service_name(provider_id);
            let status = Command::new("/usr/bin/security")
                .args(["delete-generic-password", "-s", &service])
                .status()
                .map_err(|e| {
                    AppError::new(
                        "keychain_error",
                        format!("Failed to delete from macOS keychain: {e}"),
                    )
                })?;

            Ok(status.success())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = provider_id;
            Err(AppError::new(
                "keychain_unavailable",
                "OS secure credential storage is unavailable on this platform.",
            ))
        }
    }

    /// Checks if a credential exists in storage.
    pub fn is_configured(&self, provider_id: &str) -> AppResult<bool> {
        self.read(provider_id).map(|opt| opt.is_some())
    }
}
