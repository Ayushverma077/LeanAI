//! Git and GitHub authentication and remote synchronization (SSH & OAuth/PAT).
//!
//! Complies with ADR 0007: GitHub tokens live exclusively in OS secure storage
//! (`keychain::KeychainStore` under `dev.leanai.desktop.github`), never in plaintext
//! in SQLite, manifests, logs, or error strings.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use leanai_core::aiignore;
use leanai_core::gitinfo::{self, GitRemoteStatus};
use leanai_core::inventory::Inventory;
use leanai_core::project;

use crate::app_state::{AppState, CloneJob, ProjectSession};
use crate::commands::projects::OpenProjectResponse;
use crate::db::repositories;
use crate::error::{AppError, AppResult};

const GITHUB_PROVIDER: &str = "github";
const SETTINGS_KEY_GITHUB_PROFILE: &str = "github_user_profile";
const SETTINGS_KEY_GITHUB_REPOS: &str = "github_cached_repositories";

/// Git's URL-scoped config key. Git itself only attaches this header when the
/// URL it was given starts with `https://github.com/`, so the token cannot
/// reach another host through a custom clone URL or a non-GitHub push remote.
const GITHUB_EXTRAHEADER_KEY: &str = "http.https://github.com/.extraheader";

/// True only for HTTPS URLs whose host is exactly `github.com`.
///
/// This decides whether the stored GitHub token may be used at all. It is
/// deliberately strict: plain `http://` (the token would travel unencrypted),
/// look-alike hosts such as `github.com.evil.example`, and userinfo tricks
/// such as `https://github.com@evil.example/` are all rejected.
fn is_github_https_url(url: &str) -> bool {
    let Some(rest) = url.trim().strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host_and_port = authority.rsplit('@').next().unwrap_or("");
    let host = host_and_port.split(':').next().unwrap_or("");
    host.eq_ignore_ascii_case("github.com")
}

/// Standard base64 (RFC 4648 §4). Local rather than a new dependency for the
/// single use below; covered by the RFC's own test vectors.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(chunk[0]) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The authorisation header for git over HTTPS.
///
/// GitHub documents Basic auth with the token as the password for git
/// operations over HTTPS. The code previously sent `Bearer <token>` — the REST
/// API's scheme — the likeliest reason a token that lists repositories fine
/// still failed a clone with `remote: invalid credentials`.
fn github_basic_auth_header(token: &str) -> String {
    format!(
        "Authorization: Basic {}",
        base64_encode(format!("x-access-token:{token}").as_bytes())
    )
}

/// Attaches the GitHub token to a git command.
///
/// Passed through `GIT_CONFIG_*` environment variables instead of `-c`
/// arguments: a process's arguments are readable by every local user through
/// `ps` for as long as git runs; another user's environment is not.
fn apply_github_auth(cmd: &mut Command, token: &str) {
    cmd.env("GIT_CONFIG_COUNT", "1");
    cmd.env("GIT_CONFIG_KEY_0", GITHUB_EXTRAHEADER_KEY);
    cmd.env("GIT_CONFIG_VALUE_0", github_basic_auth_header(token));
}

