//! Git and GitHub authentication and remote synchronization (SSH & OAuth/PAT).
//!
//! Complies with ADR 0007: GitHub tokens live exclusively in OS secure storage
//! (`keychain::KeychainStore` under `dev.leanai.desktop.github`), never in plaintext
//! in SQLite, manifests, logs, or error strings.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use leanai_core::aiignore;
use leanai_core::gitinfo::{self, GitRemoteStatus};
use leanai_core::inventory::Inventory;
use leanai_core::project;

use crate::app_state::{AppState, ProjectSession};
use crate::commands::projects::OpenProjectResponse;
use crate::db::repositories;
use crate::error::{AppError, AppResult};

const GITHUB_PROVIDER: &str = "github";
const SETTINGS_KEY_GITHUB_PROFILE: &str = "github_user_profile";
const SETTINGS_KEY_GITHUB_REPOS: &str = "github_cached_repositories";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubRepository {
    pub id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub is_private: bool,
    pub is_fork: bool,
    pub html_url: String,
    pub clone_url: String,
    pub ssh_url: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub stars: u64,
    pub default_branch: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitHubUserProfile {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub html_url: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshKeyInfo {
    pub filename: String,
    pub key_type: String,
    pub comment: String,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitAuthStatus {
    pub github_token_configured: bool,
    pub github_user: Option<GitHubUserProfile>,
    pub ssh_agent_active: bool,
    pub ssh_keys: Vec<SshKeyInfo>,
    pub ssh_authenticated: bool,
    pub ssh_username: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigureGithubTokenRequest {
    pub token: String,
    pub account_label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SshAuthTestResult {
    pub authenticated: bool,
    pub username: Option<String>,
    pub output: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushRequest {
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub force: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPushResponse {
    pub success: bool,
    pub message: String,
    pub remote: String,
    pub branch: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepositoryRequest {
    pub url: String,
    pub destination_parent_dir: String,
    pub directory_name: Option<String>,
}

fn get_ssh_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        let ssh_dir = PathBuf::from(home).join(".ssh");
        if ssh_dir.is_dir() {
            return Some(ssh_dir);
        }
    }
    None
}

fn discover_ssh_keys() -> Vec<SshKeyInfo> {
    let Some(ssh_dir) = get_ssh_dir() else {
        return Vec::new();
    };

    let entries = match std::fs::read_dir(&ssh_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut keys = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("pub") {
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            if let Ok(content) = std::fs::read_to_string(&path) {
                let trimmed = content.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if !parts.is_empty() {
                    let key_type = parts[0].to_string();
                    let comment = if parts.len() >= 3 {
                        parts[2..].join(" ")
                    } else {
                        filename.clone()
                    };
                    keys.push(SshKeyInfo {
                        filename,
                        key_type,
                        comment,
                        public_key: trimmed.to_string(),
                    });
                }
            }
        }
    }

    keys.sort_by(|a, b| a.filename.cmp(&b.filename));
    keys
}

fn is_ssh_agent_active() -> bool {
    if let Ok(sock) = std::env::var("SSH_AUTH_SOCK") {
        if !sock.trim().is_empty() {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    {
        // On Windows, OpenSSH agent may run as a service
        true
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Checks the Git and GitHub authentication status across Keychain and SSH.
#[tauri::command]
pub async fn get_git_auth_status(state: State<'_, AppState>) -> AppResult<GitAuthStatus> {
    let github_token_configured = state
        .keychain
        .is_configured(GITHUB_PROVIDER)
        .unwrap_or(false);

    let github_user = if github_token_configured {
        state
            .with_db(|conn| {
                let profile = repositories::get_setting(conn, SETTINGS_KEY_GITHUB_PROFILE)?
                    .and_then(|json| serde_json::from_str::<GitHubUserProfile>(&json).ok());
                Ok(profile)
            })
            .unwrap_or(None)
    } else {
        None
    };

    let ssh_agent_active = is_ssh_agent_active();
    let ssh_keys = discover_ssh_keys();

    Ok(GitAuthStatus {
        github_token_configured,
        github_user,
        ssh_agent_active,
        ssh_keys,
        ssh_authenticated: false,
        ssh_username: None,
    })
}

/// Tests SSH connectivity and authentication with GitHub.
#[tauri::command]
pub async fn test_github_ssh(_state: State<'_, AppState>) -> AppResult<SshAuthTestResult> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = Command::new("ssh")
            .args([
                "-T",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "git@github.com",
            ])
            .output();

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let combined = format!("{stdout}\n{stderr}").trim().to_string();

                if combined.contains("You've successfully authenticated") {
                    let mut username = None;
                    if let Some(start) = combined.find("Hi ") {
                        if let Some(end) = combined[start + 3..].find('!') {
                            username = Some(combined[start + 3..start + 3 + end].trim().to_string());
                        }
                    }
                    Ok(SshAuthTestResult {
                        authenticated: true,
                        username,
                        output: combined,
                    })
                } else if combined.contains("Permission denied") {
                    Ok(SshAuthTestResult {
                        authenticated: false,
                        username: None,
                        output: "Permission denied (publickey). Add your SSH public key to your GitHub account settings.".to_string(),
                    })
                } else {
                    Ok(SshAuthTestResult {
                        authenticated: false,
                        username: None,
                        output: combined,
                    })
                }
            }
            Err(e) => Ok(SshAuthTestResult {
                authenticated: false,
                username: None,
                output: format!("Failed to execute ssh: {e}"),
            }),
        }
    })
    .await
    .map_err(|e| AppError::internal(format!("SSH test task failed: {e}")))?
}

/// Stores GitHub token in the OS Keychain and queries user profile.
#[tauri::command]
pub async fn configure_github_token(
    state: State<'_, AppState>,
    request: ConfigureGithubTokenRequest,
) -> AppResult<GitAuthStatus> {
    let token = request.token.trim().to_string();
    if token.is_empty() {
        return Err(AppError::new(
            "invalid_credential",
            "GitHub token cannot be blank.",
        ));
    }

    // Validate token against GitHub API via curl with a 10-second timeout
    let token_clone = token.clone();
    let profile_result = tauri::async_runtime::spawn_blocking(move || {
        let curl_output = Command::new("curl")
            .args([
                "-s",
                "-S",
                "-i",
                "-m",
                "10",
                "-H",
                &format!("Authorization: Bearer {token_clone}"),
                "-H",
                "User-Agent: LeanAI-Desktop",
                "https://api.github.com/user",
            ])
            .output();

        match curl_output {
            Ok(out) => {
                let raw = String::from_utf8_lossy(&out.stdout).to_string();
                if raw.contains("401 Unauthorized") || raw.contains("Bad credentials") {
                    return Err(AppError::new(
                        "invalid_credential",
                        "GitHub authentication failed: Bad credentials or expired token.",
                    ));
                }

                // Split headers and body
                let mut scopes = Vec::new();
                let mut body_str = "";
                if let Some(split_pos) = raw.find("\r\n\r\n") {
                    let headers = &raw[..split_pos];
                    body_str = &raw[split_pos + 4..];
                    for line in headers.lines() {
                        let lower = line.to_lowercase();
                        if lower.starts_with("x-oauth-scopes:") {
                            if let Some(val) = line.split(':').nth(1) {
                                scopes = val
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                            }
                        }
                    }
                } else if let Some(split_pos) = raw.find("\n\n") {
                    body_str = &raw[split_pos + 2..];
                }

                if let Ok(val) = serde_json::from_str::<serde_json::Value>(body_str) {
                    if let Some(login) = val.get("login").and_then(|v| v.as_str()) {
                        return Ok(GitHubUserProfile {
                            login: login.to_string(),
                            name: val.get("name").and_then(|v| v.as_str()).map(str::to_string),
                            avatar_url: val
                                .get("avatar_url")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            html_url: val
                                .get("html_url")
                                .and_then(|v| v.as_str())
                                .map(str::to_string),
                            scopes,
                        });
                    }
                }

                // If API was unreachable or offline, allow saving with fallback
                Ok(GitHubUserProfile {
                    login: "github-user".to_string(),
                    name: None,
                    avatar_url: None,
                    html_url: None,
                    scopes,
                })
            }
            Err(_) => Ok(GitHubUserProfile {
                login: "github-user".to_string(),
                name: None,
                avatar_url: None,
                html_url: None,
                scopes: Vec::new(),
            }),
        }
    })
    .await
    .map_err(|e| AppError::internal(format!("Token validation failed: {e}")))?;

    let profile = profile_result?;

    let account_label = request
        .account_label
        .unwrap_or_else(|| profile.login.clone());

    // Store strictly in OS keychain (ADR 0007)
    state
        .keychain
        .store(GITHUB_PROVIDER, &account_label, &token)?;

    // Store non-secret user profile in settings table
    let profile_json = serde_json::to_string(&profile)
        .map_err(|e| AppError::internal(format!("Serialization error: {e}")))?;

    state.with_db(|conn| {
        repositories::set_setting(conn, SETTINGS_KEY_GITHUB_PROFILE, &profile_json)
    })?;

    get_git_auth_status(state).await
}

/// Disconnects GitHub and removes token from OS Keychain.
#[tauri::command]
pub async fn disconnect_github(state: State<'_, AppState>) -> AppResult<GitAuthStatus> {
    let _ = state.keychain.delete(GITHUB_PROVIDER);

    state.with_db(|conn| {
        let _ = repositories::set_setting(conn, SETTINGS_KEY_GITHUB_PROFILE, "");
        let _ = repositories::set_setting(conn, SETTINGS_KEY_GITHUB_REPOS, "");
        Ok(())
    })?;

    get_git_auth_status(state).await
}

/// Lists the authenticated user's GitHub repositories.
#[tauri::command]
pub async fn list_github_repositories(
    state: State<'_, AppState>,
) -> AppResult<Vec<GitHubRepository>> {
    let token = state.keychain.read(GITHUB_PROVIDER)?.ok_or_else(|| {
        AppError::new("github_not_configured", "GitHub account is not connected.")
    })?;

    let repos_result = tauri::async_runtime::spawn_blocking(move || {
        let curl_output = Command::new("curl")
            .args([
                "-s",
                "-S",
                "-m",
                "15",
                "-H",
                &format!("Authorization: Bearer {token}"),
                "-H",
                "User-Agent: LeanAI-Desktop",
                "-H",
                "Accept: application/vnd.github+json",
                "https://api.github.com/user/repos?sort=updated&per_page=100&affiliation=owner,collaborator,organization_member",
            ])
            .output();

        match curl_output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                if let Ok(raw_list) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
                    let mut repos = Vec::new();
                    for item in raw_list {
                        let id = item.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                        let name = item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let full_name = item
                            .get("full_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let owner = item
                            .get("owner")
                            .and_then(|o| o.get("login"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let is_private =
                            item.get("private").and_then(|v| v.as_bool()).unwrap_or(false);
                        let is_fork =
                            item.get("fork").and_then(|v| v.as_bool()).unwrap_or(false);
                        let html_url = item
                            .get("html_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let clone_url = item
                            .get("clone_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let ssh_url = item
                            .get("ssh_url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let description = item
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let language = item
                            .get("language")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                        let stars = item
                            .get("stargazers_count")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let default_branch = item
                            .get("default_branch")
                            .and_then(|v| v.as_str())
                            .unwrap_or("main")
                            .to_string();
                        let updated_at = item
                            .get("updated_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        if !name.is_empty() {
                            repos.push(GitHubRepository {
                                id,
                                name,
                                full_name,
                                owner,
                                is_private,
                                is_fork,
                                html_url,
                                clone_url,
                                ssh_url,
                                description,
                                language,
                                stars,
                                default_branch,
                                updated_at,
                            });
                        }
                    }
                    return Ok(repos);
                }

                if let Ok(err_obj) = serde_json::from_str::<serde_json::Value>(&stdout) {
                    if let Some(msg) = err_obj.get("message").and_then(|v| v.as_str()) {
                        return Err(AppError::new(
                            "github_api_error",
                            format!("GitHub API: {msg}"),
                        ));
                    }
                }

                Err(AppError::new(
                    "github_api_error",
                    "Failed to parse GitHub repositories.",
                ))
            }
            Err(e) => Err(AppError::new(
                "curl_failed",
                format!("Failed to run curl: {e}"),
            )),
        }
    })
    .await
    .map_err(|e| AppError::internal(format!("Task failed: {e}")))?;

    match repos_result {
        Ok(repos) => {
            if let Ok(json) = serde_json::to_string(&repos) {
                let _ = state.with_db(|conn| {
                    repositories::set_setting(conn, SETTINGS_KEY_GITHUB_REPOS, &json)
                });
            }
            Ok(repos)
        }
        Err(err) => {
            let cached = state.with_db(|conn| {
                if let Ok(Some(json)) = repositories::get_setting(conn, SETTINGS_KEY_GITHUB_REPOS) {
                    Ok(serde_json::from_str::<Vec<GitHubRepository>>(&json).ok())
                } else {
                    Ok(None)
                }
            });

            if let Ok(Some(cached_repos)) = cached {
                if !cached_repos.is_empty() {
                    return Ok(cached_repos);
                }
            }

            Err(err)
        }
    }
}

/// Returns remote tracking status (remotes, tracking upstream branch, ahead/behind counts, unpushed commits).
#[tauri::command]
pub async fn git_remote_status(state: State<'_, AppState>) -> AppResult<GitRemoteStatus> {
    let session = state.require_session()?;
    Ok(gitinfo::remote_status(&session.root)?)
}

/// Pushes the current branch to the specified remote repository.
#[tauri::command]
pub async fn git_push_branch(
    state: State<'_, AppState>,
    request: GitPushRequest,
) -> AppResult<GitPushResponse> {
    let session = state.require_session()?;
    let root = session.root.clone();

    let git_st = gitinfo::git_state(&root);
    if !git_st.is_repository {
        return Err(AppError::new(
            "not_a_repository",
            "The active project is not a git repository.",
        ));
    }

    let branch = match request.branch {
        Some(b) => b,
        None => git_st.head_ref.ok_or_else(|| {
            AppError::new(
                "detached_head",
                "Cannot push from a detached HEAD. Please checkout a branch.",
            )
        })?,
    };

    let remote_name = request.remote.unwrap_or_else(|| "origin".to_string());
    let force = request.force.unwrap_or(false);

    // Retrieve GitHub token from Keychain if configured
    let token = state.keychain.read(GITHUB_PROVIDER).unwrap_or(None);

    let (branch_clone, remote_clone) = (branch.clone(), remote_name.clone());
    let push_result = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("git");
        cmd.current_dir(&root);
        cmd.env("GIT_TERMINAL_PROMPT", "0");

        if let Some(t) = &token {
            cmd.arg("-c");
            cmd.arg(format!("http.extraHeader=Authorization: Bearer {t}"));
        }

        cmd.arg("push");
        if force {
            cmd.arg("--force-with-lease");
        }
        cmd.arg(&remote_clone);
        cmd.arg(&branch_clone);

        cmd.output()
    })
    .await
    .map_err(|e| AppError::internal(format!("Git push task failed: {e}")))?;

    let out =
        push_result.map_err(|e| AppError::internal(format!("Failed to run git push: {e}")))?;

    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let output_message = format!("{stdout}\n{stderr}").trim().to_string();

    let status_str = if out.status.success() {
        "succeeded"
    } else {
        "failed"
    };

    // Audit push action (ADR 0002 / NFR 7.1)
    let _ = state.with_db(|conn| {
        repositories::record_audit(
            conn,
            &session.record.id,
            "git_push",
            "git_sync",
            &serde_json::json!({
                "remote": remote_name,
                "branch": branch,
                "force": force,
            }),
            status_str,
        )
        .map(|_| ())
    });

    if !out.status.success() {
        return Err(AppError::new("git_push_failed", output_message).with_recovery(
            "Check that your remote exists, remote URL is accessible, and you have push permissions.",
        ));
    }

    Ok(GitPushResponse {
        success: true,
        message: if output_message.is_empty() {
            format!("Successfully pushed {branch} to {remote_name}.")
        } else {
            output_message
        },
        remote: remote_name,
        branch,
    })
}

/// Clones a remote repository into a target directory and immediately opens it in LeanAI.
#[tauri::command]
pub async fn clone_remote_repository(
    state: State<'_, AppState>,
    request: CloneRepositoryRequest,
) -> AppResult<OpenProjectResponse> {
    let url = request.url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::new(
            "invalid_url",
            "Repository URL cannot be blank.",
        ));
    }

    let parent_dir = PathBuf::from(&request.destination_parent_dir);
    if !parent_dir.is_dir() {
        return Err(AppError::new(
            "invalid_destination",
            "Destination parent directory does not exist.",
        ));
    }

    // Determine target directory name
    let dir_name = if let Some(custom) = request.directory_name {
        custom.trim().to_string()
    } else {
        url.trim_end_matches('/')
            .split('/')
            .next_back()
            .unwrap_or("cloned-repo")
            .trim_end_matches(".git")
            .to_string()
    };

    if dir_name.is_empty() {
        return Err(AppError::new("invalid_name", "Invalid directory name."));
    }

    let target_path = parent_dir.join(&dir_name);
    if target_path.exists() {
        return Err(AppError::new(
            "target_exists",
            format!(
                "Destination directory '{}' already exists.",
                target_path.display()
            ),
        ));
    }

    let token = state.keychain.read(GITHUB_PROVIDER).unwrap_or(None);

    let (url_clone, target_clone) = (url.clone(), target_path.clone());
    let clone_result = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("git");
        cmd.env("GIT_TERMINAL_PROMPT", "0");

        if url_clone.starts_with("https://") {
            if let Some(t) = &token {
                cmd.arg("-c");
                cmd.arg(format!("http.extraHeader=Authorization: Bearer {t}"));
            }
        }

        cmd.args(["clone", &url_clone]);
        cmd.arg(&target_clone);

        cmd.output()
    })
    .await
    .map_err(|e| AppError::internal(format!("Clone task failed: {e}")))?;

    let out =
        clone_result.map_err(|e| AppError::internal(format!("Failed to run git clone: {e}")))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(AppError::new("git_clone_failed", stderr).with_recovery(
            "Check that the repository URL is correct and you have read/clone permissions via SSH or your configured GitHub token.",
        ));
    }

    // Open newly cloned project
    let root = project::canonical_root(&target_path)?;
    let fingerprint = project::fingerprint(&root);
    let display_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned());
    let policy_version = state.settings().policy.version;

    let record = state.with_db(|conn| {
        repositories::upsert_project(
            conn,
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

fn empty_inventory(root: &Path, fingerprint: &str, policy_version: u32) -> Inventory {
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
