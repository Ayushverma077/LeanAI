//! End-to-end prompt runs against a scripted, OpenAI-compatible local model.
//!
//! The "local model" is the managed sidecar in its test mode, pointed at an
//! in-process HTTP server that replays scripted replies and records every
//! request body, so each test can assert exactly what context LeanAI sent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use leanai_core::apply_gate::ApplyLevel;
use leanai_core::catalog::ModelTier;
use leanai_core::gguf::create_synthetic_gguf;
use leanai_core::walker::{scan, CancelToken, ScanOptions};
use leanai_desktop_lib::app_state::{AppState, ProjectSession};
use leanai_desktop_lib::commands::models::{store_self_hosted, SelfHostedModel};
use leanai_desktop_lib::commands::prompt::{run_prompt_with, RunPromptRequest};
use leanai_desktop_lib::db::repositories::upsert_project;
use leanai_desktop_lib::sidecar_manager::SidecarStatus;

const LOGIN: &str =
    "export function LoginButton() {\n  return <button className=\"primary\">Login</button>;\n}\n";
const UNRELATED_MARKER: &str = "UNRELATED_SOURCE_MARKER_91c2";

struct Harness {
    state: AppState,
    project: tempfile::TempDir,
    _data: tempfile::TempDir,
    _model: tempfile::NamedTempFile,
    requests: Arc<Mutex<Vec<String>>>,
    server: Option<JoinHandle<()>>,
}

/// Opens a small project and a fake local model that answers with `replies`.
fn harness(replies: Vec<String>) -> Harness {
    harness_with(replies, true).0
}

/// Like [`harness`]; `sidecar` controls whether the managed local model is
/// marked running. Returns the fake server's port.
fn harness_with(replies: Vec<String>, sidecar: bool) -> (Harness, u16) {
    // Never touch the developer's real keychain: with a real Anthropic or
    // OpenAI key present, a test could otherwise route to a paid provider.
    std::env::set_var("LEANAI_MOCK_KEYCHAIN", "1");

    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    std::fs::create_dir_all(root.join("src/components")).unwrap();
    std::fs::write(root.join("src/components/LoginButton.tsx"), LOGIN).unwrap();
    std::fs::write(
        root.join("src/billing.ts"),
        format!("// {UNRELATED_MARKER}\nexport const plan = 'pro';\n"),
    )
    .unwrap();
    std::fs::write(root.join(".env"), "API_TOKEN=sk-live-should-never-leave\n").unwrap();
    std::fs::write(
        root.join("package.json"),
        "{\"name\":\"demo\",\"dependencies\":{\"react\":\"19\"}}\n",
    )
    .unwrap();

    let data = tempfile::tempdir().unwrap();
    let state = AppState::initialize(data.path().to_path_buf()).unwrap();
    let canonical = leanai_core::project::canonical_root(root).unwrap();
    let inventory = scan(
        &canonical,
        &ScanOptions::default(),
        &CancelToken::new(),
        |_| {},
    )
    .unwrap();
    let record = state
        .with_db(|conn| {
            upsert_project(
                conn,
                &inventory.project_fingerprint,
                &canonical.to_string_lossy(),
                "demo",
                1,
            )
        })
        .unwrap();
    state.set_session(ProjectSession {
        record,
        root: canonical,
        inventory: Arc::new(inventory),
    });

    // Either mark the local model as running (test hooks) and serve its
    // port, or just serve an ephemeral port for self-hosted models.
    let model = tempfile::NamedTempFile::new().unwrap();
    let listener = if sidecar {
        std::fs::write(model.path(), create_synthetic_gguf(3, 32, 1)).unwrap();
        state
            .sidecar
            .set_binary_override(Some("/usr/bin/true".into()));
        state.sidecar.set_mock_healthy(true);
        // The sidecar picks a free port and releases it; a test running in
        // parallel can take it before this listener binds. Restart the
        // sidecar on a fresh port when that happens.
        let mut attempts = 0;
        loop {
            attempts += 1;
            let port = match state
                .sidecar
                .start("local-test", "Local test", model.path(), None)
                .unwrap()
            {
                SidecarStatus::Ready { port, .. } => port,
                other => panic!("sidecar not ready: {other:?}"),
            };
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(listener) => break listener,
                Err(_) if attempts < 5 => {
                    state.sidecar.stop().unwrap();
                }
                Err(error) => panic!("no free port for the fake model: {error}"),
            }
        }
    } else {
        // Keep the listener open: releasing and re-binding a port races with
        // other tests running in parallel.
        TcpListener::bind("127.0.0.1:0").unwrap()
    };
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let log = requests.clone();
    let server = std::thread::spawn(move || {
        for reply in replies {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            let mut headers = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
                headers.push_str(&line.to_ascii_lowercase());
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0u8; length];
            reader.read_exact(&mut body).unwrap();
            log.lock()
                .unwrap()
                .push(format!("{headers}\r\n{}", String::from_utf8(body).unwrap()));
            if reply == "!400" {
                let error = r#"{"error":{"message":"Unsupported parameter: response_format"}}"#;
                write!(
                    stream,
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error}",
                    error.len()
                )
                .unwrap();
                continue;
            }
            if reply == "!429" {
                let error = r#"{"error":{"message":"Rate limit reached: tokens per minute"}}"#;
                write!(
                    stream,
                    "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{error}",
                    error.len()
                )
                .unwrap();
                continue;
            }
            let payload = serde_json::json!({
                "choices": [{ "message": { "role": "assistant", "content": reply }, "finish_reason": "stop" }],
                "usage": { "prompt_tokens": 500, "completion_tokens": 40 }
            })
            .to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            )
            .unwrap();
        }
    });

    (
        Harness {
            state,
            project,
            _data: data,
            _model: model,
            requests,
            server: Some(server),
        },
        port,
    )
}

