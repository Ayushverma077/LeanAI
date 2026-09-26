//! Phase 1 acceptance tests: migrations, repositories and retention.

use leanai_core::concat::BundleOptions;
use leanai_core::selection::SelectionSpec;
use leanai_desktop_lib::db::migrations::{self, LATEST_VERSION};
use leanai_desktop_lib::db::repositories::{self, BundleRetention};
use rusqlite::Connection;

fn migrated() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    migrations::migrate(&mut connection).unwrap();
    connection
}

/// Backlog 1.6: migrations apply from empty and are idempotent.
#[test]
fn migrations_apply_and_are_idempotent() {
    let mut connection = Connection::open_in_memory().unwrap();
    assert_eq!(
        migrations::migrate(&mut connection).unwrap(),
        LATEST_VERSION
    );
    assert_eq!(
        migrations::migrate(&mut connection).unwrap(),
        LATEST_VERSION
    );

    let applied: u32 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(applied, migrations::MIGRATIONS.len() as u32);
}

/// Migration versions are unique and strictly increasing: an accidental
/// duplicate would silently skip a schema change on upgrade.
#[test]
fn migration_versions_are_ordered_and_unique() {
    let versions: Vec<u32> = migrations::MIGRATIONS.iter().map(|m| m.version).collect();
    let mut sorted = versions.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(versions, sorted);
    assert_eq!(versions.last().copied().unwrap(), LATEST_VERSION);
}

/// A database written by a newer build fails safely instead of being migrated
/// backwards into corruption.
#[test]
fn a_newer_database_is_refused_with_a_clear_message() {
    let mut connection = migrated();
    connection
        .execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (999, 'future', 0)",
            [],
        )
        .unwrap();
    let error = migrations::migrate(&mut connection).unwrap_err();
    assert_eq!(error.code, "database_too_new");
    assert!(error.recovery.unwrap().contains("Update LeanAI"));
}

/// FR-24: the schema has nowhere to put a credential.
#[test]
fn no_table_stores_credentials() {
    let connection = migrated();
    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();

    for table in &tables {
        let columns: Vec<String> = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for column in columns {
            let lower = column.to_ascii_lowercase();
            assert!(
                !(lower.contains("api_key")
                    || lower.contains("secret")
                    || lower.contains("token")
                    || lower.contains("password")),
                "{table}.{column} looks like credential storage; keys belong in OS secure storage"
            );
        }
    }
}

/// Projects are keyed by fingerprint, so reopening the same repository updates
/// the existing row instead of creating a duplicate.
#[test]
fn reopening_a_project_updates_rather_than_duplicates() {
    let connection = migrated();
    let first = repositories::upsert_project(&connection, "proj_a", "/tmp/a", "a", 1).unwrap();
    let second = repositories::upsert_project(&connection, "proj_a", "/tmp/moved", "a", 1).unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(second.canonical_path, "/tmp/moved");
    assert_eq!(repositories::list_projects(&connection).unwrap().len(), 1);
}

/// Backlog 4.1: presets round-trip their rule, not a frozen file list.
#[test]
fn presets_round_trip_and_upsert_by_name() {
    let connection = migrated();
    let project = repositories::upsert_project(&connection, "proj_b", "/tmp/b", "b", 1).unwrap();
    let spec = SelectionSpec {
        directories: vec!["src".into()],
        overrides: vec![".env".into()],
        ..Default::default()
    };

    repositories::save_preset(
        &connection,
        &project.id,
        "backend",
        &spec,
        &BundleOptions::default(),
    )
    .unwrap();
    let mut changed = spec.clone();
    changed.directories = vec!["src/api".into()];
    repositories::save_preset(
        &connection,
        &project.id,
        "backend",
        &changed,
        &BundleOptions::default(),
    )
    .unwrap();

    let presets = repositories::list_presets(&connection, &project.id).unwrap();
    assert_eq!(
        presets.len(),
        1,
        "saving under the same name must update it"
    );
    assert_eq!(
        presets[0].selection.directories,
        vec!["src/api".to_string()]
    );
    assert_eq!(presets[0].selection.overrides, vec![".env".to_string()]);
}

