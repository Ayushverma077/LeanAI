mod support;

use std::collections::HashSet;

use leanai_core::policy::Policy;
use leanai_core::retrieval::{rank_context_for_task, RetrievalConfig};
use leanai_core::walker::{scan, CancelToken, ScanOptions};
use support::Fixture;

#[test]
fn hybrid_retrieval_ranks_by_pins_diffs_and_terms() {
    let fixture = Fixture::new();
    fixture.file(
        "src/auth/login.rs",
        "pub fn login_user() { // user authentication logic }",
    );
    fixture.file(
        "src/auth/token.rs",
        "pub fn verify_jwt() { // jwt token validation }",
    );
    fixture.file(
        "src/utils/math.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    );

    let policy = Policy::default();
    let inventory = scan(
        fixture.root(),
        &ScanOptions::default(),
        &CancelToken::new(),
        |_| {},
    )
    .expect("scan succeeds");

    let mut pinned = HashSet::new();
    pinned.insert("src/auth/login.rs".to_string());

    let mut git_changed = HashSet::new();
    git_changed.insert("src/auth/token.rs".to_string());

    let config = RetrievalConfig::default();

    // Query for authentication
    let result = rank_context_for_task(
        "authenticate user with jwt",
        &inventory,
        &pinned,
        &git_changed,
        fixture.root(),
        &config,
        &policy,
    );

    assert!(!result.proposed_files.is_empty());
    // Both auth files should be ranked at the top
    let top_paths: Vec<_> = result
        .proposed_files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(top_paths.contains(&"src/auth/login.rs"));
    assert!(top_paths.contains(&"src/auth/token.rs"));

    // Verify transparent ranking reasons are exposed (10.3)
    let login_file = result
        .proposed_files
        .iter()
        .find(|f| f.path == "src/auth/login.rs")
        .unwrap();
    assert!(login_file
        .reasons
        .iter()
        .any(|r| r.contains("Explicitly pinned")));

    let token_file = result
        .proposed_files
        .iter()
        .find(|f| f.path == "src/auth/token.rs")
        .unwrap();
    assert!(token_file
        .reasons
        .iter()
        .any(|r| r.contains("Modified in current working tree")));
}

#[test]
fn disabled_retrieval_falls_back_deterministically_to_pinned_files() {
    let fixture = Fixture::new();
    fixture.file("src/main.rs", "fn main() {}");
    fixture.file("src/other.rs", "fn other() {}");

    let policy = Policy::default();
    let inventory = scan(
        fixture.root(),
        &ScanOptions::default(),
        &CancelToken::new(),
        |_| {},
    )
    .expect("scan succeeds");

    let mut pinned = HashSet::new();
    pinned.insert("src/main.rs".to_string());

    let config = RetrievalConfig {
        enabled: false,
        ..Default::default()
    };

    let result = rank_context_for_task(
        "search query",
        &inventory,
        &pinned,
        &HashSet::new(),
        fixture.root(),
        &config,
        &policy,
    );

    // Fallback: only pinned file returned, deterministic
    assert_eq!(result.proposed_files.len(), 1);
    assert_eq!(result.proposed_files[0].path, "src/main.rs");
    assert!(result
        .ranking_disclaimer
        .contains("Retrieval engine disabled"));
}