fn request(prompt: &str, auto_apply: bool) -> RunPromptRequest {
    serde_json::from_value(serde_json::json!({ "prompt": prompt, "autoApply": auto_apply }))
        .unwrap()
}

#[tokio::test]
async fn simple_edit_retrieves_only_requested_files_and_applies_safely() {
    let mut h = harness(vec![
        r#"{"action":"search","query":"Login"}"#.to_string(),
        r#"{"action":"read_files","files":[{"path":"src/components/LoginButton.tsx"},{"path":"../outside.txt"},{"path":".env"}]}"#.to_string(),
        r#"{"action":"edit","summary":"Rename the login button","edits":[{"path":"src/components/LoginButton.tsx","find":">Login<","replace":">Sign In<"}]}"#.to_string(),
    ]);

    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", true),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    // Routing: code edit, small budget, minimal initial context.
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["analysis"]["intent"], "CODE_EDIT");
    assert_eq!(json["budget"]["level"], "minimal");
    assert_eq!(json["selection"]["provider"], "local");

    // A one-line label change is low risk, so the gate let it apply.
    let decision = response.apply_decision.as_ref().unwrap();
    assert_eq!(decision.level, ApplyLevel::AutoApply);
    assert!(decision.reasons.is_empty());

    // Applied transactionally, and the project index refreshed.
    assert_eq!(response.status, "applied");
    assert_eq!(
        response.files_changed,
        vec!["src/components/LoginButton.tsx"]
    );
    let updated =
        std::fs::read_to_string(h.project.path().join("src/components/LoginButton.tsx")).unwrap();
    assert_eq!(updated, LOGIN.replace(">Login<", ">Sign In<"));
    assert!(response.context_refreshed);
    assert!(h.project.path().join("PROJECT_CONTEXT.md").exists());

    // Retrieval was model-driven, validated and bounded.
    let refused: Vec<_> = response
        .retrievals
        .iter()
        .filter(|r| r.refused.is_some())
        .map(|r| r.path.as_str())
        .collect();
    assert_eq!(refused, vec!["../outside.txt", ".env"]);
    assert_eq!(response.searches, vec!["Login"]);
    assert!(response.metrics.retrieved_tokens > 0);
    assert_eq!(
        response.metrics.context_tokens_sent,
        response.metrics.initial_context_tokens + response.metrics.retrieved_tokens
    );
    assert_eq!(response.metrics.llm_calls, 3);

    // Exactly what each call carried.
    let sent = h.requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 3);
    assert!(
        sent[0].contains("Project index"),
        "first call carries the index"
    );
    assert!(
        !sent[0].contains("className=\\\"primary\\\""),
        "no source file before the model asks"
    );
    assert!(
        sent[1].contains("src/components/LoginButton.tsx:2"),
        "search results come back with line numbers"
    );
    assert!(
        sent[2].contains("className=\\\"primary\\\""),
        "requested file is supplied after read_files"
    );
    for body in &sent {
        assert!(
            body.contains("\"response_format\"") && body.contains("leanai_action"),
            "agent turns ask for schema-constrained replies"
        );
        assert!(
            !body.contains("sk-live-should-never-leave"),
            "credential files are never sent"
        );
        assert!(
            !body.contains(UNRELATED_MARKER),
            "unrequested files are never sent"
        );
    }
}

