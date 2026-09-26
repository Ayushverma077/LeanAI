use rusqlite::Connection;

use crate::error::AppResult;

/// One forward migration. `down` is recorded for documentation and for the
/// downgrade rehearsal in the release runbook; it is not run automatically.
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub up: &'static str,
    pub down: &'static str,
}

/// Schema history. Append only — never edit a released migration.
///
/// Credentials are deliberately absent: API keys live in OS secure storage, so
/// a stolen database file contains no secrets (FR-24, backlog 1.6).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        up: r#"
CREATE TABLE settings (
    key           TEXT PRIMARY KEY,
    value         TEXT NOT NULL,
    updated_at_ms INTEGER NOT NULL
);

CREATE TABLE projects (
    id                  TEXT PRIMARY KEY,
    fingerprint         TEXT NOT NULL UNIQUE,
    canonical_path      TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    last_scan_revision  TEXT,
    last_opened_at_ms   INTEGER,
    policy_version      INTEGER NOT NULL,
    created_at_ms       INTEGER NOT NULL,
    updated_at_ms       INTEGER NOT NULL
);

CREATE TABLE presets (
    id             TEXT PRIMARY KEY,
    project_id     TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    selection_spec TEXT NOT NULL,
    bundle_options TEXT NOT NULL,
    created_at_ms  INTEGER NOT NULL,
    updated_at_ms  INTEGER NOT NULL,
    UNIQUE (project_id, name)
);

CREATE TABLE bundles (
    id                 TEXT PRIMARY KEY,
    project_id         TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_revision    TEXT NOT NULL,
    selection_manifest TEXT NOT NULL,
    output_hash        TEXT NOT NULL,
    estimate_value     INTEGER NOT NULL,
    estimate_kind      TEXT NOT NULL,
    file_count         INTEGER NOT NULL,
    byte_len           INTEGER NOT NULL,
    -- Only populated when the retention setting allows storing bundle text.
    output_text        TEXT,
    created_at_ms      INTEGER NOT NULL
);

CREATE INDEX idx_bundles_project ON bundles(project_id, created_at_ms DESC);

CREATE TABLE context_documents (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_revision TEXT NOT NULL,
    schema_version  INTEGER NOT NULL,
    content_hash    TEXT NOT NULL,
    freshness_state TEXT NOT NULL,
    document        TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL
);

CREATE INDEX idx_context_project ON context_documents(project_id, created_at_ms DESC);

CREATE TABLE context_sections (
    id                  TEXT PRIMARY KEY,
    context_document_id TEXT NOT NULL REFERENCES context_documents(id) ON DELETE CASCADE,
    section_key         TEXT NOT NULL,
    source_refs         TEXT NOT NULL,
    freshness_state     TEXT NOT NULL,
    generator           TEXT NOT NULL
);

CREATE INDEX idx_sections_document ON context_sections(context_document_id);
"#,
        down: "DROP TABLE context_sections; DROP TABLE context_documents; DROP TABLE bundles; DROP TABLE presets; DROP TABLE projects; DROP TABLE settings;",
    },
    Migration {
        version: 2,
        name: "run_ledger",
        up: r#"
-- Audit root for any consequential action, including exports made before the
-- agent workspace exists (NFR 7.1: "maintain an audit record for every
-- consequential decision").
CREATE TABLE runs (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task             TEXT NOT NULL,
    mode             TEXT NOT NULL,
    context_manifest TEXT NOT NULL,
    policy           TEXT NOT NULL,
    status           TEXT NOT NULL,
    budget           TEXT,
    started_at_ms    INTEGER NOT NULL,
    ended_at_ms      INTEGER,
    validation_state TEXT
);

CREATE INDEX idx_runs_project ON runs(project_id, started_at_ms DESC);

CREATE TABLE run_events (
    id              TEXT PRIMARY KEY,
    run_id          TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    sequence        INTEGER NOT NULL,
    event_type      TEXT NOT NULL,
    redacted_payload TEXT NOT NULL,
    created_at_ms   INTEGER NOT NULL,
    UNIQUE (run_id, sequence)
);

CREATE TABLE approvals (
    id            TEXT PRIMARY KEY,
    run_id        TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    capability    TEXT NOT NULL,
    scope         TEXT NOT NULL,
    decision      TEXT NOT NULL,
    approver      TEXT NOT NULL,
    decided_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER
);

CREATE INDEX idx_approvals_run ON approvals(run_id);
"#,
        down: "DROP TABLE approvals; DROP TABLE run_events; DROP TABLE runs;",
    },
    Migration {
        version: 3,
        name: "models_and_providers",
        up: r#"
CREATE TABLE models (
    id                 TEXT PRIMARY KEY,
    kind               TEXT NOT NULL,
    display_name       TEXT NOT NULL,
    source             TEXT NOT NULL,
    version            TEXT NOT NULL,
    file_path          TEXT,
    capability_profile TEXT NOT NULL,
    checksum           TEXT,
    status             TEXT NOT NULL,
    created_at_ms      INTEGER NOT NULL,
    updated_at_ms      INTEGER NOT NULL
);

CREATE INDEX idx_models_kind ON models(kind);

CREATE TABLE provider_configs (
    provider_id        TEXT PRIMARY KEY,
    display_name       TEXT NOT NULL,
    account_label      TEXT NOT NULL,
    is_configured      INTEGER NOT NULL DEFAULT 0,
    price_catalog_ver  TEXT NOT NULL,
    enabled            INTEGER NOT NULL DEFAULT 1,
    updated_at_ms      INTEGER NOT NULL
);
"#,
        down: "DROP TABLE provider_configs; DROP TABLE models;",
    },
    Migration {
        version: 4,
        name: "agent_allowlists_and_memory",
        up: r#"
CREATE TABLE project_command_allowlists (
    project_id         TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    command_pattern    TEXT NOT NULL,
    created_at_ms      INTEGER NOT NULL,
    PRIMARY KEY (project_id, command_pattern)
);

CREATE TABLE project_memory (
    id                 TEXT PRIMARY KEY,
    project_id         TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_summary       TEXT NOT NULL,
    relevant_paths     TEXT NOT NULL,
    key_findings       TEXT NOT NULL,
    created_at_ms      INTEGER NOT NULL,
    expires_at_ms      INTEGER NOT NULL
);

CREATE INDEX idx_memory_project ON project_memory(project_id, created_at_ms DESC);
"#,
        down: "DROP TABLE project_memory; DROP TABLE project_command_allowlists;",
    },
];

pub const LATEST_VERSION: u32 = 4;

/// Applies pending migrations inside a transaction each, so a failure leaves
/// the database at the last good version rather than half-migrated.
pub fn migrate(connection: &mut Connection) -> AppResult<u32> {
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version     INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            applied_at_ms INTEGER NOT NULL
        )",
        [],
    )?;

    let current: u32 = connection
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current > LATEST_VERSION {
        return Err(crate::error::AppError::new(
            "database_too_new",
            format!(
                "This database was created by a newer version of LeanAI (schema v{current}); this build understands v{LATEST_VERSION}."
            ),
        )
        .with_recovery("Update LeanAI, or open Settings on the newer version to export your data."));
    }

    for migration in MIGRATIONS.iter().filter(|m| m.version > current) {
        let transaction = connection.transaction()?;
        transaction.execute_batch(migration.up)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?3)",
            rusqlite::params![
                migration.version,
                migration.name,
                leanai_core::project::now_ms() as i64
            ],
        )?;
        transaction.commit()?;
    }

    Ok(LATEST_VERSION)
}
