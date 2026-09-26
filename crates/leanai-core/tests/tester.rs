mod support;

use leanai_core::agent::{execute_tester_command, parse_command_tokens};
use leanai_core::approval::validate_allowlisted_command;
use support::Fixture;

#[test]
fn parse_command_tokens_handles_quotes_and_whitespace() {
    let tokens = parse_command_tokens("cargo test --workspace -- --nocapture").unwrap();
    assert_eq!(
        tokens,
        vec!["cargo", "test", "--workspace", "--", "--nocapture"]
    );

    let quoted = parse_command_tokens("echo 'hello world' \"second arg\"").unwrap();
    assert_eq!(quoted, vec!["echo", "hello world", "second arg"]);

    let extra_spaces = parse_command_tokens("   npm   test   --   --watch=false   ").unwrap();
    assert_eq!(extra_spaces, vec!["npm", "test", "--", "--watch=false"]);

    // Unclosed quotes error
    assert!(parse_command_tokens("echo \"unclosed").is_err());
}

#[test]
fn validate_allowlist_enforces_token_prefix_and_rejects_evasion() {
    let allowlist = vec!["cargo test".to_string(), "npm test".to_string()];

    // Valid commands matching prefix
    assert!(validate_allowlisted_command("cargo test", &allowlist).is_ok());
    assert!(validate_allowlisted_command("cargo test --test agent", &allowlist).is_ok());
    assert!(validate_allowlisted_command("npm test", &allowlist).is_ok());

    // Non-prefix match rejected (e.g. cargo tester, cargo testevil)
    assert!(validate_allowlisted_command("cargo tester", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo build", &allowlist).is_err());

    // Metacharacter evasion rejected
    assert!(validate_allowlisted_command("cargo test; ls", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test && whoami", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test | cat", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test $(whoami)", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test `whoami`", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test \\; rm -rf", &allowlist).is_err());
    assert!(validate_allowlisted_command("cargo test (echo test)", &allowlist).is_err());
}

#[test]
fn execute_tester_runs_in_canonical_root_and_captures_output() {
    let fixture = Fixture::new();
    fixture.file("test_marker.txt", "leanai_test_content");

    let allowlist = vec![
        "ls".to_string(),
        "cmd".to_string(),
        "cargo test".to_string(),
    ];

    #[cfg(unix)]
    let cmd = "ls test_marker.txt";
    #[cfg(windows)]
    let cmd = "cmd /c dir test_marker.txt";

    let artifact =
        execute_tester_command(cmd, fixture.root(), &allowlist, None).expect("execution succeeds");
    assert!(artifact.passed);
    assert_eq!(artifact.exit_code, Some(0));
    assert!(artifact.stdout.contains("test_marker.txt"));
    assert!(artifact.summary.contains("PASSED"));
    assert!(artifact.timestamp_ms > 0);
}

#[test]
fn execute_tester_rejects_unallowlisted_commands() {
    let fixture = Fixture::new();
    let allowlist = vec!["cargo test".to_string()];

    let res = execute_tester_command("rm -rf /", fixture.root(), &allowlist, None);
    assert!(res.is_err());
}

#[test]
fn execute_tester_handles_command_failure_without_panic() {
    let fixture = Fixture::new();
    let allowlist = vec!["ls".to_string(), "cmd".to_string()];

    #[cfg(unix)]
    let cmd = "ls non_existent_file_leanai_12345";
    #[cfg(windows)]
    let cmd = "cmd /c dir non_existent_file_leanai_12345";

    let artifact =
        execute_tester_command(cmd, fixture.root(), &allowlist, None).expect("execution captured");
    assert!(!artifact.passed);
    assert_ne!(artifact.exit_code, Some(0));
    assert!(!artifact.stderr.is_empty() || !artifact.stdout.is_empty());
    assert!(artifact.summary.contains("FAILED"));
}

#[test]
fn execute_tester_caps_large_output() {
    let fixture = Fixture::new();
    fixture.file("large.txt", "1234567890abcdefghijklmnopqrstuvwxyz");

    let allowlist = vec!["cat".to_string(), "type".to_string(), "cmd".to_string()];

    #[cfg(unix)]
    let cmd = "cat large.txt";
    #[cfg(windows)]
    let cmd = "cmd /c type large.txt";

    let artifact = execute_tester_command(cmd, fixture.root(), &allowlist, Some(10))
        .expect("execution succeeds");
    assert!(artifact.stdout.contains("[output truncated]"));
}

#[test]
fn execute_tester_sanitizes_environment_secrets() {
    let fixture = Fixture::new();

    // Set sensitive environment variables
    std::env::set_var("OPENAI_API_KEY", "sk-secret-leak-should-be-stripped");
    std::env::set_var("ANTHROPIC_API_KEY", "anthropic-secret-should-be-stripped");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "aws-secret-should-be-stripped");

    let allowlist = vec!["env".to_string(), "cmd".to_string()];

    #[cfg(unix)]
    let cmd = "env";
    #[cfg(windows)]
    let cmd = "cmd /c set";

    let artifact =
        execute_tester_command(cmd, fixture.root(), &allowlist, None).expect("execution succeeds");
    assert!(artifact.passed);
    assert!(!artifact
        .stdout
        .contains("sk-secret-leak-should-be-stripped"));
    assert!(!artifact
        .stdout
        .contains("anthropic-secret-should-be-stripped"));
    assert!(!artifact.stdout.contains("aws-secret-should-be-stripped"));
}