#[tokio::test]
async fn non_repository_prompt_sends_no_project_context() {
    let mut h = harness(vec![
        "Recursion is when a function calls itself.".to_string()
    ]);
    let response = run_prompt_with(&h.state, request("Explain recursion.", false))
        .await
        .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(response.status, "completed");
    assert_eq!(
        response.answer.as_deref(),
        Some("Recursion is when a function calls itself.")
    );
    assert_eq!(response.metrics.context_tokens_sent, 0);
    assert_eq!(response.metrics.llm_calls, 1);
    let sent = h.requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 1);
    assert!(!sent[0].contains("Project index"));
    assert!(!sent[0].contains("LoginButton"));
    assert!(
        !sent[0].contains("response_format"),
        "free-text answers are not schema-constrained"
    );
    assert!(
        !h.project.path().join("PROJECT_CONTEXT.md").exists(),
        "no context generated when not needed"
    );
}

#[tokio::test]
async fn repeated_invalid_output_fails_safely_without_writing() {
    let mut h = harness(vec![
        "I think you should change it.".to_string(),
        "Sure! Done.".to_string(),
    ]);
    let error = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", true),
    )
    .await
    .unwrap_err();
    h.server.take().unwrap().join().unwrap();

    // Only one model is available, so there is nothing to escalate to.
    assert_eq!(error.code, "model_insufficient");
    let unchanged =
        std::fs::read_to_string(h.project.path().join("src/components/LoginButton.tsx")).unwrap();
    assert_eq!(unchanged, LOGIN);
}

#[tokio::test]
async fn review_mode_stages_the_change_and_writes_nothing() {
    let mut h = harness(vec![
        r#"{"action":"read_files","files":[{"path":"src/components/LoginButton.tsx"}]}"#.to_string(),
        r#"{"action":"edit","summary":"Rename","edits":[{"path":"src/components/LoginButton.tsx","find":">Login<","replace":">Sign In<"}]}"#.to_string(),
    ]);
    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", false),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(response.status, "awaiting_approval");
    let decision = response.apply_decision.as_ref().unwrap();
    assert_eq!(decision.level, ApplyLevel::Confirm);
    assert!(
        decision.reasons.is_empty(),
        "only the setting held this low-risk change back"
    );
    let approval = response.pending_approval.as_ref().unwrap();
    let proposal = response.patch_proposal.as_ref().unwrap();
    assert_eq!(
        approval.patch_hash,
        leanai_core::llm_protocol::proposal_hash(proposal)
    );
    assert_eq!(
        approval.affected_paths,
        vec!["src/components/LoginButton.tsx"]
    );
    let unchanged =
        std::fs::read_to_string(h.project.path().join("src/components/LoginButton.tsx")).unwrap();
    assert_eq!(unchanged, LOGIN, "nothing is written before approval");
}

fn self_hosted(id: &str, port: u16, model: &str, tier: ModelTier, key: bool) -> SelfHostedModel {
    SelfHostedModel {
        id: id.to_string(),
        display_name: id.to_string(),
        base_url: format!("http://127.0.0.1:{port}"),
        model: model.to_string(),
        tier,
        context_cap: 32_768,
        has_api_key: key,
    }
}