/// Recognises git's authentication failures, so a clone can retry
/// anonymously instead of failing a public repository over a bad token.
///
/// "Repository not found" is intentionally *not* an auth failure: under a
/// valid token it means no access, and an anonymous retry cannot help.
fn is_auth_failure(stderr: &str) -> bool {
    let lower = stderr.to_ascii_lowercase();
    [
        "authentication failed",
        "invalid credentials",
        "could not read username",
        "could not read password",
        "the requested url returned error: 401",
        "the requested url returned error: 403",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// A transfer that stays below this many bytes per second for
/// `STALL_WINDOW_SECONDS` is aborted, so a stalled clone fails with a message
/// instead of waiting forever.
const STALL_LIMIT_BYTES_PER_SECOND: u32 = 1_000;
const STALL_WINDOW_SECONDS: u32 = 60;

/// Event carrying `CloneProgress` updates to the UI.
pub const CLONE_PROGRESS_EVENT: &str = "leanai://clone-progress";

/// How much of the repository to download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloneDepth {
    /// The latest snapshot only (`--depth 1`). The default: LeanAI bundles
    /// current files, and for a large repository the snapshot is a small
    /// fraction of its history (calcom/cal.diy is 1.1 GB with history).
    Shallow,
    /// Complete history, for `git log` or diffs against older commits.
    Full,
}

/// Builds `git clone`.
///
/// - The URL and target come after `--`, so neither can be read as an option:
///   a "URL" such as `--upload-pack=<command>` would otherwise make git run an
///   arbitrary program.
/// - `--progress` makes git report progress even though it is not writing to a
///   terminal. Without it a GUI sees nothing until the clone ends.
/// - `LC_ALL=C` keeps git's messages in English, because both the progress
///   parser and the rejected-token fallback read them. On a translated system
///   the fallback would otherwise silently stop working.
/// - git gets its own process group on Unix, so cancelling reaches the helpers
///   it starts (`git-remote-https`, `index-pack`) as well as git itself.
fn build_clone_command(
    url: &str,
    target: &Path,
    token: Option<&str>,
    depth: CloneDepth,
) -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env("LC_ALL", "C");
    if let Some(t) = token {
        apply_github_auth(&mut cmd, t);
    }
    cmd.arg("-c")
        .arg(format!("http.lowSpeedLimit={STALL_LIMIT_BYTES_PER_SECOND}"));
    cmd.arg("-c")
        .arg(format!("http.lowSpeedTime={STALL_WINDOW_SECONDS}"));
    cmd.arg("clone").arg("--progress");
    if depth == CloneDepth::Shallow {
        cmd.arg("--depth").arg("1");
    }
    cmd.arg("--").arg(url).arg(target);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd
}

/// One progress update from `git clone --progress`, forwarded to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneProgress {
    /// e.g. "Receiving objects", "Resolving deltas", "Updating files".
    pub phase: String,
    pub percent: u8,
    pub current: u64,
    pub total: u64,
    /// Amount received so far, as git formats it ("45.67 MiB"). Only present
    /// while objects are being received.
    pub transferred: Option<String>,
    /// Throughput, as git formats it ("2.31 MiB/s").
    pub speed: Option<String>,
}

/// Parses one `\r`- or `\n`-delimited segment of git's progress output, such
/// as `Receiving objects:  23% (2345/10234), 45.67 MiB | 2.31 MiB/s`. Anything
/// else — including errors — returns `None`.
fn parse_git_progress(raw: &str) -> Option<CloneProgress> {
    let line = raw.trim();
    let line = line.strip_prefix("remote:").map(str::trim).unwrap_or(line);
    let (phase, rest) = line.split_once(':')?;
    let (percent, rest) = rest.split_once('%')?;
    let percent: u8 = percent.trim().parse().ok()?;
    let rest = rest.trim_start().strip_prefix('(')?;
    let (counts, tail) = rest.split_once(')')?;
    let (current, total) = counts.split_once('/')?;
    let current: u64 = current.trim().parse().ok()?;
    let total: u64 = total.trim().parse().ok()?;

    let tail = tail.trim().trim_start_matches(',').trim();
    let tail = tail
        .strip_suffix(", done.")
        .or_else(|| tail.strip_suffix("done."))
        .unwrap_or(tail)
        .trim();
    let (transferred, speed) = match tail.split_once('|') {
        Some((amount, rate)) => (
            Some(amount.trim().to_string()),
            Some(rate.trim().to_string()),
        ),
        None => (None, None),
    };

    Some(CloneProgress {
        phase: phase.trim().to_string(),
        percent: percent.min(100),
        current,
        total,
        transferred,
        speed,
    })
}

/// Turns git's raw progress stream into UI updates, and keeps the non-progress
/// lines — where errors such as `fatal: ...` appear — for the failure message.
struct ProgressForwarder {
    last: Option<(String, u8)>,
    last_emit: Instant,
    tail: Vec<String>,
}

impl ProgressForwarder {
    /// Minimum gap between updates that change only the byte count or speed.
    const INTERVAL: Duration = Duration::from_millis(250);
    /// Non-progress lines kept for the error message.
    const TAIL_LINES: usize = 40;

