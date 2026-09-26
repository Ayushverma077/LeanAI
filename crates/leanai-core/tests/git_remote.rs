//! Acceptance tests for Git remote tracking and inspection.

mod support;

use leanai_core::gitinfo;

#[test]
fn remote_status_non_repository() {
    let fixture = support::Fixture::new();
    let status = gitinfo::remote_status(fixture.root()).expect("query succeeds");
    assert!(!status.is_repository);
    assert!(status.current_branch.is_none());
    assert!(status.remotes.is_empty());
}

#[test]
fn remote_status_detects_branch_and_remotes() {
    let fixture = support::Fixture::new();
    let repo = fixture.git_init();
    fixture.file("README.md", "# Test Project\n");
    fixture.git_commit_all(&repo, "Initial commit");

    // Add SSH and HTTPS remotes
    repo.remote("origin", "git@github.com:octocat/Hello-World.git")
        .expect("add origin");
    repo.remote(
        "upstream",
        "https://github.com/octocat/Hello-World-Fork.git",
    )
    .expect("add upstream");

    let status = gitinfo::remote_status(fixture.root()).expect("query succeeds");
    assert!(status.is_repository);
    assert!(status.current_branch.is_some());
    assert!(!status.is_dirty);
    assert_eq!(status.remotes.len(), 2);

    let origin = status
        .remotes
        .iter()
        .find(|r| r.name == "origin")
        .expect("origin found");
    assert_eq!(origin.url, "git@github.com:octocat/Hello-World.git");
    assert_eq!(origin.protocol, "ssh");

    let upstream = status
        .remotes
        .iter()
        .find(|r| r.name == "upstream")
        .expect("upstream found");
    assert_eq!(
        upstream.url,
        "https://github.com/octocat/Hello-World-Fork.git"
    );
    assert_eq!(upstream.protocol, "https");
}