#[tokio::test]
async fn self_hosted_model_on_another_device_handles_reasoning_and_auth() {
    let (mut h, port) = harness_with(
        vec![
            "<think>The user wants {a rename}. I should read the file.</think>\n{\"action\":\"read_files\",\"files\":[{\"path\":\"src/components/LoginButton.tsx\"}]}".to_string(),
            "<think>ok</think>{\"action\":\"edit\",\"summary\":\"Rename\",\"edits\":[{\"path\":\"src/components/LoginButton.tsx\",\"find\":\">Login<\",\"replace\":\">Sign In<\"}]}".to_string(),
        ],
        false,
    );
    store_self_hosted(
        &h.state,
        &[self_hosted(
            "qwen",
            port,
            "qwen3:14b",
            ModelTier::Balanced,
            true,
        )],
    )
    .unwrap();
    h.state
        .keychain
        .store("custom-qwen", "api_key", "lan-secret-123")
        .unwrap();

    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", true),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(response.selection.model_id, "custom/qwen");
    assert_eq!(response.status, "applied");
    let sent = h.requests.lock().unwrap().clone();
    assert!(sent[0].contains("authorization: bearer lan-secret-123"));
    assert!(sent[0].contains("\"model\":\"qwen3:14b\""));
    let updated =
        std::fs::read_to_string(h.project.path().join("src/components/LoginButton.tsx")).unwrap();
    assert!(updated.contains(">Sign In<"));
}

#[tokio::test]
async fn unreachable_self_hosted_server_escalates_to_the_next_model() {
    let (mut h, port) = harness_with(
        vec![
            r#"{"action":"read_files","files":[{"path":"src/components/LoginButton.tsx"}]}"#.to_string(),
            r#"{"action":"edit","summary":"Rename","edits":[{"path":"src/components/LoginButton.tsx","find":">Login<","replace":">Sign In<"}]}"#.to_string(),
        ],
        false,
    );
    // Nothing listens on this port: the device is "switched off".
    let dead = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    store_self_hosted(
        &h.state,
        &[
            self_hosted("gemma", dead, "gemma4", ModelTier::Fast, false),
            self_hosted("qwen", port, "qwen3", ModelTier::Balanced, false),
        ],
    )
    .unwrap();

    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", true),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(
        response.selection.model_id, "custom/gemma",
        "cheapest sufficient first"
    );
    assert_eq!(response.escalations.len(), 1);
    assert_eq!(response.escalations[0].to_model_id, "custom/qwen");
    assert_eq!(response.final_model_id, "custom/qwen");
    assert_eq!(response.status, "applied");
    assert!(
        !h.requests.lock().unwrap()[0].contains("authorization"),
        "no key configured, none sent"
    );
}

