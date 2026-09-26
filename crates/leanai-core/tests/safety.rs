//! Phase 4 acceptance tests: `.aiignore`, git-diff selection and export review.

mod support;

use leanai_core::aiignore;
use leanai_core::gitinfo::{self, DiffRequest, DiffScope};
use leanai_core::secrets::{self, Confidence, SecretReport};
use leanai_core::walker::{scan, CancelToken, ScanOptions};

fn inventory_of(root: &std::path::Path) -> leanai_core::inventory::Inventory {
    scan(root, &ScanOptions::default(), &CancelToken::new(), |_| {}).unwrap()
}

/// FR-03 / backlog 4.2: the effect of a candidate `.aiignore` is previewed
/// before it is written.
#[test]
fn aiignore_preview_reports_effect_before_saving() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());

    let preview = aiignore::preview(
        fixture.root(),
        "src/util/\n*.nothing-matches-this\nbad[pattern\n",
        &inventory,
    )
    .unwrap();

    assert_eq!(
        preview.newly_excluded,
        vec!["src/util/format.ts".to_string()]
    );
    assert_eq!(
        preview.selectable_after,
        preview.selectable_before - 1,
        "the preview must state the resulting count"
    );
    assert!(preview
        .unmatched_rules
        .contains(&"*.nothing-matches-this".to_string()));
    // The `.aiignore` file itself was not written.
    assert!(!fixture.root().join(".aiignore").exists());
}

/// `.aiignore` is layered above `.gitignore` and takes effect during a scan.
#[test]
fn aiignore_is_applied_by_the_scanner() {
    let fixture = support::small_app();
    let before = inventory_of(fixture.root());
    assert!(before.get("src/util/format.ts").is_some());

    fixture.file(".aiignore", "src/util/\n");
    let after = inventory_of(fixture.root());
    assert!(
        after.get("src/util/format.ts").is_none(),
        ".aiignore must exclude the path from the scan"
    );
    assert!(after.get("src/index.ts").is_some());
}

/// `.aiignore` cannot re-include a credential-sensitive path: LeanAI's safety
/// policy outranks it (docs/adr/0003).
#[test]
fn aiignore_cannot_reexpose_a_blocked_path() {
    let fixture = support::secrets_repo();
    fixture.file(".aiignore", "!.env\n");
    let inventory = inventory_of(fixture.root());
    let entry = inventory.get(".env").expect(".env is still listed");
    assert!(
        !entry.selectable,
        "a negation in .aiignore must not unlock a credential path"
    );
}

/// FR-12: a git project's revision includes the commit and a dirty marker.
#[test]
fn source_revision_tracks_git_state() {
    let fixture = support::small_app();
    let repo = fixture.git_init();
    fixture.git_commit_all(&repo, "initial");

    let clean = inventory_of(fixture.root());
    assert!(clean.source_revision.starts_with("git:"));
    assert!(!clean.source_revision.ends_with("+dirty"));

    fixture.file("src/greet.ts", "export function greet() { return 1; }\n");
    let dirty = inventory_of(fixture.root());
    assert!(dirty.source_revision.ends_with("+dirty"));
}

/// A project without git still gets a deterministic revision from its content.
#[test]
fn non_git_projects_get_a_content_revision() {
    let fixture = support::small_app();
    let first = inventory_of(fixture.root());
    assert!(first.source_revision.starts_with("scan:"));
    let second = inventory_of(fixture.root());
    assert_eq!(first.source_revision, second.source_revision);

    fixture.file("src/greet.ts", "export function greet() { return 2; }\n");
    let third = inventory_of(fixture.root());
    assert_ne!(first.source_revision, third.source_revision);
}

/// Backlog 4.3: diff mode distinguishes staged, unstaged and untracked work.
#[test]
fn git_diff_mode_separates_change_scopes() {
    let fixture = support::small_app();
    let repo = fixture.git_init();
    fixture.git_commit_all(&repo, "initial");

    fixture.file(
        "src/greet.ts",
        "export function greet() { return \"changed\"; }\n",
    );
    fixture.file("src/brand-new.ts", "export const fresh = true;\n");

    let changed = gitinfo::changed_files(
        fixture.root(),
        &DiffRequest {
            scopes: vec![DiffScope::Unstaged, DiffScope::Untracked],
            base_ref: None,
        },
    )
    .unwrap();

    let modified = changed
        .iter()
        .find(|file| file.path == "src/greet.ts")
        .unwrap();
    assert_eq!(modified.scope, DiffScope::Unstaged);
    let added = changed
        .iter()
        .find(|file| file.path == "src/brand-new.ts")
        .unwrap();
    assert_eq!(added.scope, DiffScope::Untracked);

    // Asking only for untracked work excludes the modified file.
    let untracked_only = gitinfo::changed_files(
        fixture.root(),
        &DiffRequest {
            scopes: vec![DiffScope::Untracked],
            base_ref: None,
        },
    )
    .unwrap();
    assert!(!untracked_only
        .iter()
        .any(|file| file.path == "src/greet.ts"));
}