    fn new() -> Self {
        Self {
            last: None,
            last_emit: Instant::now(),
            tail: Vec::new(),
        }
    }

    fn feed(&mut self, segment: &str, on_progress: &mut dyn FnMut(CloneProgress)) {
        let segment = segment.trim();
        if segment.is_empty() {
            return;
        }
        match parse_git_progress(segment) {
            Some(progress) => {
                // Git redraws the same line many times a second. Forward every
                // change of phase or percentage, and otherwise only often
                // enough for the byte count and speed to feel live. On a large
                // repository one percent can take a minute, so percentage
                // changes alone would leave the bar looking frozen.
                let changed = self.last.as_ref().is_none_or(|(phase, percent)| {
                    *phase != progress.phase || *percent != progress.percent
                });
                if changed || self.last_emit.elapsed() >= Self::INTERVAL {
                    self.last = Some((progress.phase.clone(), progress.percent));
                    self.last_emit = Instant::now();
                    on_progress(progress);
                }
            }
            None => {
                // Progress lines never land here, so a failure after a long
                // download is not reported as thousands of percentages.
                self.tail.push(segment.to_string());
                if self.tail.len() > Self::TAIL_LINES {
                    self.tail.remove(0);
                }
            }
        }
    }

    fn tail(&self) -> String {
        self.tail.join("\n")
    }
}

enum CloneAttempt {
    Succeeded {
        auth_notice: Option<String>,
    },
    Failed {
        stderr: String,
        token_rejected: bool,
    },
    Cancelled,
    Io(String),
}

/// Runs one `git clone`, streaming progress to `on_progress` as it arrives.
///
/// The process is registered with `job` for its whole life, so the UI can
/// cancel it and the app can stop it on exit.
fn run_clone(
    mut cmd: Command,
    job: &CloneJob,
    on_progress: &mut dyn FnMut(CloneProgress),
) -> std::io::Result<(std::process::ExitStatus, String)> {
    let mut child = cmd.spawn()?;
    let mut stderr = child
        .stderr
        .take()
        .expect("build_clone_command pipes stderr");
    job.attach(child);

    let mut forwarder = ProgressForwarder::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        // Git separates progress redraws with `\r` and finished lines with `\n`.
        for &byte in &buffer[..read] {
            if byte == b'\r' || byte == b'\n' {
                forwarder.feed(&String::from_utf8_lossy(&pending), on_progress);
                pending.clear();
            } else {
                pending.push(byte);
            }
        }
    }
    if !pending.is_empty() {
        forwarder.feed(&String::from_utf8_lossy(&pending), on_progress);
    }

    let status = job.wait()?;
    Ok((status, forwarder.tail()))
}

/// Runs `git clone`, and if GitHub refuses the supplied token, retries once
/// without it.
///
/// A token can therefore only ever help: public repositories clone even when
/// the saved token has expired or been revoked, which is exactly the case that
/// used to fail with `remote: invalid credentials`.
fn clone_with_fallback(
    url: &str,
    target: &Path,
    token: Option<&str>,
    depth: CloneDepth,
    job: &CloneJob,
    on_progress: &mut dyn FnMut(CloneProgress),
) -> CloneAttempt {
    let first = run_clone(
        build_clone_command(url, target, token, depth),
        job,
        on_progress,
    );
    if job.is_cancelled() {
        remove_partial_clone(target);
        return CloneAttempt::Cancelled;
    }
    let (status, stderr) = match first {
        Ok(result) => result,
        Err(error) => return CloneAttempt::Io(error.to_string()),
    };
    if status.success() {
        return CloneAttempt::Succeeded { auth_notice: None };
    }
    if token.is_none() || !is_auth_failure(&stderr) {
        return CloneAttempt::Failed {
            stderr,
            token_rejected: false,
        };
    }

    // git removes a directory it created when it fails, but make sure before
    // retrying: git refuses to clone into a non-empty directory.
    remove_partial_clone(target);
    if job.is_cancelled() {
        return CloneAttempt::Cancelled;
    }
    on_progress(CloneProgress {
        phase: "Retrying without your GitHub token".to_string(),
        percent: 0,
        current: 0,
        total: 0,
        transferred: None,
        speed: None,
    });

    let second = run_clone(
        build_clone_command(url, target, None, depth),
        job,
        on_progress,
    );
    if job.is_cancelled() {
        remove_partial_clone(target);
        return CloneAttempt::Cancelled;
    }
    match second {
        Ok((status, _)) if status.success() => CloneAttempt::Succeeded {
            auth_notice: Some(
                "Cloned without your saved GitHub token, because GitHub rejected it. Public \
                 repositories are unaffected, but private ones will fail until you reconnect \
                 GitHub in Settings."
                    .to_string(),
            ),
        },
        Ok((_, stderr)) => CloneAttempt::Failed {
            stderr,
            token_rejected: true,
        },
        Err(error) => CloneAttempt::Io(error.to_string()),
    }
}