#[tokio::test]
async fn rate_limited_api_is_retried_after_its_retry_after() {
    // An OpenAI-compatible hosted API (e.g. a free tier) that rate-limits once.
    let (mut h, port) = harness_with(
        vec![
            "!429".to_string(),
            "Recursion is a function calling itself.".to_string(),
        ],
        false,
    );
    store_self_hosted(
        &h.state,
        &[self_hosted(
            "groq-trial",
            port,
            "some-model",
            ModelTier::Balanced,
            true,
        )],
    )
    .unwrap();
    h.state
        .keychain
        .store("custom-groq-trial", "api_key", "gsk_test")
        .unwrap();

    let response = run_prompt_with(&h.state, request("Explain recursion.", false))
        .await
        .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(
        response.answer.as_deref(),
        Some("Recursion is a function calling itself.")
    );
    assert!(
        response.escalations.is_empty(),
        "a short rate limit is waited out, not escalated"
    );
    assert_eq!(h.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn gate_holds_back_build_file_changes_even_with_auto_apply_on() {
    let mut h = harness(vec![
        r#"{"action":"read_files","files":[{"path":"package.json"}]}"#.to_string(),
        r#"{"action":"edit","summary":"Rename package","edits":[{"path":"package.json","find":"\"demo\"","replace":"\"demo-app\""}]}"#.to_string(),
    ]);
    let response = run_prompt_with(
        &h.state,
        request("Change the package name to demo-app in package.json.", true),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(response.status, "awaiting_approval");
    let decision = response.apply_decision.as_ref().unwrap();
    assert_eq!(decision.level, ApplyLevel::CarefulReview);
    assert!(decision.reasons[0].text.contains("package.json"));
    assert!(response
        .trace
        .iter()
        .any(|s| s.kind == "apply_gate" && s.detail.starts_with("Careful review")));
    let unchanged = std::fs::read_to_string(h.project.path().join("package.json")).unwrap();
    assert!(
        unchanged.contains("\"demo\""),
        "nothing is written before review"
    );
}

#[tokio::test]
async fn server_without_schema_support_falls_back_to_instructions_once() {
    let mut h = harness(vec![
        "!400".to_string(),
        r#"{"action":"read_files","files":[{"path":"src/components/LoginButton.tsx"}]}"#.to_string(),
        r#"{"action":"edit","summary":"Rename","edits":[{"path":"src/components/LoginButton.tsx","find":">Login<","replace":">Sign In<"}]}"#.to_string(),
    ]);
    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", false),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    assert_eq!(response.status, "awaiting_approval");
    let sent = h.requests.lock().unwrap().clone();
    assert_eq!(sent.len(), 3, "one rejected request, then one per turn");
    assert!(sent[0].contains("response_format"));
    assert!(
        !sent[1].contains("response_format") && !sent[2].contains("response_format"),
        "the schema is not re-sent to a server that rejected it"
    );
    assert_eq!(
        response
            .warnings
            .iter()
            .filter(|w| w.contains("does not support schema-constrained replies"))
            .count(),
        1
    );
    assert_eq!(response.metrics.llm_calls, 2);
}

#[tokio::test]
async fn a_project_context_file_leanai_did_not_write_is_never_overwritten() {
    let mut h = harness(vec![
        r#"{"action":"read_files","files":[{"path":"src/components/LoginButton.tsx"}]}"#.to_string(),
        r#"{"action":"edit","summary":"Rename","edits":[{"path":"src/components/LoginButton.tsx","find":">Login<","replace":">Sign In<"}]}"#.to_string(),
    ]);
    let own = "# Our team's project notes\n\nHand-written; please keep.\n";
    std::fs::write(h.project.path().join("PROJECT_CONTEXT.md"), own).unwrap();

    let response = run_prompt_with(
        &h.state,
        request("Change the Login button text to Sign In.", true),
    )
    .await
    .unwrap();
    h.server.take().unwrap().join().unwrap();

    // The run used LeanAI's own map and applied the change...
    assert_eq!(response.status, "applied");
    assert!(h.requests.lock().unwrap()[0].contains("Project index"));
    // ...but the team's file was left exactly as it was.
    assert_eq!(
        std::fs::read_to_string(h.project.path().join("PROJECT_CONTEXT.md")).unwrap(),
        own
    );
}

#[tokio::test]
async fn follow_up_includes_conversation_as_separate_messages() {
    let mut h = harness(vec!["For example, factorial calls itself.".to_string()]);
    let follow_up: RunPromptRequest = serde_json::from_value(serde_json::json!({
        "prompt": "Give me an example.",
        "history": [
            { "role": "user", "content": "Explain recursion." },
            { "role": "assistant", "content": "Recursion is a function calling itself." }
        ]
    }))
    .unwrap();
    let response = run_prompt_with(&h.state, follow_up).await.unwrap();
    h.server.take().unwrap().join().unwrap();
    assert_eq!(
        response.answer.as_deref(),
        Some("For example, factorial calls itself.")
    );
    let sent = h.requests.lock().unwrap();
    let body: serde_json::Value =
        serde_json::from_str(sent[0].split_once("\r\n\r\n").unwrap().1).unwrap();
    let messages = body["messages"].as_array().unwrap();
    let user_messages: Vec<_> = messages.iter().filter(|m| m["role"] == "user").collect();
    assert_eq!(user_messages.len(), 2);
    assert_eq!(user_messages[0]["content"], "Explain recursion.");
    assert_eq!(user_messages[1]["content"], "Give me an example.");
    assert!(messages
        .iter()
        .any(|m| m["role"] == "assistant"
            && m["content"] == "Recursion is a function calling itself."));
}

#[tokio::test]
async fn oversized_history_is_rejected_before_calling_a_model() {
    let mut h = harness(vec![]);
    let request: RunPromptRequest = serde_json::from_value(serde_json::json!({
        "prompt": "Continue.",
        "history": [{ "role": "user", "content": "x".repeat(48_001) }]
    }))
    .unwrap();
    let error = run_prompt_with(&h.state, request).await.unwrap_err();
    assert_eq!(error.code, "history_too_large");
    assert!(h.requests.lock().unwrap().is_empty());
    h.server.take().unwrap().join().unwrap();
}
