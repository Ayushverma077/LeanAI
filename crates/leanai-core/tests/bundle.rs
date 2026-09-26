//! Phase 3 acceptance tests: selection safety, deterministic bundling, token
//! labelling and the bundle manifest.

mod support;

use leanai_core::concat::{self, BundleOptions};
use leanai_core::manifest::BundleManifest;
use leanai_core::policy::Policy;
use leanai_core::selection::{self, SelectionSpec};
use leanai_core::tokenizer::{self, EstimateKind};
use leanai_core::walker::{scan, CancelToken, ScanOptions};

fn inventory_of(root: &std::path::Path) -> leanai_core::inventory::Inventory {
    scan(root, &ScanOptions::default(), &CancelToken::new(), |_| {}).unwrap()
}

/// FR-09: same project revision + selection + options ⇒ identical bytes.
#[test]
fn bundles_are_byte_identical_across_runs() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let policy = Policy::default();
    let spec = SelectionSpec {
        directories: vec!["src".to_string()],
        ..Default::default()
    };
    let selection = selection::resolve(&inventory, &spec, &policy).unwrap();
    let options = BundleOptions::default();

    let first = concat::build(fixture.root(), &inventory, &selection, &options).unwrap();
    let second = concat::build(fixture.root(), &inventory, &selection, &options).unwrap();

    assert_eq!(first.output_hash, second.output_hash);
    assert_eq!(first.text, second.text);
    assert_eq!(first.file_count, 3);
}

/// FR-09: selection order does not change the output.
#[test]
fn selection_order_does_not_change_the_bundle() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let policy = Policy::default();
    let options = BundleOptions::default();

    let forward = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["src/greet.ts", "src/index.ts"]),
        &policy,
    )
    .unwrap();
    let reverse = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["src/index.ts", "src/greet.ts"]),
        &policy,
    )
    .unwrap();

    let a = concat::build(fixture.root(), &inventory, &forward, &options).unwrap();
    let b = concat::build(fixture.root(), &inventory, &reverse, &options).unwrap();
    assert_eq!(a.output_hash, b.output_hash);
}

/// FR-08: a folder selection cannot pull in a credential-sensitive file.
#[test]
fn folder_selection_never_includes_blocked_files() {
    let fixture = support::secrets_repo();
    let inventory = inventory_of(fixture.root());
    let policy = Policy::default();

    let selection = selection::resolve(
        &inventory,
        &SelectionSpec {
            directories: vec![String::new()], // the whole project
            ..Default::default()
        },
        &policy,
    )
    .unwrap();

    for blocked in [".env", "deploy/id_rsa", "deploy/service-account.json"] {
        assert!(
            !selection.files.iter().any(|path| path == blocked),
            "{blocked} must never be added by a folder selection"
        );
    }
    assert!(selection.files.iter().any(|path| path == "src/config.ts"));
}

/// FR-08: an explicit request for a blocked file is rejected with a reason, and
/// only a per-file override lets it through.
#[test]
fn blocked_files_require_an_explicit_per_file_override() {
    let fixture = support::secrets_repo();
    let inventory = inventory_of(fixture.root());
    let policy = Policy::default();

    let denied =
        selection::resolve(&inventory, &SelectionSpec::from_files([".env"]), &policy).unwrap();
    assert!(denied.files.is_empty());
    assert_eq!(denied.rejected.len(), 1);
    assert!(denied.rejected[0].overridable);
    assert!(denied.rejected[0].reason.contains("credentials"));

    let allowed = selection::resolve(
        &inventory,
        &SelectionSpec {
            files: vec![".env".to_string()],
            overrides: vec![".env".to_string()],
            ..Default::default()
        },
        &policy,
    )
    .unwrap();
    assert_eq!(allowed.files, vec![".env".to_string()]);
}

