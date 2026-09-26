//! Phase 5 acceptance tests: context provenance, freshness and invalidation.

mod support;

use leanai_core::context::{self, Freshness, Generator, SourceRef, SECTION_KEYS};
use leanai_core::walker::{scan, CancelToken, ScanOptions};

fn inventory_of(root: &std::path::Path) -> leanai_core::inventory::Inventory {
    scan(root, &ScanOptions::default(), &CancelToken::new(), |_| {}).unwrap()
}

/// §8.3: every required section is present, in the specified order.
#[test]
fn document_contains_every_required_section_in_order() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    let produced: Vec<&str> = document.sections.iter().map(|s| s.key.as_str()).collect();
    let required: Vec<&str> = SECTION_KEYS.iter().map(|(key, _)| *key).collect();
    assert_eq!(produced, required);

    let markdown = context::render(&document);
    for (_, title) in SECTION_KEYS {
        assert!(markdown.contains(&format!("## {title}")), "missing {title}");
    }
}

/// FR-17: sections carry provenance, and the deterministic pass never claims a
/// model wrote it.
#[test]
fn sections_record_provenance_and_generator() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    assert_eq!(document.source_revision, inventory.source_revision);
    for section in &document.sections {
        assert!(matches!(section.generator, Generator::Deterministic));
        assert_eq!(section.generated_revision, inventory.source_revision);
        for source in &section.source_refs {
            let entry = inventory
                .get(&source.path)
                .unwrap_or_else(|| panic!("{} cited but not in inventory", source.path));
            assert_eq!(source.content_hash, entry.content_hash);
        }
    }
    assert_eq!(document.freshness(), Freshness::Fresh);
}

/// FR-18: uncertainty is stated rather than implied by silence.
#[test]
fn sections_disclose_their_limitations() {
    let fixture = support::monorepo();
    let inventory = inventory_of(fixture.root());
    let document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    for key in [
        "key_modules",
        "api_contracts",
        "dependencies",
        "core_architecture",
    ] {
        assert!(
            !document.section(key).unwrap().limitations.is_empty(),
            "{key} must state what it cannot determine"
        );
    }
    let markdown = context::render(&document);
    assert!(markdown.contains("**Limitations**"));
    assert!(markdown.contains("not** a replacement for reading the code"));
}

/// FR-19: changing a cited file marks the dependent sections stale.
#[test]
fn changed_files_invalidate_dependent_sections() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let mut document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();
    assert_eq!(document.freshness(), Freshness::Fresh);

    fixture.file(
        "src/greet.ts",
        "export function greet(): string { return \"bye\"; }\n",
    );
    let rescanned = inventory_of(fixture.root());
    let impact = context::apply_change_impact(&mut document, &rescanned);

    assert!(impact.changed_files.contains(&"src/greet.ts".to_string()));
    assert!(impact.stale_sections.contains(&"key_modules".to_string()));
    assert_eq!(document.freshness(), Freshness::Stale);
    assert!(context::render(&document).contains("STALE"));
}

/// FR-19: a new file invalidates structural sections even though it is cited
/// by none of them — impact that cannot be proven is treated as impact.
#[test]
fn added_files_conservatively_invalidate_structural_sections() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let mut document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    fixture.file("src/new-module.ts", "export const added = true;\n");
    let rescanned = inventory_of(fixture.root());
    let impact = context::apply_change_impact(&mut document, &rescanned);

    assert!(impact
        .added_files
        .contains(&"src/new-module.ts".to_string()));
    assert!(impact
        .stale_sections
        .contains(&"directory_structure".to_string()));
    assert!(impact
        .stale_sections
        .contains(&"file_inventory".to_string()));
}

/// A deleted source is reported and its sections go stale.
#[test]
fn deleted_files_are_reported() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let mut document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    std::fs::remove_file(fixture.root().join("src/greet.ts")).unwrap();
    let rescanned = inventory_of(fixture.root());
    let impact = context::apply_change_impact(&mut document, &rescanned);
    assert!(impact.deleted_files.contains(&"src/greet.ts".to_string()));
}

/// Regenerating an unchanged project yields the same semantic content.
#[test]
fn generation_is_deterministic() {
    let fixture = support::monorepo();
    let inventory = inventory_of(fixture.root());
    let options = context::GenerateOptions::default();
    let first = context::generate(fixture.root(), &inventory, &options).unwrap();
    let second = context::generate(fixture.root(), &inventory, &options).unwrap();
    assert_eq!(
        context::semantic_hash(&first),
        context::semantic_hash(&second)
    );
    assert_eq!(first.content_hash, second.content_hash);
}