/// Removes a partial clone. `target` is only ever the directory the command
/// verified did not exist before the clone began, so this deletes nothing but
/// what the clone itself wrote.
fn remove_partial_clone(target: &Path) {
    if target.exists() {
        let _ = std::fs::remove_dir_all(target);
    }
}

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

/// `open_project`'s response, plus what happened to authentication on the way.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneRepositoryResponse {
    #[serde(flatten)]
    pub opened: OpenProjectResponse,
    /// Set when the clone only succeeded after dropping a rejected GitHub
    /// token, so the user learns their token needs replacing before a private
    /// repository fails on it.
    pub auth_notice: Option<String>,
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
    /// Download complete history. Off by default: only the latest snapshot,
    /// which for a large repository is a small fraction of the download.
    #[serde(default)]
    pub full_history: bool,
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

        // Scoped to github.com by git itself, so pushing to a non-GitHub remote
        // never sends the GitHub token anywhere.
        if let Some(t) = &token {
            apply_github_auth(&mut cmd, t);
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
    app: AppHandle,
    state: State<'_, AppState>,
    request: CloneRepositoryRequest,
) -> AppResult<CloneRepositoryResponse> {
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

    // The GitHub token is only ever offered to https://github.com. Custom URLs
    // on other hosts, plain http://, and SSH URLs never see it.
    let is_github = is_github_https_url(&url);
    let token = if is_github {
        state.keychain.read(GITHUB_PROVIDER).unwrap_or(None)
    } else {
        None
    };

    let depth = if request.full_history {
        CloneDepth::Full
    } else {
        CloneDepth::Shallow
    };

    // One clone at a time: two large downloads split the connection and both
    // crawl, which is how one report of a "hung" clone came about.
    let job = state.begin_clone()?;
    let (url_clone, target_clone) = (url.clone(), target_path.clone());
    let attempt = tauri::async_runtime::spawn_blocking(move || {
        let mut forward = |progress: CloneProgress| {
            let _ = app.emit(CLONE_PROGRESS_EVENT, progress);
        };
        clone_with_fallback(
            &url_clone,
            &target_clone,
            token.as_deref(),
            depth,
            &job,
            &mut forward,
        )
    })
    .await;
    state.end_clone();
    let attempt = attempt.map_err(|e| AppError::internal(format!("Clone task failed: {e}")))?;

    let auth_notice = match attempt {
        CloneAttempt::Succeeded { auth_notice } => auth_notice,
        CloneAttempt::Cancelled => {
            return Err(AppError::new("clone_cancelled", "Clone stopped.")
                .with_recovery("The partial download was removed. Nothing was added to LeanAI."));
        }
        CloneAttempt::Failed {
            stderr,
            token_rejected,
        } => {
            let recovery = if stderr.to_ascii_lowercase().contains("operation too slow") {
                "The download stalled: less than 1 KB/s arrived for a minute, so LeanAI \
                 stopped it. Check your connection and try again. With Full history off, far \
                 less has to be downloaded."
            } else if token_rejected {
                "Your saved GitHub token was rejected, and the repository is not publicly \
                 readable either. Reconnect GitHub in Settings with a token that can read this \
                 repository, or check the URL."
            } else if is_github {
                "Check the URL. A private repository needs a GitHub token with read access \
                 (Settings \u{2192} Git); a public one needs nothing."
            } else {
                "Check the URL and your access. LeanAI only sends your GitHub token to \
                 github.com, so other hosts need SSH or their own credentials."
            };
            let message = if stderr.trim().is_empty() {
                "git clone failed without printing a reason.".to_string()
            } else {
                stderr
            };
            return Err(AppError::new("git_clone_failed", message).with_recovery(recovery));
        }
        CloneAttempt::Io(message) => {
            return Err(AppError::internal(format!(
                "Failed to run git clone: {message}"
            )));
        }
    };

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

    Ok(CloneRepositoryResponse {
        opened: OpenProjectResponse {
            project: record,
            git: gitinfo::git_state(&root),
            has_ai_ignore: root.join(aiignore::FILE_NAME).exists(),
        },
        auth_notice,
    })
}