/// An unreadable or invalid-encoding file cannot be overridden into a bundle:
/// there is nothing safe to include.
#[test]
fn unreadable_files_cannot_be_overridden() {
    let fixture = support::binary_heavy();
    let inventory = inventory_of(fixture.root());
    let result = selection::resolve(
        &inventory,
        &SelectionSpec {
            files: vec!["data/invalid.txt".to_string()],
            overrides: vec!["data/invalid.txt".to_string()],
            ..Default::default()
        },
        &Policy::default(),
    )
    .unwrap();
    assert!(result.files.is_empty());
    assert!(!result.rejected[0].overridable);
}

/// FR-10: every output mode has a stable, inspectable shape.
#[test]
fn output_modes_are_reversible_and_snapshot_stable() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let selection = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["src/greet.ts"]),
        &Policy::default(),
    )
    .unwrap();

    let plain = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions {
            include_tree: false,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        plain.text,
        "===== src/greet.ts =====\nexport interface Greeting {\n  text: string;\n}\n\nexport function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n\n"
    );

    let fenced = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions {
            include_tree: false,
            code_fences: true,
            line_numbers: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(fenced.text.contains("```typescript\n"));
    assert!(fenced.text.contains("1 | export interface Greeting {"));
    assert!(fenced.text.trim_end().ends_with("```"));

    let annotated = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions {
            include_tree: false,
            file_size_annotations: true,
            ..Default::default()
        },
    )
    .unwrap();
    let expected_bytes = std::fs::metadata(fixture.root().join("src/greet.ts"))
        .unwrap()
        .len();
    assert!(annotated
        .text
        .starts_with(&format!("===== src/greet.ts ({expected_bytes} B, ~")));
    assert!(annotated.text.contains("tokens) ====="));
}

/// FR-10: the tree preamble reflects exactly the selected files.
#[test]
fn tree_preamble_lists_the_selection() {
    let paths = vec![
        "README.md".to_string(),
        "src/index.ts".to_string(),
        "src/util/format.ts".to_string(),
    ];
    assert_eq!(
        concat::render_tree(&paths),
        "README.md\nsrc/\n  index.ts\n  util/\n    format.ts\n"
    );
}

/// FR-09: CRLF input produces the same bytes as LF input.
#[test]
fn line_endings_are_normalized() {
    let fixture = support::Fixture::new();
    fixture.file("crlf.ts", "const a = 1;\r\nconst b = 2;\r\n");
    fixture.file("lf.ts", "const a = 1;\nconst b = 2;\n");
    let inventory = inventory_of(fixture.root());
    let options = BundleOptions {
        include_tree: false,
        headers: false,
        ..Default::default()
    };

    let crlf = concat::build(
        fixture.root(),
        &inventory,
        &selection::resolve(
            &inventory,
            &SelectionSpec::from_files(["crlf.ts"]),
            &Policy::default(),
        )
        .unwrap(),
        &options,
    )
    .unwrap();
    let lf = concat::build(
        fixture.root(),
        &inventory,
        &selection::resolve(
            &inventory,
            &SelectionSpec::from_files(["lf.ts"]),
            &Policy::default(),
        )
        .unwrap(),
        &options,
    )
    .unwrap();
    assert_eq!(crlf.text, lf.text);
    assert!(!crlf.text.contains('\r'));
}

/// FR-13 / §2: a local count is always labelled as an OpenAI-family estimate.
#[test]
fn token_estimates_disclose_their_scope() {
    let estimate = tokenizer::estimate("const greeting = \"hello world\";");
    assert!(estimate.value > 0);
    assert!(matches!(
        estimate.kind,
        EstimateKind::OpenAiFamilyEstimate { .. }
    ));
    assert!(estimate.kind.label().contains("estimate"));
    assert!(estimate.kind.label().contains("cl100k_base"));
    assert!(!estimate.kind.is_exact());
}

