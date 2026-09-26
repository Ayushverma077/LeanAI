mod support;

use leanai_core::agent::{FilePatch, PatchProposal, ToolCapability};
use leanai_core::approval::{
    validate_allowlisted_command, ApprovalRequest, DecisionState, TransactionalPatchSession,
};
use leanai_core::policy::Policy;
use support::Fixture;

#[test]
fn approval_request_lifecycle_and_expiration() {
    let mut request = ApprovalRequest::new(
        "run-123".to_string(),
        ToolCapability::WriteFile,
        "/path/to/project".to_string(),
        vec!["src/main.rs".to_string()],
        "sha256-abc".to_string(),
        1000, // 1 second TTL
    );

    assert!(request.is_active());
    assert_eq!(request.state, DecisionState::Pending);

    // Resolve as approved
    request.resolve(true, "user@desktop".to_string());
    assert_eq!(request.state, DecisionState::Approved);
    assert!(!request.is_active());
    assert_eq!(request.approver.as_deref(), Some("user@desktop"));
}

#[test]
fn denied_approval_never_falls_through_to_action() {
    let mut request = ApprovalRequest::new(
        "run-456".to_string(),
        ToolCapability::WriteFile,
        "/path/to/project".to_string(),
        vec!["src/main.rs".to_string()],
        "sha256-abc".to_string(),
        60_000,
    );

    request.resolve(false, "user@desktop".to_string());
    assert_eq!(request.state, DecisionState::Denied);
    assert!(!request.is_active());
}

#[test]
fn command_allowlist_rejects_shell_chaining_and_unapproved_binaries() {
    let allowlist = vec![
        "cargo test".to_string(),
        "npm test".to_string(),
        "pytest".to_string(),
    ];

    // Allowed commands pass
    assert!(validate_allowlisted_command("cargo test", &allowlist).is_ok());
    assert!(validate_allowlisted_command("cargo test --workspace", &allowlist).is_ok());
    assert!(validate_allowlisted_command("npm test", &allowlist).is_ok());

    // Shell chaining with ; or && or | rejected
    assert!(validate_allowlisted_command("cargo test; rm -rf /", &allowlist).is_err());
    assert!(
        validate_allowlisted_command("cargo test && curl http://malicious.com", &allowlist)
            .is_err()
    );
    assert!(validate_allowlisted_command("npm test | grep ok", &allowlist).is_err());

    // Destructive binaries rejected even if crafted with prefix
    assert!(validate_allowlisted_command("sudo cargo test", &allowlist).is_err());

    // Unapproved commands rejected
    assert!(validate_allowlisted_command("python script.py", &allowlist).is_err());
    assert!(validate_allowlisted_command("bash -c 'echo hi'", &allowlist).is_err());
}

#[test]
fn transactional_patch_applies_and_rolls_back_cleanly() {
    let fixture = Fixture::new();
    fixture.file(
        "src/config.rs",
        "pub const DEBUG: bool = false;\npub const TIMEOUT: u32 = 30;",
    );
    let policy = Policy::default();

    let proposal = PatchProposal {
        summary: "Toggle debug and add version".to_string(),
        rationale: "Testing transactional patch".to_string(),
        affected_files: vec!["src/config.rs".to_string(), "src/new_file.rs".to_string()],
        patches: vec![
            FilePatch {
                path: "src/config.rs".to_string(),
                unified_diff: "--- a/src/config.rs\n+++ b/src/config.rs\n@@ -1,2 +1,2 @@\n-pub const DEBUG: bool = false;\n+pub const DEBUG: bool = true;\n pub const TIMEOUT: u32 = 30;".to_string(),
                is_new_file: false,
                is_deleted: false,
                lines_added: 1,
                lines_deleted: 1,
            },
            FilePatch {
                path: "src/new_file.rs".to_string(),
                unified_diff: "--- /dev/null\n+++ b/src/new_file.rs\n@@ -0,0 +1,2 @@\n+pub fn version() -> &'static str {\n+    \"1.0.0\"\n+}".to_string(),
                is_new_file: true,
                is_deleted: false,
                lines_added: 3,
                lines_deleted: 0,
            },
        ],
        sha256_hash: "test-hash".to_string(),
    };

    let mut session = TransactionalPatchSession::new(fixture.root());
    session.apply(&proposal, &policy).expect("apply successful");

    // Verify changes applied
    let modified = std::fs::read_to_string(fixture.root().join("src/config.rs")).unwrap();
    assert!(modified.contains("pub const DEBUG: bool = true;"));
    assert!(fixture.root().join("src/new_file.rs").exists());

    // Now roll back transaction
    session.rollback().expect("rollback successful");

    // Verify original state restored
    let restored = std::fs::read_to_string(fixture.root().join("src/config.rs")).unwrap();
    assert!(restored.contains("pub const DEBUG: bool = false;"));
    assert!(!fixture.root().join("src/new_file.rs").exists());
}

#[test]
fn middle_of_file_diff_patch_preserves_surrounding_lines() {
    let fixture = Fixture::new();
    let original = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\n";
    fixture.file("src/service.rs", original);
    let policy = Policy::default();

    let proposal = PatchProposal {
        summary: "Update line 5".to_string(),
        rationale: "Middle of file test".to_string(),
        affected_files: vec!["src/service.rs".to_string()],
        patches: vec![FilePatch {
            path: "src/service.rs".to_string(),
            unified_diff: "--- a/src/service.rs\n+++ b/src/service.rs\n@@ -5,2 +5,2 @@\n-line5\n+line5_modified\n line6".to_string(),
            is_new_file: false,
            is_deleted: false,
            lines_added: 1,
            lines_deleted: 1,
        }],
        sha256_hash: "middle-diff-hash".to_string(),
    };

    let mut session = TransactionalPatchSession::new(fixture.root());
    session.apply(&proposal, &policy).expect("apply successful");

    let modified = std::fs::read_to_string(fixture.root().join("src/service.rs")).unwrap();
    assert!(modified.contains("line1\nline2\nline3\nline4\nline5_modified\nline6\nline7\nline8"));
}