/// Stops the clone in progress, if any. Returns false when nothing was running.
#[tauri::command]
pub async fn cancel_clone(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.cancel_clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_https_github_com_may_receive_the_token() {
        for url in [
            "https://github.com/calcom/cal.diy",
            "https://github.com/owner/repo.git",
            "https://GitHub.com/owner/repo",
            "https://user@github.com/owner/repo",
            "https://github.com:443/owner/repo",
        ] {
            assert!(is_github_https_url(url), "{url} should be GitHub");
        }

        for url in [
            // Plaintext: the token would travel unencrypted.
            "http://github.com/owner/repo",
            // SSH uses keys, never the token.
            "git@github.com:owner/repo.git",
            // Other hosts must never see a GitHub token.
            "https://gitlab.com/owner/repo",
            "https://bitbucket.org/owner/repo",
            // Look-alikes and userinfo tricks.
            "https://github.com.evil.example/owner/repo",
            "https://evil.example/github.com/owner/repo",
            "https://evil.example?next=github.com",
            "https://github.com@evil.example/owner/repo",
            // Not a git host.
            "https://api.github.com/repos/owner/repo",
            "",
        ] {
            assert!(
                !is_github_https_url(url),
                "{url} must not receive the token"
            );
        }
    }

    #[test]
    fn base64_matches_the_rfc_4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected, "{input:?}");
        }
    }

    #[test]
    fn git_auth_uses_basic_not_bearer() {
        let header = github_basic_auth_header("ghp_example");
        assert!(header.starts_with("Authorization: Basic "), "{header}");
        assert!(!header.contains("Bearer"));
        assert_eq!(
            header,
            format!(
                "Authorization: Basic {}",
                base64_encode(b"x-access-token:ghp_example")
            )
        );
    }

    /// The regression this change exists for: the token used to be a `-c`
    /// argument, readable through `ps` by any local process.
    #[test]
    fn the_token_never_appears_in_the_process_arguments() {
        let secret = "ghp_mustNotAppearInArgv";
        let mut cmd = Command::new("git");
        apply_github_auth(&mut cmd, secret);
        cmd.args(["clone", "https://github.com/owner/repo"]);

        for arg in cmd.get_args() {
            assert!(
                !arg.to_string_lossy().contains(secret),
                "token leaked into argv: {arg:?}"
            );
        }

        let envs: std::collections::HashMap<_, _> = cmd
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect();
        assert_eq!(envs.get("GIT_CONFIG_COUNT").map(String::as_str), Some("1"));
        // Scoped by git to github.com, so no other host can receive it.
        assert_eq!(
            envs.get("GIT_CONFIG_KEY_0").map(String::as_str),
            Some("http.https://github.com/.extraheader")
        );
        let value = envs.get("GIT_CONFIG_VALUE_0").expect("header is set");
        assert!(value.starts_with("Authorization: Basic "));
        assert!(
            !value.contains(secret),
            "the raw token must be encoded, not verbatim"
        );
    }

    #[test]
    fn auth_failures_are_recognised_but_missing_repositories_are_not() {
        // The exact output from the failing clone.
        let observed = "Cloning into '/Users/me/Developer/untitled folder/cal.diy'...\n\
                        remote: invalid credentials\n\
                        fatal: Authentication failed for 'https://github.com/calcom/cal.diy/'";
        assert!(is_auth_failure(observed));
        assert!(is_auth_failure(
            "fatal: could not read Username for 'https://github.com': terminal prompts disabled"
        ));
        assert!(is_auth_failure("The requested URL returned error: 403"));

        assert!(!is_auth_failure("remote: Repository not found."));
        assert!(!is_auth_failure(
            "fatal: destination path 'repo' already exists and is not an empty directory."
        ));
    }

    fn clone_args(depth: CloneDepth) -> Vec<String> {
        build_clone_command("https://github.com/o/r", Path::new("/tmp/t"), None, depth)
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn clone_arguments_cannot_be_read_as_options() {
        let hostile = "--upload-pack=touch /tmp/pwned";
        let args: Vec<String> =
            build_clone_command(hostile, Path::new("/tmp/target"), None, CloneDepth::Shallow)
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
        let separator = args
            .iter()
            .position(|arg| arg == "--")
            .expect("`--` is present");
        assert_eq!(
            args[separator + 1],
            hostile,
            "the URL comes straight after `--`"
        );
        assert_eq!(args[separator + 2], "/tmp/target");
        assert_eq!(args.len(), separator + 3, "nothing follows the target");
    }

    #[test]
    fn clones_are_shallow_unless_full_history_is_requested() {
        let shallow = clone_args(CloneDepth::Shallow);
        let at = shallow
            .iter()
            .position(|arg| arg == "--depth")
            .expect("a shallow clone passes --depth");
        assert_eq!(shallow[at + 1], "1");
        assert!(!clone_args(CloneDepth::Full)
            .iter()
            .any(|arg| arg == "--depth"));
    }

    #[test]
    fn clones_report_progress_and_give_up_on_a_stalled_connection() {
        let args = clone_args(CloneDepth::Full);
        assert!(
            args.iter().any(|arg| arg == "--progress"),
            "without it git prints nothing when not on a terminal"
        );
        let clone_at = args.iter().position(|arg| arg == "clone").unwrap();
        let limit_at = args
            .iter()
            .position(|arg| arg == "http.lowSpeedLimit=1000")
            .expect("stall limit is set");
        let window_at = args
            .iter()
            .position(|arg| arg == "http.lowSpeedTime=60")
            .expect("stall window is set");
        assert!(
            limit_at < clone_at && window_at < clone_at,
            "config must precede the subcommand"
        );
    }

    #[test]
    fn git_messages_are_forced_to_english_for_parsing() {
        let cmd = build_clone_command(
            "https://github.com/o/r",
            Path::new("/tmp/t"),
            None,
            CloneDepth::Shallow,
        );
        let lc_all = cmd
            .get_envs()
            .find(|(key, _)| *key == "LC_ALL")
            .and_then(|(_, value)| value);
        assert_eq!(lc_all, Some(std::ffi::OsStr::new("C")));
    }

    #[test]
    fn git_progress_lines_are_parsed() {
        assert_eq!(
            parse_git_progress("Receiving objects:  23% (2345/10234), 45.67 MiB | 2.31 MiB/s"),
            Some(CloneProgress {
                phase: "Receiving objects".to_string(),
                percent: 23,
                current: 2345,
                total: 10234,
                transferred: Some("45.67 MiB".to_string()),
                speed: Some("2.31 MiB/s".to_string()),
            })
        );

        let finished = parse_git_progress(
            "Receiving objects: 100% (10234/10234), 1.10 GiB | 2.31 MiB/s, done.",
        )
        .unwrap();
        assert_eq!(finished.percent, 100);
        assert_eq!(finished.transferred.as_deref(), Some("1.10 GiB"));
        assert_eq!(finished.speed.as_deref(), Some("2.31 MiB/s"));

        let remote = parse_git_progress("remote: Compressing objects:  45% (50/110)").unwrap();
        assert_eq!(remote.phase, "Compressing objects");
        assert_eq!((remote.current, remote.total), (50, 110));
        assert_eq!(remote.transferred, None);

        let deltas = parse_git_progress("Resolving deltas: 100% (890/890), done.").unwrap();
        assert_eq!(deltas.phase, "Resolving deltas");
        assert_eq!((deltas.transferred, deltas.speed), (None, None));
    }

    #[test]
    fn non_progress_output_is_not_mistaken_for_progress() {
        for line in [
            "Cloning into 'cal.diy'...",
            "remote: Enumerating objects: 12345, done.",
            "fatal: Authentication failed for 'https://github.com/calcom/cal.diy/'",
            "error: RPC failed; curl 28 Operation too slow. Less than 1000 bytes/sec transferred the last 60 seconds",
            "",
        ] {
            assert_eq!(parse_git_progress(line), None, "{line:?}");
        }
    }

    #[test]
    fn errors_are_kept_while_progress_is_forwarded() {
        let mut forwarder = ProgressForwarder::new();
        let mut percents = Vec::new();
        for segment in [
            "Cloning into 'x'...",
            "Receiving objects:  10% (1/10)",
            "Receiving objects:  20% (2/10)",
            "fatal: early EOF",
        ] {
            forwarder.feed(segment, &mut |progress| percents.push(progress.percent));
        }
        assert_eq!(
            percents,
            [10, 20],
            "every change of percentage is forwarded"
        );
        assert_eq!(
            forwarder.tail(),
            "Cloning into 'x'...\nfatal: early EOF",
            "progress never reaches the error text"
        );
    }

    /// Cancelling must reach everything git started, the way a real clone
    /// runs `git-remote-https` and `index-pack` beneath it.
    #[cfg(unix)]
    #[test]
    fn cancelling_stops_the_process_and_its_children() {
        use std::os::unix::process::CommandExt;

        let job = CloneJob::default();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30 & wait"]).process_group(0);
        job.attach(cmd.spawn().unwrap());

        let started = Instant::now();
        assert!(job.cancel(), "a running process is cancelled");
        let status = job.wait().unwrap();
        assert!(!status.success());
        assert!(job.is_cancelled());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "stopped, not waited out"
        );
    }

    #[test]
    fn an_anonymous_clone_carries_no_credentials() {
        let cmd = build_clone_command(
            "https://github.com/owner/repo",
            Path::new("/tmp/t"),
            None,
            CloneDepth::Shallow,
        );
        assert!(cmd
            .get_envs()
            .all(|(key, _)| !key.to_string_lossy().starts_with("GIT_CONFIG_")));
    }

    /// End to end, the reported failure: a token GitHub refuses, on a public
    /// repository. Network-dependent, so run explicitly with `--ignored`.
    ///
    /// Global and system git config are disabled for the child process so the
    /// test never consults the developer's own stored credentials.
    #[test]
    #[ignore = "needs network access to github.com"]
    fn a_rejected_token_falls_back_to_an_anonymous_shallow_clone() {
        std::env::set_var("GIT_CONFIG_GLOBAL", "/dev/null");
        std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("hello-world");
        let job = CloneJob::default();
        let mut events = Vec::new();
        let attempt = clone_with_fallback(
            "https://github.com/octocat/Hello-World",
            &target,
            Some("ghp_deliberatelyInvalidToken000000000000"),
            CloneDepth::Shallow,
            &job,
            &mut |progress| events.push(progress),
        );

        match attempt {
            CloneAttempt::Succeeded { auth_notice } => {
                assert!(target.join(".git").is_dir(), "the repository was cloned");
                assert!(
                    target.join(".git/shallow").is_file(),
                    "only the latest snapshot was downloaded"
                );
                let notice = auth_notice.expect("the user is told their token was rejected");
                assert!(notice.contains("rejected"), "{notice}");
                assert!(events.iter().all(|event| event.percent <= 100));
            }
            CloneAttempt::Failed { stderr, .. } => {
                panic!("fallback did not rescue the clone: {stderr}")
            }
            CloneAttempt::Cancelled => panic!("nothing cancelled this clone"),
            CloneAttempt::Io(message) => panic!("git could not run: {message}"),
        }
    }
}