/// FR-13: per-file contributions add up and are reported for the preview.
#[test]
fn per_file_contributions_are_reported() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let selection = selection::resolve(
        &inventory,
        &SelectionSpec {
            directories: vec!["src".to_string()],
            ..Default::default()
        },
        &Policy::default(),
    )
    .unwrap();
    let bundle = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions::default(),
    )
    .unwrap();

    assert_eq!(bundle.contributions.len(), 3);
    let sum: f64 = bundle.contributions.iter().map(|c| c.share).sum();
    assert!(
        sum > 0.5 && sum <= 1.0,
        "shares should be a sane fraction: {sum}"
    );
    assert!(bundle.contributions.iter().all(|c| c.tokens > 0));
}

/// FR-12 / §8.2: the manifest records identity, revision, options and the
/// estimate's provenance, and warns that the count is an estimate.
#[test]
fn manifest_records_provenance_and_warnings() {
    let fixture = support::secrets_repo();
    let inventory = inventory_of(fixture.root());
    let selection = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["src/config.ts", ".env"]),
        &Policy::default(),
    )
    .unwrap();
    let options = BundleOptions::default();
    let bundle = concat::build(fixture.root(), &inventory, &selection, &options).unwrap();

    let manifest = BundleManifest::new(
        &inventory,
        &selection,
        &options,
        &bundle.estimate,
        &bundle.truncations,
        &bundle.skipped,
    );
    assert_eq!(manifest.format, "leanai.bundle/v1");
    assert_eq!(manifest.project_fingerprint, inventory.project_fingerprint);
    assert_eq!(manifest.token_estimate.kind, "openai_family_estimate");
    assert!(manifest
        .selection
        .excluded_files
        .contains(&".env".to_string()));
    assert!(manifest.selection.exclusion_reasons.contains_key(".env"));
    assert!(manifest
        .warnings
        .iter()
        .any(|warning| warning.contains("not a provider count")));

    let front_matter = manifest.to_front_matter();
    assert!(front_matter.starts_with("---\n"));
    assert!(front_matter.trim_end().ends_with("---"));
    assert!(front_matter.contains("format: leanai.bundle/v1"));
    assert!(front_matter.contains("kind: openai_family_estimate"));
}

/// Front matter is included in the hashed output when the option is on.
#[test]
fn front_matter_is_part_of_the_bundle_when_enabled() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let selection = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["README.md"]),
        &Policy::default(),
    )
    .unwrap();
    let bundle = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions {
            include_front_matter: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(bundle.text.starts_with("---\nformat: leanai.bundle/v1"));
}

/// 7.4: selection limits fail loudly with an actionable message.
#[test]
fn selection_limits_are_enforced() {
    let fixture = support::monorepo();
    let inventory = inventory_of(fixture.root());
    let mut policy = Policy::default();
    policy.limits.max_selected_files = 1;
    let result = selection::resolve(
        &inventory,
        &SelectionSpec {
            directories: vec![String::new()],
            ..Default::default()
        },
        &policy,
    );
    match result {
        Err(leanai_core::CoreError::LimitExceeded(message)) => {
            assert!(message.contains("the limit is 1"));
        }
        other => panic!("expected a limit error, got {other:?}"),
    }
}

/// A preset that refers to files which no longer exist reports them instead of
/// silently dropping them (backlog 4.1).
#[test]
fn missing_paths_are_reported_for_preset_revalidation() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let result = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["src/index.ts", "src/deleted.ts"]),
        &Policy::default(),
    )
    .unwrap();
    assert_eq!(result.files, vec!["src/index.ts".to_string()]);
    assert_eq!(result.missing, vec!["src/deleted.ts".to_string()]);
}

