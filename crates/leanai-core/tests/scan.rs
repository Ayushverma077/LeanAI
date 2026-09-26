//! Phase 2 acceptance tests: safe project access, scanning and inventory.

mod support;

use std::collections::HashSet;

use leanai_core::classify::FileClass;
use leanai_core::policy::Policy;
use leanai_core::project;
use leanai_core::walker::{scan, CancelToken, ScanOptions};

fn scan_fixture(root: &std::path::Path, options: ScanOptions) -> leanai_core::inventory::Inventory {
    scan(root, &options, &CancelToken::new(), |_| {}).expect("scan succeeds")
}

/// FR-02, FR-09: ordering is lexicographic and stable across runs.
#[test]
fn scan_is_deterministic_and_ordered() {
    let fixture = support::small_app();
    let first = scan_fixture(fixture.root(), ScanOptions::default());
    let second = scan_fixture(fixture.root(), ScanOptions::default());

    let paths: Vec<&str> = first.files.iter().map(|f| f.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(
        paths, sorted,
        "scan output must be lexicographically ordered"
    );
    assert_eq!(
        paths,
        second
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(first.source_revision, second.source_revision);
}

/// FR-02: nested `.gitignore` rules, including negations, are honoured.
#[test]
fn nested_gitignore_rules_and_negations_are_respected() {
    let fixture = support::nested_ignore();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());
    let paths: HashSet<&str> = inventory.files.iter().map(|f| f.path.as_str()).collect();

    assert!(
        paths.contains("keep.log"),
        "negated rule must re-include keep.log"
    );
    assert!(paths.contains("src/public-notes.md"));
    assert!(!paths.contains("drop.log"), "*.log must be ignored");
    assert!(
        !paths.contains("src/secret-notes.md"),
        "nested .gitignore must apply to its own directory"
    );
    assert!(!paths.contains("build/output.js"), "build/ must be ignored");
}

/// FR-04: binaries are caught by extension *and* by content probe.
#[test]
fn binary_detection_uses_extension_and_content_probe() {
    let fixture = support::binary_heavy();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    assert_eq!(
        inventory.get("assets/logo.png").unwrap().class,
        FileClass::Binary
    );
    let blob = inventory.get("data/blob.dat").unwrap();
    assert_eq!(
        blob.class,
        FileClass::Binary,
        "a NUL byte in the probe window means binary even without a known extension"
    );
    assert!(!blob.selectable);
}

/// FR-04: oversized, invalid-encoding, lockfile and generated files each get a
/// distinct, explainable classification.
#[test]
fn unsafe_and_unhelpful_files_are_classified_with_reasons() {
    let fixture = support::binary_heavy();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    let huge = inventory.get("data/huge.txt").unwrap();
    assert_eq!(huge.class, FileClass::TooLarge);
    assert!(huge.exclusion.as_ref().unwrap().reason.contains("exceeds"));

    assert_eq!(
        inventory.get("data/invalid.txt").unwrap().class,
        FileClass::UnsupportedEncoding
    );
    assert_eq!(
        inventory.get("package-lock.json").unwrap().class,
        FileClass::Lockfile
    );
    assert_eq!(
        inventory.get("public/app.min.js").unwrap().class,
        FileClass::Generated
    );
    assert!(
        inventory.get("dist/bundle.min.js").is_none(),
        "dist/ is an always-skipped directory, so it is never walked"
    );

    for entry in &inventory.files {
        if !entry.selectable {
            assert!(
                entry.exclusion.is_some(),
                "{} is not selectable but has no reason",
                entry.path
            );
        }
    }
}

/// FR-04 / NFR 7.1: credential paths are excluded by policy, and `.env.example`
/// is not swept up with them.
#[test]
fn credential_paths_are_excluded_but_examples_are_not() {
    let fixture = support::secrets_repo();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    for path in [".env", "deploy/id_rsa", "deploy/service-account.json"] {
        let entry = inventory
            .get(path)
            .unwrap_or_else(|| panic!("{path} missing"));
        assert_eq!(entry.class, FileClass::CredentialSensitive, "{path}");
        assert!(!entry.selectable, "{path} must not be selectable");
    }
    let example = inventory.get(".env.example").unwrap();
    assert_ne!(
        example.class,
        FileClass::CredentialSensitive,
        ".env.example is a documented exception"
    );
}

/// FR-05: every entry carries path, size, class, and a content hash for text.
#[test]
fn inventory_records_hash_and_freshness_metadata() {
    let fixture = support::small_app();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    let entry = inventory.get("src/index.ts").unwrap();
    assert!(entry.size_bytes > 0);
    assert!(entry.modified_ms.is_some());
    let hash = entry.content_hash.as_ref().expect("text files are hashed");
    let expected =
        project::sha256_hex(&std::fs::read(fixture.root().join("src/index.ts")).unwrap());
    assert_eq!(hash, &expected);
}

/// FR-06: a cancelled scan returns `Cancelled` rather than a partial inventory.
#[test]
fn cancellation_returns_an_error_not_partial_data() {
    let fixture = support::monorepo();
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = scan(fixture.root(), &ScanOptions::default(), &cancel, |_| {});
    assert!(matches!(result, Err(leanai_core::CoreError::Cancelled)));
}

/// NFR 7.1: no scanned path resolves outside the approved root, and a symlink
/// pointing outside is reported rather than followed.
#[cfg(unix)]
#[test]
fn symlinks_never_escape_the_project_root() {
    let outside = tempfile::TempDir::new().unwrap();
    std::fs::write(outside.path().join("secret.txt"), "not yours\n").unwrap();

    let fixture = support::small_app();
    fixture.symlink(outside.path(), "linked");
    fixture.symlink(&outside.path().join("secret.txt"), "src/escape.ts");

    let inventory = scan_fixture(fixture.root(), ScanOptions::default());
    let root = project::canonical_root(fixture.root()).unwrap();

    for entry in &inventory.files {
        assert!(
            !entry.path.contains(".."),
            "{} contains a traversal segment",
            entry.path
        );
        if entry.class != FileClass::Symlink {
            let resolved = project::resolve_within_root(&root, &entry.path);
            assert!(resolved.is_ok(), "{} escaped the root", entry.path);
        }
    }

    let escape = inventory.get("src/escape.ts").expect("symlink is listed");
    assert_eq!(escape.class, FileClass::Symlink);
    assert!(!escape.selectable, "symlinks are not followed by default");
    assert!(escape
        .exclusion
        .as_ref()
        .unwrap()
        .reason
        .contains("outside the project root"));

    assert!(
        !inventory
            .files
            .iter()
            .any(|entry| entry.path.contains("secret.txt")),
        "content behind a symlinked directory must not be walked"
    );
}

/// NFR 7.1: path resolution rejects absolute paths and traversal outright.
#[test]
fn path_resolution_rejects_traversal() {
    let fixture = support::small_app();
    let root = project::canonical_root(fixture.root()).unwrap();

    assert!(project::resolve_within_root(&root, "src/index.ts").is_ok());
    for hostile in ["../outside.txt", "src/../../outside.txt", "/etc/passwd"] {
        assert!(
            project::resolve_within_root(&root, hostile).is_err(),
            "{hostile} must be rejected"
        );
    }
}

/// FR-06 + 7.4: the scan stops at the configured file limit and says so.
#[test]
fn scan_reports_truncation_at_the_file_limit() {
    let fixture = support::monorepo();
    let mut policy = Policy::default();
    policy.limits.max_files_scanned = 3;
    let inventory = scan_fixture(
        fixture.root(),
        ScanOptions {
            policy,
            ..Default::default()
        },
    );
    assert!(inventory.stats.truncated);
    assert_eq!(inventory.files.len(), 3);
}

/// Progress is emitted so the UI can stay responsive during a long scan.
#[test]
fn scan_emits_progress() {
    let fixture = support::monorepo();
    let mut updates = 0;
    let options = ScanOptions {
        progress_every: 1,
        ..Default::default()
    };
    scan(fixture.root(), &options, &CancelToken::new(), |_| {
        updates += 1
    })
    .unwrap();
    assert!(updates > 1, "expected repeated progress callbacks");
}

/// Always-skipped directories are never walked, so a `node_modules` tree costs
/// nothing.
#[test]
fn vendor_directories_are_not_walked() {
    let fixture = support::monorepo();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());
    assert!(!inventory
        .files
        .iter()
        .any(|entry| entry.path.starts_with("node_modules/")));
}

