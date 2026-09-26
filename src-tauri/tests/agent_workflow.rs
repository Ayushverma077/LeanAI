//! Integration tests for agent runs, approvals, allowlists, and episodic memory persistence.

use leanai_core::agent::ToolCapability;
use leanai_core::approval::{ApprovalRequest, DecisionState};
use leanai_core::retrieval::EpisodicMemoryEntry;
use leanai_desktop_lib::db::migrations::{self, LATEST_VERSION};
use leanai_desktop_lib::db::repositories::{
    append_run_event, get_approval, get_command_allowlist, get_run, list_project_memory,
    list_run_events, list_runs, resolve_approval_record, save_approval, save_project_memory,
    set_command_allowlist, upsert_project, upsert_run, RunRecord,
};
use rusqlite::Connection;

fn migrated() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();
    connection
}

#[test]
fn schema_v4_applies_cleanly() {
    let connection = migrated();
    assert_eq!(LATEST_VERSION, 4);

    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert!(tables.contains(&"project_command_allowlists".to_string()));
    assert!(tables.contains(&"project_memory".to_string()));
    assert!(tables.contains(&"runs".to_string()));
    assert!(tables.contains(&"run_events".to_string()));
    assert!(tables.contains(&"approvals".to_string()));
}

#[test]
fn agent_run_and_event_ledger_roundtrip() {
    let connection = migrated();
    let project = upsert_project(&connection, "fp-1", "/test/project", "test-project", 1).unwrap();

    let run_id = "run-test-001";
    let run = RunRecord {
        id: run_id.to_string(),
        project_id: project.id.clone(),
        task: "Implement safe user authentication".to_string(),
        mode: "agent_guided".to_string(),
        context_manifest: serde_json::json!({ "files": ["src/auth.rs"] }),
        policy: serde_json::json!({ "policyVersion": 1 }),
        status: "awaiting_approval".to_string(),
        budget: Some(serde_json::json!({ "maxTokens": 128000, "maxCostUsd": 1.50 })),
        started_at_ms: 1000,
        ended_at_ms: None,
        validation_state: Some(serde_json::json!({ "isValid": true })),
    };

    upsert_run(&connection, &run).unwrap();

    // Verify run retrieval
    let retrieved = get_run(&connection, run_id).unwrap().expect("run found");
    assert_eq!(retrieved.task, "Implement safe user authentication");
    assert_eq!(retrieved.status, "awaiting_approval");

    // Append run events
    append_run_event(
        &connection,
        run_id,
        1,
        "planner_plan",
        &serde_json::json!({ "subtasks": 3 }),
    )
    .unwrap();
    append_run_event(
        &connection,
        run_id,
        2,
        "coder_diff",
        &serde_json::json!({ "linesAdded": 15 }),
    )
    .unwrap();

    let events = list_run_events(&connection, run_id).unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["eventType"], "planner_plan");
    assert_eq!(events[1]["eventType"], "coder_diff");

    // List runs for project
    let runs = list_runs(&connection, &project.id, 10).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].id, run_id);
}

#[test]
fn approval_persistence_and_resolution() {
    let connection = migrated();
    let project =
        upsert_project(&connection, "fp-2", "/test/project2", "test-project-2", 1).unwrap();

    let run_id = "run-test-002";
    let run = RunRecord {
        id: run_id.to_string(),
        project_id: project.id.clone(),
        task: "Update dependencies".to_string(),
        mode: "agent_guided".to_string(),
        context_manifest: serde_json::json!({}),
        policy: serde_json::json!({}),
        status: "running".to_string(),
        budget: None,
        started_at_ms: 2000,
        ended_at_ms: None,
        validation_state: None,
    };
    upsert_run(&connection, &run).unwrap();

    let approval_req = ApprovalRequest::new(
        run_id.to_string(),
        ToolCapability::WriteFile,
        "/test/project2".to_string(),
        vec!["Cargo.toml".to_string()],
        "sha256-hash-xyz".to_string(),
        60_000,
    );
    save_approval(&connection, &approval_req).unwrap();

    // Check retrieval
    let fetched = get_approval(&connection, &approval_req.id)
        .unwrap()
        .expect("approval found");
    assert_eq!(fetched.state, DecisionState::Pending);
    assert_eq!(fetched.affected_paths, vec!["Cargo.toml".to_string()]);

    // Resolve as approved
    resolve_approval_record(
        &connection,
        &approval_req.id,
        "approved",
        "admin@desktop",
        2500,
    )
    .unwrap();
    let resolved = get_approval(&connection, &approval_req.id)
        .unwrap()
        .expect("resolved found");
    assert_eq!(resolved.state, DecisionState::Approved);
    assert_eq!(resolved.approver.as_deref(), Some("admin@desktop"));
}

#[test]
fn command_allowlist_and_episodic_memory() {
    let connection = migrated();
    let project =
        upsert_project(&connection, "fp-3", "/test/project3", "test-project-3", 1).unwrap();

    // Default allowlist when empty
    let defaults = get_command_allowlist(&connection, &project.id).unwrap();
    assert!(defaults.contains(&"cargo test".to_string()));

    // Custom allowlist
    let mut custom = vec!["cargo test".to_string(), "cargo clippy".to_string()];
    set_command_allowlist(&connection, &project.id, &custom).unwrap();
    let fetched = get_command_allowlist(&connection, &project.id).unwrap();
    custom.sort();
    assert_eq!(fetched, custom);

    // Episodic memory
    let entry = EpisodicMemoryEntry {
        id: "mem-1".to_string(),
        task_summary: "Refactored tokenizer to BPE".to_string(),
        relevant_paths: vec!["src/tokenizer.rs".to_string()],
        key_findings: "Reduced token estimate divergence by 4%".to_string(),
        created_at_ms: leanai_core::project::now_ms(),
        expires_at_ms: leanai_core::project::now_ms() + 10_000_000,
    };
    save_project_memory(&connection, &entry, &project.id).unwrap();

    let memories = list_project_memory(&connection, &project.id, 10).unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].task_summary, "Refactored tokenizer to BPE");
}

#[test]
fn tester_step_event_ledger_and_allowlist() {
    let connection = migrated();
    let project =
        upsert_project(&connection, "fp-4", "/test/project4", "test-project-4", 1).unwrap();

    let run_id = "run-test-004";
    let run = RunRecord {
        id: run_id.to_string(),
        project_id: project.id.clone(),
        task: "Validate test runner flow".to_string(),
        mode: "agent_guided".to_string(),
        context_manifest: serde_json::json!({}),
        policy: serde_json::json!({}),
        status: "running".to_string(),
        budget: None,
        started_at_ms: 3000,
        ended_at_ms: None,
        validation_state: None,
    };
    upsert_run(&connection, &run).unwrap();

    // Log a tester step event into the immutable ledger
    let tester_artifact = serde_json::json!({
        "command": "cargo test",
        "passed": true,
        "exit_code": 0,
        "stdout": "test result: ok",
        "stderr": "",
        "duration_ms": 1240,
        "timestamp_ms": 3100,
        "summary": "Test command 'cargo test' PASSED (exit code 0)."
    });

    append_run_event(&connection, run_id, 3, "tester_step", &tester_artifact).unwrap();

    let events = list_run_events(&connection, run_id).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["eventType"], "tester_step");
    assert_eq!(events[0]["payload"]["passed"], true);
    assert_eq!(events[0]["payload"]["command"], "cargo test");
}