/// Truncation is applied at a char boundary and warned about.
#[test]
fn oversized_files_are_truncated_with_a_warning() {
    let fixture = support::Fixture::new();
    fixture.file("big.ts", &"const x = \"ünïcödé\";\n".repeat(200));
    let inventory = inventory_of(fixture.root());
    let selection = selection::resolve(
        &inventory,
        &SelectionSpec::from_files(["big.ts"]),
        &Policy::default(),
    )
    .unwrap();
    let bundle = concat::build(
        fixture.root(),
        &inventory,
        &selection,
        &BundleOptions {
            max_file_bytes: Some(101),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(bundle.truncations.len(), 1);
    assert!(bundle.text.contains("[truncated by LeanAI]"));
}

/// A recipe is the answer to "why must I click a hundred checkboxes?" — it
/// proposes a starting selection instead of an empty one.
#[test]
fn the_source_recipe_leaves_out_tests_docs_and_examples() {
    let fixture = support::small_app();
    fixture.file("src/greet.test.ts", "test('greets', () => {});\n");
    fixture.file("tests/e2e/checkout.ts", "export const flow = 1;\n");
    fixture.file("docs/architecture.md", "# Architecture\n");
    fixture.file("examples/demo.ts", "export const demo = 1;\n");
    fixture.file("src/api/handler.ts", "export function handle() {}\n");

    let inventory = inventory_of(fixture.root());
    let suggestion = selection::suggest(&inventory, selection::SelectionRecipe::SourceOnly);

    assert!(suggestion.files.contains(&"src/api/handler.ts".to_string()));
    assert!(suggestion.files.contains(&"src/index.ts".to_string()));
    for excluded in [
        "src/greet.test.ts",
        "tests/e2e/checkout.ts",
        "docs/architecture.md",
        "examples/demo.ts",
    ] {
        assert!(
            !suggestion.files.contains(&excluded.to_string()),
            "{excluded} should not be in the source recipe"
        );
    }
    assert_eq!(suggestion.skipped_by_recipe, 4);
    assert!(suggestion.description.contains("leaving out tests"));
}

/// The tests recipe is the mirror image, for when the suite is the task.
#[test]
fn the_tests_recipe_selects_only_tests() {
    let fixture = support::small_app();
    fixture.file("src/greet.test.ts", "test('greets', () => {});\n");
    let inventory = inventory_of(fixture.root());

    let suggestion = selection::suggest(&inventory, selection::SelectionRecipe::TestsOnly);
    assert_eq!(suggestion.files, vec!["src/greet.test.ts".to_string()]);
}

/// No recipe can propose a file the policy blocks — a recipe is a shortcut
/// through the *allowed* set, never around it (FR-08).
#[test]
fn recipes_never_propose_a_blocked_file() {
    let fixture = support::secrets_repo();
    let inventory = inventory_of(fixture.root());

    for recipe in [
        selection::SelectionRecipe::SourceOnly,
        selection::SelectionRecipe::Everything,
        selection::SelectionRecipe::TestsOnly,
    ] {
        let suggestion = selection::suggest(&inventory, recipe);
        for blocked in [".env", "deploy/id_rsa", "deploy/service-account.json"] {
            assert!(
                !suggestion.files.contains(&blocked.to_string()),
                "{recipe:?} proposed {blocked}"
            );
        }
    }
}

/// Directory summaries let a folder picker show real weight per folder, and
/// each directory counts its whole subtree.
#[test]
fn directory_summaries_roll_up_the_subtree() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let summaries = selection::directory_summaries(&inventory);

    let src = summaries
        .iter()
        .find(|summary| summary.path == "src")
        .expect("src is summarised");
    // src/index.ts, src/greet.ts and src/util/format.ts
    assert_eq!(src.selectable_files, 3);
    assert_eq!(src.depth, 0);

    let util = summaries
        .iter()
        .find(|summary| summary.path == "src/util")
        .expect("nested directories are summarised too");
    assert_eq!(util.selectable_files, 1);
    assert_eq!(util.depth, 1);
    assert!(src.total_bytes > util.total_bytes);

    // Directories holding no selectable file are not offered.
    assert!(!summaries
        .iter()
        .any(|summary| summary.selectable_files == 0));
}