/// FR-06 / NFR 7.1 / Backlog 11.2: scanner gracefully reports ScanIssue on unreadable
/// directories and unreadable files under real EACCES conditions without crashing.
#[test]
fn scan_handles_permission_restricted_directories_and_files() {
    let fixture = support::permission_restricted();
    let options = ScanOptions::default();
    let inventory = scan(fixture.root(), &options, &CancelToken::new(), |_| {})
        .expect("scan must succeed even when paths are permission-restricted");

    assert!(
        inventory.files.iter().any(|f| f.path == "readable/main.ts"),
        "readable/main.ts should be found in inventory"
    );

    #[cfg(unix)]
    {
        assert!(
            !inventory.issues.is_empty(),
            "permission-restricted paths must generate ScanIssue entries"
        );
        assert!(
            inventory
                .issues
                .iter()
                .any(|issue| issue.path.contains("unreadable")),
            "issues must identify unreadable paths"
        );
    }
}

/// FR-06 / NFR 7.4 / Backlog 2.7 + 11.2: scanner handles a 100,000+ file tree deterministically,
/// emits regular progress updates, and completes within performance limits.
#[test]
fn scan_large_tree_repository_performance() {
    let count = 100_000;
    let fixture = support::large_tree(count);
    let mut progress_count = 0;
    let options = ScanOptions {
        progress_every: 5_000,
        ..Default::default()
    };

    let start = std::time::Instant::now();
    let inventory = scan(fixture.root(), &options, &CancelToken::new(), |_| {
        progress_count += 1;
    })
    .expect("100k file scan must succeed");
    let duration = start.elapsed();

    assert_eq!(
        inventory.files.len(),
        count,
        "all 100k files must be inventoried"
    );
    assert!(
        !inventory.stats.truncated,
        "100k files should be within default 200k limit"
    );
    assert!(
        progress_count >= 15,
        "progress callback should have been called periodically"
    );
    assert!(
        inventory.files.windows(2).all(|w| w[0].path <= w[1].path),
        "large tree inventory must be sorted lexicographically"
    );
    println!(
        "100k scan completed in {:?} with {} progress events",
        duration, progress_count
    );
}