/// Deterministic extraction actually finds the structure it claims to.
#[test]
fn deterministic_sections_extract_real_structure() {
    let fixture = support::monorepo();
    let inventory = inventory_of(fixture.root());
    let document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    let dependencies = &document.section("dependencies").unwrap().body;
    assert!(dependencies.contains("react"));
    assert!(dependencies.contains("serde"));
    assert!(dependencies.contains("fastapi"));

    let modules = &document.section("key_modules").unwrap().body;
    assert!(modules.contains("Engine"));
    assert!(modules.contains("compute"));

    let routes = &document.section("api_contracts").unwrap().body;
    assert!(routes.contains("/items"));

    let workflows = &document.section("workflows").unwrap().body;
    assert!(workflows.contains(".github/workflows/ci.yml"));
}

/// TODO markers and environment-variable *names* are surfaced; values are not.
#[test]
fn known_issues_and_configuration_are_extracted_without_values() {
    let fixture = support::small_app();
    fixture.file(
        "src/env.ts",
        "export const key = process.env.STRIPE_SECRET_KEY;\n// FIXME: move this to a secret manager\n",
    );
    let inventory = inventory_of(fixture.root());
    let document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    let issues = &document.section("known_issues").unwrap().body;
    assert!(issues.contains("FIXME"));
    assert!(issues.contains("src/env.ts"));

    let configuration = &document.section("configuration").unwrap().body;
    assert!(configuration.contains("STRIPE_SECRET_KEY"));
    assert!(configuration.contains("never reads or records their values"));
}

/// Backlog 5.6: a model-written section is rejected unless every citation
/// exists and still hashes to the recorded value.
#[test]
fn model_sections_are_rejected_without_valid_citations() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let entry = inventory.get("src/index.ts").unwrap();

    let mut section = context::ContextSection {
        key: "summary".to_string(),
        title: "Project Summary".to_string(),
        body: "The service exposes a health endpoint.".to_string(),
        source_refs: vec![SourceRef::file("src/index.ts", entry.content_hash.clone())],
        freshness: Freshness::Fresh,
        generator: Generator::Model {
            provider: "test".to_string(),
            model: "test-model".to_string(),
            prompt_version: "v1".to_string(),
        },
        limitations: Vec::new(),
        generated_revision: inventory.source_revision.clone(),
        generated_at_ms: 0,
    };
    assert!(context::validate_model_section(&section, &inventory).is_ok());

    // No citations at all.
    let mut uncited = section.clone();
    uncited.source_refs.clear();
    let problems = context::validate_model_section(&uncited, &inventory).unwrap_err();
    assert!(problems
        .iter()
        .any(|p| p.contains("must cite at least one source")));

    // Citing a file that does not exist.
    let mut invented = section.clone();
    invented.source_refs = vec![SourceRef::file("src/imaginary.ts", Some("deadbeef".into()))];
    let problems = context::validate_model_section(&invented, &inventory).unwrap_err();
    assert!(problems.iter().any(|p| p.contains("not in the project")));

    // Citing a file whose content changed after the claim was written.
    section.source_refs = vec![SourceRef::file("src/index.ts", Some("stalehash".into()))];
    let problems = context::validate_model_section(&section, &inventory).unwrap_err();
    assert!(problems.iter().any(|p| p.contains("changed since")));
}

/// A model-written section is visibly labelled in the rendered document.
#[test]
fn model_written_sections_are_labelled_in_the_output() {
    let fixture = support::small_app();
    let inventory = inventory_of(fixture.root());
    let mut document = context::generate(
        fixture.root(),
        &inventory,
        &context::GenerateOptions::default(),
    )
    .unwrap();

    let summary = document
        .sections
        .iter_mut()
        .find(|section| section.key == "summary")
        .unwrap();
    summary.generator = Generator::Model {
        provider: "acme".to_string(),
        model: "writer-1".to_string(),
        prompt_version: "documenter/v3".to_string(),
    };

    let markdown = context::render(&document);
    assert!(markdown.contains("Written by `acme/writer-1`"));
    assert!(markdown.contains("documenter/v3"));
}