/// Backlog 4.7 / NFR 7.1: retention is honoured, so "metadata only" never
/// leaves a copy of the user's source in the database.
#[test]
fn retention_controls_whether_bundle_text_is_stored() {
    let connection = migrated();
    let project = repositories::upsert_project(&connection, "proj_c", "/tmp/c", "c", 1).unwrap();
    let manifest = sample_manifest();

    for (retention, expect_row, expect_text) in [
        (BundleRetention::MetadataOnly, true, false),
        (BundleRetention::FullText, true, true),
        (BundleRetention::None, false, false),
    ] {
        let id = repositories::record_bundle(
            &connection,
            &project.id,
            "git:abc",
            &manifest,
            "hash",
            10,
            "openai_family_estimate",
            2,
            100,
            Some("SECRET SOURCE TEXT"),
            retention,
        )
        .unwrap();
        assert_eq!(id.is_some(), expect_row, "{retention:?}");

        if let Some(id) = id {
            let stored: Option<String> = connection
                .query_row(
                    "SELECT output_text FROM bundles WHERE id = ?1",
                    [&id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(stored.is_some(), expect_text, "{retention:?}");
        }
    }
}

/// Deleting a project removes everything derived from it and reports what went.
#[test]
fn forgetting_a_project_cascades_and_reports() {
    let mut connection = migrated();
    let project = repositories::upsert_project(&connection, "proj_d", "/tmp/d", "d", 1).unwrap();
    repositories::save_preset(
        &connection,
        &project.id,
        "p",
        &SelectionSpec::default(),
        &BundleOptions::default(),
    )
    .unwrap();
    repositories::record_bundle(
        &connection,
        &project.id,
        "git:abc",
        &sample_manifest(),
        "hash",
        1,
        "openai_family_estimate",
        1,
        1,
        None,
        BundleRetention::MetadataOnly,
    )
    .unwrap();

    let summary = repositories::delete_project(&connection, &project.id).unwrap();
    assert_eq!(summary["presetsDeleted"], 1);
    assert_eq!(summary["bundlesDeleted"], 1);
    assert_eq!(summary["filesOnDiskDeleted"], 0);
    assert!(summary["note"]
        .as_str()
        .unwrap()
        .contains("No file in your project was touched"));

    assert!(repositories::list_projects(&connection).unwrap().is_empty());
    let orphan_presets: i64 = connection
        .query_row("SELECT COUNT(*) FROM presets", [], |row| row.get(0))
        .unwrap();
    assert_eq!(orphan_presets, 0, "cascade must not leave orphans");
    let _ = &mut connection;
}

/// Every export is auditable, before any agent exists.
#[test]
fn exports_are_recorded_in_the_audit_log() {
    let connection = migrated();
    let project = repositories::upsert_project(&connection, "proj_e", "/tmp/e", "e", 1).unwrap();
    repositories::record_audit(
        &connection,
        &project.id,
        "export_bundle",
        "clipboard",
        &serde_json::json!({ "fileCount": 3 }),
        "succeeded",
    )
    .unwrap();

    let entries = repositories::list_audit(&connection, &project.id, 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["task"], "export_bundle");
    assert_eq!(entries[0]["mode"], "clipboard");
    assert_eq!(entries[0]["manifest"]["fileCount"], 3);
}

fn sample_manifest() -> leanai_core::manifest::BundleManifest {
    leanai_core::manifest::BundleManifest {
        format: "leanai.bundle/v1".into(),
        project_fingerprint: "proj_test".into(),
        source_revision: "git:abc".into(),
        policy_version: 1,
        created_at_ms: 0,
        selection: leanai_core::manifest::SelectionManifest {
            included_files: vec!["src/main.rs".into()],
            excluded_files: vec![],
            exclusion_reasons: Default::default(),
        },
        options: BundleOptions::default(),
        token_estimate: leanai_core::manifest::TokenEstimateManifest {
            value: 10,
            kind: "openai_family_estimate".into(),
            tokenizer_or_provider: "cl100k_base".into(),
        },
        warnings: vec![],
    }
}