/// A build-tool output directory that no ignore file covers is still kept out
/// of the selectable set. On LeanAI's own repository `src-tauri/gen/` was 43%
/// of the total token count before this rule existed.
#[test]
fn build_tool_output_directories_are_marked_generated() {
    let fixture = support::binary_heavy();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    let entry = inventory
        .get("src-tauri/gen/schemas/desktop-schema.json")
        .expect("the file is still listed, with a reason");
    assert_eq!(entry.class, FileClass::Generated);
    assert!(!entry.selectable);
}

/// Structured data is held to a much lower ceiling than source: a 128 KB JSON
/// file passes the 1 MB code limit but is thousands of tokens of noise.
#[test]
fn large_data_files_are_excluded_by_a_separate_ceiling() {
    let fixture = support::binary_heavy();
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    let big = inventory.get("config/big-fixture.json").unwrap();
    assert_eq!(big.class, FileClass::TooLarge);
    assert!(!big.selectable);
    let reason = &big.exclusion.as_ref().unwrap().reason;
    assert!(reason.contains("structured data"), "{reason}");
    assert_eq!(
        big.exclusion.as_ref().unwrap().rule.as_deref(),
        Some("max_data_file_bytes")
    );

    // A small JSON file is ordinary source text and stays selectable.
    let small = inventory.get("config/small.json").unwrap();
    assert_eq!(small.class, FileClass::SourceText);
    assert!(small.selectable);
}

/// The data ceiling must not swallow ordinary source files of the same size.
#[test]
fn the_data_ceiling_does_not_apply_to_code() {
    let fixture = support::Fixture::new();
    fixture.file("src/big_module.ts", &"export const x = 1;\n".repeat(8_000));
    let inventory = scan_fixture(fixture.root(), ScanOptions::default());

    let entry = inventory.get("src/big_module.ts").unwrap();
    assert!(
        entry.size_bytes > 65_536,
        "fixture must exceed the data ceiling"
    );
    assert_eq!(entry.class, FileClass::SourceText);
    assert!(entry.selectable);
}

/// Settings written before `max_data_file_bytes` existed must still load, or
/// upgrading LeanAI would silently reset every stored preference.
#[test]
fn limits_from_an_older_release_still_deserialize() {
    let older = r#"{
        "maxFileBytes": 2048,
        "maxFilesScanned": 1000,
        "maxSelectedFiles": 50,
        "maxBundleBytes": 4096,
        "binaryProbeBytes": 512,
        "maxDepth": 8
    }"#;

    let limits: leanai_core::policy::Limits =
        serde_json::from_str(older).expect("older settings must still parse");
    assert_eq!(limits.max_file_bytes, 2048, "existing values are preserved");
    assert_eq!(
        limits.max_data_file_bytes,
        Policy::default().limits.max_data_file_bytes,
        "the new field falls back to its default rather than zero"
    );
}