/// Diff against an explicit base reference, and a clear error for a bad ref.
#[test]
fn git_diff_against_a_base_ref() {
    let fixture = support::small_app();
    let repo = fixture.git_init();
    let first = fixture.git_commit_all(&repo, "initial");
    fixture.file(
        "src/greet.ts",
        "export function greet() { return \"v2\"; }\n",
    );
    fixture.git_commit_all(&repo, "second");

    let changed = gitinfo::changed_files(
        fixture.root(),
        &DiffRequest {
            scopes: vec![DiffScope::AgainstRef],
            base_ref: Some(first.to_string()),
        },
    )
    .unwrap();
    assert!(changed.iter().any(|file| file.path == "src/greet.ts"));

    let error = gitinfo::changed_files(
        fixture.root(),
        &DiffRequest {
            scopes: vec![DiffScope::AgainstRef],
            base_ref: Some("no-such-ref".to_string()),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("no-such-ref"));
}

/// A non-git project reports that clearly instead of failing obscurely.
#[test]
fn diff_mode_falls_back_for_non_git_projects() {
    let fixture = support::small_app();
    let state = gitinfo::git_state(fixture.root());
    assert!(!state.is_repository);

    let error = gitinfo::changed_files(
        fixture.root(),
        &DiffRequest {
            scopes: vec![DiffScope::Unstaged],
            base_ref: None,
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        leanai_core::CoreError::NotAGitRepository(_)
    ));
}

/// Backlog 4.5: secret findings are graded, redacted and never claim to be
/// complete.
#[test]
fn secret_findings_are_graded_and_redacted() {
    let text = "const gh = \"ghp_0123456789abcdefghijklmnopqrstuvwxyz\";\nconst debug = true;\n";
    let findings = secrets::scan_text("src/leaky.ts", text, 10);

    assert_eq!(findings.len(), 1);
    let finding = &findings[0];
    assert_eq!(finding.confidence, Confidence::High);
    assert_eq!(finding.line, 1);
    assert!(
        !finding
            .redacted_excerpt
            .contains("0123456789abcdefghijklmnopqrstuvwxyz"),
        "the raw secret must never appear in a finding: {}",
        finding.redacted_excerpt
    );
    assert!(finding.redacted_excerpt.contains("[redacted"));

    let mut report = SecretReport::default();
    report.push(findings);
    assert_eq!(report.high, 1);
    assert!(report.requires_confirmation());
    assert!(SecretReport::DISCLAIMER.contains("can miss real secrets"));
}

/// The scanner recognises the common provider token formats and a private key
/// block, and does not fire on ordinary code.
#[test]
fn secret_rules_cover_common_formats_without_flagging_plain_code() {
    let hits = [
        "AWS_KEY = \"AKIAIOSFODNN7EXAMPLE\"",
        "-----BEGIN RSA PRIVATE KEY-----",
        "DATABASE_URL=postgres://admin:hunter2@db.internal:5432/app",
        "stripe = 'sk_live_abcdefghijklmnop1234'",
    ];
    for line in hits {
        assert!(
            !secrets::scan_text("f.ts", line, 5).is_empty(),
            "expected a finding for: {line}"
        );
    }

    let misses = [
        "const total = items.reduce((a, b) => a + b, 0);",
        "// the password field is validated on the server",
        "import { key } from './key';",
    ];
    for line in misses {
        assert!(
            secrets::scan_text("f.ts", line, 5).is_empty(),
            "unexpected finding for: {line}"
        );
    }
}

/// Findings are capped per file so a generated fixture cannot flood review.
#[test]
fn secret_findings_are_capped_per_file() {
    let text = "token = \"ghp_0123456789abcdefghijklmnopqrstuvwxyz\"\n".repeat(50);
    let findings = secrets::scan_text("dump.txt", &text, 5);
    assert_eq!(findings.len(), 5);
}
