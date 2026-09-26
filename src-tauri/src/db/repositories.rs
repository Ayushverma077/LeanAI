use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use leanai_core::concat::BundleOptions;
use leanai_core::context::ContextDocument;
use leanai_core::project::now_ms;
use leanai_core::selection::SelectionSpec;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRecord {
    pub id: String,
    pub fingerprint: String,
    pub canonical_path: String,
    pub display_name: String,
    pub last_scan_revision: Option<String>,
    pub last_opened_at_ms: Option<i64>,
    pub policy_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetRecord {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub selection: SelectionSpec,
    pub options: BundleOptions,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleRecord {
    pub id: String,
    pub project_id: String,
    pub source_revision: String,
    pub output_hash: String,
    pub estimate_value: i64,
    pub estimate_kind: String,
    pub file_count: i64,
    pub byte_len: i64,
    /// Present only when the retention setting allows storing bundle text.
    pub has_text: bool,
    pub created_at_ms: i64,
}

/// Retention choices. The default keeps metadata but not bundle text, so
/// history stays useful without accumulating copies of the user's source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleRetention {
    /// Store the manifest and hash only.
    #[default]
    MetadataOnly,
    /// Also store the bundle text, so it can be re-copied later.
    FullText,
    /// Store nothing.
    None,
}

pub fn upsert_project(
    connection: &Connection,
    fingerprint: &str,
    canonical_path: &str,
    display_name: &str,
    policy_version: u32,
) -> AppResult<ProjectRecord> {
    let now = now_ms() as i64;
    let existing: Option<String> = connection
        .query_row(
            "SELECT id FROM projects WHERE fingerprint = ?1",
            params![fingerprint],
            |row| row.get(0),
        )
        .optional()?;

    let id = match existing {
        Some(id) => {
            connection.execute(
                "UPDATE projects SET canonical_path = ?2, display_name = ?3, last_opened_at_ms = ?4, policy_version = ?5, updated_at_ms = ?4 WHERE id = ?1",
                params![id, canonical_path, display_name, now, policy_version],
            )?;
            id
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            connection.execute(
                "INSERT INTO projects (id, fingerprint, canonical_path, display_name, last_opened_at_ms, policy_version, created_at_ms, updated_at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5, ?5)",
                params![id, fingerprint, canonical_path, display_name, now, policy_version],
            )?;
            id
        }
    };
    get_project(connection, &id)?.ok_or_else(|| AppError::internal("project vanished after upsert"))
}

pub fn get_project(connection: &Connection, id: &str) -> AppResult<Option<ProjectRecord>> {
    Ok(connection
        .query_row(
            "SELECT id, fingerprint, canonical_path, display_name, last_scan_revision, last_opened_at_ms, policy_version
             FROM projects WHERE id = ?1",
            params![id],
            map_project,
        )
        .optional()?)
}

pub fn list_projects(connection: &Connection) -> AppResult<Vec<ProjectRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, fingerprint, canonical_path, display_name, last_scan_revision, last_opened_at_ms, policy_version
         FROM projects ORDER BY last_opened_at_ms DESC NULLS LAST, display_name ASC",
    )?;
    let rows = statement.query_map([], map_project)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn map_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectRecord> {
    Ok(ProjectRecord {
        id: row.get(0)?,
        fingerprint: row.get(1)?,
        canonical_path: row.get(2)?,
        display_name: row.get(3)?,
        last_scan_revision: row.get(4)?,
        last_opened_at_ms: row.get(5)?,
        policy_version: row.get(6)?,
    })
}

pub fn set_scan_revision(connection: &Connection, id: &str, revision: &str) -> AppResult<()> {
    connection.execute(
        "UPDATE projects SET last_scan_revision = ?2, updated_at_ms = ?3 WHERE id = ?1",
        params![id, revision, now_ms() as i64],
    )?;
    Ok(())
}

/// Removes a project and everything derived from it. Returns what was deleted
/// so the UI can say exactly what happened (backlog 4.7).
pub fn delete_project(connection: &Connection, id: &str) -> AppResult<serde_json::Value> {
    let presets: i64 = connection.query_row(
        "SELECT COUNT(*) FROM presets WHERE project_id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    let bundles: i64 = connection.query_row(
        "SELECT COUNT(*) FROM bundles WHERE project_id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    let contexts: i64 = connection.query_row(
        "SELECT COUNT(*) FROM context_documents WHERE project_id = ?1",
        params![id],
        |row| row.get(0),
    )?;
    connection.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
    Ok(serde_json::json!({
        "presetsDeleted": presets,
        "bundlesDeleted": bundles,
        "contextDocumentsDeleted": contexts,
        "filesOnDiskDeleted": 0,
        "note": "LeanAI removed its own records only. No file in your project was touched."
    }))
}

pub fn save_preset(
    connection: &Connection,
    project_id: &str,
    name: &str,
    selection: &SelectionSpec,
    options: &BundleOptions,
) -> AppResult<PresetRecord> {
    let now = now_ms() as i64;
    let selection_json =
        serde_json::to_string(selection).map_err(|e| AppError::internal(e.to_string()))?;
    let options_json =
        serde_json::to_string(options).map_err(|e| AppError::internal(e.to_string()))?;
    let id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO presets (id, project_id, name, selection_spec, bundle_options, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(project_id, name) DO UPDATE SET
            selection_spec = excluded.selection_spec,
            bundle_options = excluded.bundle_options,
            updated_at_ms  = excluded.updated_at_ms",
        params![id, project_id, name, selection_json, options_json, now],
    )?;
    list_presets(connection, project_id)?
        .into_iter()
        .find(|preset| preset.name == name)
        .ok_or_else(|| AppError::internal("preset vanished after save"))
}

pub fn list_presets(connection: &Connection, project_id: &str) -> AppResult<Vec<PresetRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, project_id, name, selection_spec, bundle_options, updated_at_ms
         FROM presets WHERE project_id = ?1 ORDER BY name ASC",
    )?;
    let rows = statement.query_map(params![project_id], |row| {
        let selection: String = row.get(3)?;
        let options: String = row.get(4)?;
        Ok(PresetRecord {
            id: row.get(0)?,
            project_id: row.get(1)?,
            name: row.get(2)?,
            selection: serde_json::from_str(&selection).unwrap_or_default(),
            options: serde_json::from_str(&options).unwrap_or_default(),
            updated_at_ms: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn rename_preset(connection: &Connection, id: &str, name: &str) -> AppResult<()> {
    connection.execute(
        "UPDATE presets SET name = ?2, updated_at_ms = ?3 WHERE id = ?1",
        params![id, name, now_ms() as i64],
    )?;
    Ok(())
}

pub fn delete_preset(connection: &Connection, id: &str) -> AppResult<()> {
    connection.execute("DELETE FROM presets WHERE id = ?1", params![id])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn record_bundle(
    connection: &Connection,
    project_id: &str,
    source_revision: &str,
    manifest: &leanai_core::manifest::BundleManifest,
    output_hash: &str,
    estimate_value: usize,
    estimate_kind: &str,
    file_count: usize,
    byte_len: u64,
    text: Option<&str>,
    retention: BundleRetention,
) -> AppResult<Option<String>> {
    if retention == BundleRetention::None {
        return Ok(None);
    }
    let id = uuid::Uuid::new_v4().to_string();
    let manifest_json =
        serde_json::to_string(manifest).map_err(|e| AppError::internal(e.to_string()))?;
    let stored_text = match retention {
        BundleRetention::FullText => text,
        _ => None,
    };
    connection.execute(
        "INSERT INTO bundles (id, project_id, source_revision, selection_manifest, output_hash, estimate_value, estimate_kind, file_count, byte_len, output_text, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            project_id,
            source_revision,
            manifest_json,
            output_hash,
            estimate_value as i64,
            estimate_kind,
            file_count as i64,
            byte_len as i64,
            stored_text,
            now_ms() as i64
        ],
    )?;
    Ok(Some(id))
}

pub fn list_bundles(
    connection: &Connection,
    project_id: &str,
    limit: u32,
) -> AppResult<Vec<BundleRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, project_id, source_revision, output_hash, estimate_value, estimate_kind, file_count, byte_len, output_text IS NOT NULL, created_at_ms
         FROM bundles WHERE project_id = ?1 ORDER BY created_at_ms DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![project_id, limit], |row| {
        Ok(BundleRecord {
            id: row.get(0)?,
            project_id: row.get(1)?,
            source_revision: row.get(2)?,
            output_hash: row.get(3)?,
            estimate_value: row.get(4)?,
            estimate_kind: row.get(5)?,
            file_count: row.get(6)?,
            byte_len: row.get(7)?,
            has_text: row.get(8)?,
            created_at_ms: row.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn clear_history(connection: &Connection, project_id: &str) -> AppResult<usize> {
    Ok(connection.execute(
        "DELETE FROM bundles WHERE project_id = ?1",
        params![project_id],
    )?)
}

pub fn save_context(
    connection: &mut Connection,
    project_id: &str,
    document: &ContextDocument,
) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let document_json =
        serde_json::to_string(document).map_err(|e| AppError::internal(e.to_string()))?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO context_documents (id, project_id, source_revision, schema_version, content_hash, freshness_state, document, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            project_id,
            document.source_revision,
            document.schema_version,
            document.content_hash,
            format!("{:?}", document.freshness()).to_lowercase(),
            document_json,
            now_ms() as i64
        ],
    )?;
    for section in &document.sections {
        transaction.execute(
            "INSERT INTO context_sections (id, context_document_id, section_key, source_refs, freshness_state, generator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                uuid::Uuid::new_v4().to_string(),
                id,
                section.key,
                serde_json::to_string(&section.source_refs).unwrap_or_else(|_| "[]".into()),
                format!("{:?}", section.freshness).to_lowercase(),
                serde_json::to_string(&section.generator).unwrap_or_else(|_| "{}".into())
            ],
        )?;
    }
    transaction.commit()?;
    Ok(id)
}

pub fn latest_context(
    connection: &Connection,
    project_id: &str,
) -> AppResult<Option<ContextDocument>> {
    let json: Option<String> = connection
        .query_row(
            "SELECT document FROM context_documents WHERE project_id = ?1 ORDER BY created_at_ms DESC LIMIT 1",
            params![project_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(json.and_then(|json| serde_json::from_str(&json).ok()))
}

pub fn get_setting(connection: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()?)
}

pub fn set_setting(connection: &Connection, key: &str, value: &str) -> AppResult<()> {
    connection.execute(
        "INSERT INTO settings (key, value, updated_at_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
        params![key, value, now_ms() as i64],
    )?;
    Ok(())
}

/// Appends an audit entry. Used for exports today and for agent runs later.
pub fn record_audit(
    connection: &Connection,
    project_id: &str,
    task: &str,
    mode: &str,
    manifest: &serde_json::Value,
    status: &str,
) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO runs (id, project_id, task, mode, context_manifest, policy, status, started_at_ms, ended_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id,
            project_id,
            task,
            mode,
            manifest.to_string(),
            serde_json::json!({ "policyVersion": leanai_core::policy::POLICY_VERSION }).to_string(),
            status,
            now_ms() as i64
        ],
    )?;
    Ok(id)
}

pub fn list_audit(
    connection: &Connection,
    project_id: &str,
    limit: u32,
) -> AppResult<Vec<serde_json::Value>> {
    let mut statement = connection.prepare(
        "SELECT id, task, mode, context_manifest, status, started_at_ms
         FROM runs WHERE project_id = ?1 ORDER BY started_at_ms DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![project_id, limit], |row| {
        let manifest: String = row.get(3)?;
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "task": row.get::<_, String>(1)?,
            "mode": row.get::<_, String>(2)?,
            "manifest": serde_json::from_str::<serde_json::Value>(&manifest).unwrap_or(serde_json::Value::Null),
            "status": row.get::<_, String>(4)?,
            "startedAtMs": row.get::<_, i64>(5)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRecord {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub source: String,
    pub version: String,
    pub file_path: Option<String>,
    pub capability_profile: leanai_core::provider::CapabilityProfile,
    pub checksum: Option<String>,
    pub status: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfigRecord {
    pub provider_id: String,
    pub display_name: String,
    pub account_label: String,
    pub is_configured: bool,
    pub price_catalog_ver: String,
    pub enabled: bool,
    pub updated_at_ms: i64,
}

pub fn upsert_model(connection: &Connection, record: &ModelRecord) -> AppResult<ModelRecord> {
    let now = now_ms() as i64;
    let profile_json = serde_json::to_string(&record.capability_profile)
        .map_err(|e| AppError::internal(e.to_string()))?;
    connection.execute(
        "INSERT INTO models (id, kind, display_name, source, version, file_path, capability_profile, checksum, status, created_at_ms, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            kind = ?2,
            display_name = ?3,
            source = ?4,
            version = ?5,
            file_path = ?6,
            capability_profile = ?7,
            checksum = ?8,
            status = ?9,
            updated_at_ms = ?11",
        params![
            record.id,
            record.kind,
            record.display_name,
            record.source,
            record.version,
            record.file_path,
            profile_json,
            record.checksum,
            record.status,
            record.created_at_ms,
            now,
        ],
    )?;
    get_model(connection, &record.id)?
        .ok_or_else(|| AppError::internal("model disappeared after upsert"))
}

pub fn list_models(connection: &Connection) -> AppResult<Vec<ModelRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, kind, display_name, source, version, file_path, capability_profile, checksum, status, created_at_ms, updated_at_ms
         FROM models ORDER BY display_name ASC",
    )?;
    let rows = statement.query_map([], |row| {
        let profile_raw: String = row.get(6)?;
        let profile = serde_json::from_str(&profile_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(ModelRecord {
            id: row.get(0)?,
            kind: row.get(1)?,
            display_name: row.get(2)?,
            source: row.get(3)?,
            version: row.get(4)?,
            file_path: row.get(5)?,
            capability_profile: profile,
            checksum: row.get(7)?,
            status: row.get(8)?,
            created_at_ms: row.get(9)?,
            updated_at_ms: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_model(connection: &Connection, id: &str) -> AppResult<Option<ModelRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, kind, display_name, source, version, file_path, capability_profile, checksum, status, created_at_ms, updated_at_ms
         FROM models WHERE id = ?1",
    )?;
    let mut rows = statement.query_map(params![id], |row| {
        let profile_raw: String = row.get(6)?;
        let profile = serde_json::from_str(&profile_raw).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(ModelRecord {
            id: row.get(0)?,
            kind: row.get(1)?,
            display_name: row.get(2)?,
            source: row.get(3)?,
            version: row.get(4)?,
            file_path: row.get(5)?,
            capability_profile: profile,
            checksum: row.get(7)?,
            status: row.get(8)?,
            created_at_ms: row.get(9)?,
            updated_at_ms: row.get(10)?,
        })
    })?;

    match rows.next() {
        Some(result) => Ok(Some(result?)),
        None => Ok(None),
    }
}

pub fn delete_model(connection: &Connection, id: &str) -> AppResult<bool> {
    let affected = connection.execute("DELETE FROM models WHERE id = ?1", params![id])?;
    Ok(affected > 0)
}

pub fn list_provider_configs(connection: &Connection) -> AppResult<Vec<ProviderConfigRecord>> {
    let mut statement = connection.prepare(
        "SELECT provider_id, display_name, account_label, is_configured, price_catalog_ver, enabled, updated_at_ms
         FROM provider_configs ORDER BY provider_id ASC",
    )?;
    let rows = statement.query_map([], |row| {
        let is_configured_int: i64 = row.get(3)?;
        let enabled_int: i64 = row.get(5)?;
        Ok(ProviderConfigRecord {
            provider_id: row.get(0)?,
            display_name: row.get(1)?,
            account_label: row.get(2)?,
            is_configured: is_configured_int != 0,
            price_catalog_ver: row.get(4)?,
            enabled: enabled_int != 0,
            updated_at_ms: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn set_provider_config(
    connection: &Connection,
    provider_id: &str,
    display_name: &str,
    account_label: &str,
    is_configured: bool,
    price_catalog_ver: &str,
    enabled: bool,
) -> AppResult<ProviderConfigRecord> {
    let now = now_ms() as i64;
    connection.execute(
        "INSERT INTO provider_configs (provider_id, display_name, account_label, is_configured, price_catalog_ver, enabled, updated_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider_id) DO UPDATE SET
            display_name = ?2,
            account_label = ?3,
            is_configured = ?4,
            price_catalog_ver = ?5,
            enabled = ?6,
            updated_at_ms = ?7",
        params![
            provider_id,
            display_name,
            account_label,
            if is_configured { 1 } else { 0 },
            price_catalog_ver,
            if enabled { 1 } else { 0 },
            now,
        ],
    )?;

    Ok(ProviderConfigRecord {
        provider_id: provider_id.to_string(),
        display_name: display_name.to_string(),
        account_label: account_label.to_string(),
        is_configured,
        price_catalog_ver: price_catalog_ver.to_string(),
        enabled,
        updated_at_ms: now,
    })
}

pub fn delete_provider_config(connection: &Connection, provider_id: &str) -> AppResult<bool> {
    let affected = connection.execute(
        "DELETE FROM provider_configs WHERE provider_id = ?1",
        params![provider_id],
    )?;
    Ok(affected > 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: String,
    pub project_id: String,
    pub task: String,
    pub mode: String,
    pub context_manifest: serde_json::Value,
    pub policy: serde_json::Value,
    pub status: String,
    pub budget: Option<serde_json::Value>,
    pub started_at_ms: i64,
    pub ended_at_ms: Option<i64>,
    pub validation_state: Option<serde_json::Value>,
}

pub fn upsert_run(connection: &Connection, record: &RunRecord) -> AppResult<()> {
    connection.execute(
        "INSERT INTO runs (id, project_id, task, mode, context_manifest, policy, status, budget, started_at_ms, ended_at_ms, validation_state)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            ended_at_ms = excluded.ended_at_ms,
            validation_state = excluded.validation_state",
        params![
            record.id,
            record.project_id,
            record.task,
            record.mode,
            record.context_manifest.to_string(),
            record.policy.to_string(),
            record.status,
            record.budget.as_ref().map(|b| b.to_string()),
            record.started_at_ms,
            record.ended_at_ms,
            record.validation_state.as_ref().map(|v| v.to_string()),
        ],
    )?;
    Ok(())
}

pub fn get_run(connection: &Connection, id: &str) -> AppResult<Option<RunRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, project_id, task, mode, context_manifest, policy, status, budget, started_at_ms, ended_at_ms, validation_state
         FROM runs WHERE id = ?1",
    )?;
    let row = statement
        .query_row(params![id], |row| {
            let manifest_str: String = row.get(4)?;
            let policy_str: String = row.get(5)?;
            let budget_str: Option<String> = row.get(7)?;
            let val_str: Option<String> = row.get(10)?;
            Ok(RunRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                task: row.get(2)?,
                mode: row.get(3)?,
                context_manifest: serde_json::from_str(&manifest_str)
                    .unwrap_or(serde_json::Value::Null),
                policy: serde_json::from_str(&policy_str).unwrap_or(serde_json::Value::Null),
                status: row.get(6)?,
                budget: budget_str.and_then(|s| serde_json::from_str(&s).ok()),
                started_at_ms: row.get(8)?,
                ended_at_ms: row.get(9)?,
                validation_state: val_str.and_then(|s| serde_json::from_str(&s).ok()),
            })
        })
        .optional()?;
    Ok(row)
}

pub fn list_runs(
    connection: &Connection,
    project_id: &str,
    limit: u32,
) -> AppResult<Vec<RunRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, project_id, task, mode, context_manifest, policy, status, budget, started_at_ms, ended_at_ms, validation_state
         FROM runs WHERE project_id = ?1 ORDER BY started_at_ms DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![project_id, limit], |row| {
        let manifest_str: String = row.get(4)?;
        let policy_str: String = row.get(5)?;
        let budget_str: Option<String> = row.get(7)?;
        let val_str: Option<String> = row.get(10)?;
        Ok(RunRecord {
            id: row.get(0)?,
            project_id: row.get(1)?,
            task: row.get(2)?,
            mode: row.get(3)?,
            context_manifest: serde_json::from_str(&manifest_str)
                .unwrap_or(serde_json::Value::Null),
            policy: serde_json::from_str(&policy_str).unwrap_or(serde_json::Value::Null),
            status: row.get(6)?,
            budget: budget_str.and_then(|s| serde_json::from_str(&s).ok()),
            started_at_ms: row.get(8)?,
            ended_at_ms: row.get(9)?,
            validation_state: val_str.and_then(|s| serde_json::from_str(&s).ok()),
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn append_run_event(
    connection: &Connection,
    run_id: &str,
    sequence: i64,
    event_type: &str,
    payload: &serde_json::Value,
) -> AppResult<()> {
    let id = uuid::Uuid::new_v4().to_string();
    connection.execute(
        "INSERT INTO run_events (id, run_id, sequence, event_type, redacted_payload, created_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            id,
            run_id,
            sequence,
            event_type,
            payload.to_string(),
            now_ms() as i64
        ],
    )?;
    Ok(())
}

pub fn list_run_events(connection: &Connection, run_id: &str) -> AppResult<Vec<serde_json::Value>> {
    let mut statement = connection.prepare(
        "SELECT sequence, event_type, redacted_payload, created_at_ms
         FROM run_events WHERE run_id = ?1 ORDER BY sequence ASC",
    )?;
    let rows = statement.query_map(params![run_id], |row| {
        let payload: String = row.get(2)?;
        Ok(serde_json::json!({
            "sequence": row.get::<_, i64>(0)?,
            "eventType": row.get::<_, String>(1)?,
            "payload": serde_json::from_str::<serde_json::Value>(&payload).unwrap_or(serde_json::Value::Null),
            "createdAtMs": row.get::<_, i64>(3)?,
        }))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn save_approval(
    connection: &Connection,
    req: &leanai_core::approval::ApprovalRequest,
) -> AppResult<()> {
    connection.execute(
        "INSERT INTO approvals (id, run_id, capability, scope, decision, approver, decided_at_ms, expires_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
            decision = excluded.decision,
            approver = excluded.approver,
            decided_at_ms = excluded.decided_at_ms",
        params![
            req.id,
            req.run_id,
            format!("{:?}", req.capability).to_lowercase(),
            serde_json::json!({
                "projectRoot": req.project_root,
                "affectedPaths": req.affected_paths,
                "patchHash": req.patch_hash,
                "token": req.token,
            }).to_string(),
            format!("{:?}", req.state).to_lowercase(),
            req.approver.as_deref().unwrap_or(""),
            req.decided_at_ms.map(|d| d as i64).unwrap_or(0),
            req.expires_at_ms as i64,
        ],
    )?;
    Ok(())
}

pub fn resolve_approval_record(
    connection: &Connection,
    id: &str,
    decision: &str,
    approver: &str,
    decided_at_ms: i64,
) -> AppResult<()> {
    connection.execute(
        "UPDATE approvals SET decision = ?2, approver = ?3, decided_at_ms = ?4 WHERE id = ?1",
        params![id, decision, approver, decided_at_ms],
    )?;
    Ok(())
}

pub fn get_approval(
    connection: &Connection,
    id: &str,
) -> AppResult<Option<leanai_core::approval::ApprovalRequest>> {
    let mut statement = connection.prepare(
        "SELECT id, run_id, capability, scope, decision, approver, decided_at_ms, expires_at_ms
         FROM approvals WHERE id = ?1",
    )?;
    let row = statement
        .query_row(params![id], |row| {
            let scope_str: String = row.get(3)?;
            let scope: serde_json::Value =
                serde_json::from_str(&scope_str).unwrap_or(serde_json::Value::Null);
            let cap_str: String = row.get(2)?;
            let capability = if cap_str.contains("write") {
                leanai_core::agent::ToolCapability::WriteFile
            } else {
                leanai_core::agent::ToolCapability::RunAllowlistedCommand
            };
            let decision_str: String = row.get(4)?;
            let state = match decision_str.as_str() {
                "approved" => leanai_core::approval::DecisionState::Approved,
                "denied" => leanai_core::approval::DecisionState::Denied,
                "expired" => leanai_core::approval::DecisionState::Expired,
                _ => leanai_core::approval::DecisionState::Pending,
            };
            Ok(leanai_core::approval::ApprovalRequest {
                id: row.get(0)?,
                run_id: row.get(1)?,
                capability,
                project_root: scope["projectRoot"].as_str().unwrap_or("").to_string(),
                affected_paths: scope["affectedPaths"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                patch_hash: scope["patchHash"].as_str().unwrap_or("").to_string(),
                token: scope["token"].as_str().unwrap_or("").to_string(),
                created_at_ms: 0,
                expires_at_ms: row.get::<_, i64>(7)? as u64,
                state,
                approver: {
                    let s: String = row.get(5)?;
                    if s.is_empty() || s == "pending" {
                        None
                    } else {
                        Some(s)
                    }
                },
                decided_at_ms: {
                    let ms: i64 = row.get(6)?;
                    if ms > 0 {
                        Some(ms as u64)
                    } else {
                        None
                    }
                },
            })
        })
        .optional()?;
    Ok(row)
}

pub fn get_command_allowlist(connection: &Connection, project_id: &str) -> AppResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT command_pattern FROM project_command_allowlists WHERE project_id = ?1 ORDER BY command_pattern ASC",
    )?;
    let rows = statement.query_map(params![project_id], |row| row.get(0))?;
    let list: Vec<String> = rows.collect::<Result<Vec<_>, _>>()?;
    if list.is_empty() {
        // Default safe allowlist for any standard repo (Instructions §8.6, 9.7)
        Ok(vec![
            "cargo test".to_string(),
            "npm test".to_string(),
            "npm run test".to_string(),
            "pytest".to_string(),
            "vitest run".to_string(),
        ])
    } else {
        Ok(list)
    }
}

pub fn set_command_allowlist(
    connection: &Connection,
    project_id: &str,
    commands: &[String],
) -> AppResult<()> {
    connection.execute(
        "DELETE FROM project_command_allowlists WHERE project_id = ?1",
        params![project_id],
    )?;
    for cmd in commands {
        connection.execute(
            "INSERT INTO project_command_allowlists (project_id, command_pattern, created_at_ms)
             VALUES (?1, ?2, ?3)",
            params![project_id, cmd.trim(), now_ms() as i64],
        )?;
    }
    Ok(())
}

pub fn save_project_memory(
    connection: &Connection,
    entry: &leanai_core::retrieval::EpisodicMemoryEntry,
    project_id: &str,
) -> AppResult<()> {
    connection.execute(
        "INSERT INTO project_memory (id, project_id, task_summary, relevant_paths, key_findings, created_at_ms, expires_at_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            entry.id,
            project_id,
            entry.task_summary,
            serde_json::to_string(&entry.relevant_paths).unwrap_or_else(|_| "[]".to_string()),
            entry.key_findings,
            entry.created_at_ms as i64,
            entry.expires_at_ms as i64,
        ],
    )?;
    Ok(())
}

pub fn list_project_memory(
    connection: &Connection,
    project_id: &str,
    limit: u32,
) -> AppResult<Vec<leanai_core::retrieval::EpisodicMemoryEntry>> {
    let now = now_ms() as i64;
    let mut statement = connection.prepare(
        "SELECT id, task_summary, relevant_paths, key_findings, created_at_ms, expires_at_ms
         FROM project_memory WHERE project_id = ?1 AND expires_at_ms > ?2
         ORDER BY created_at_ms DESC LIMIT ?3",
    )?;
    let rows = statement.query_map(params![project_id, now, limit], |row| {
        let paths_str: String = row.get(2)?;
        Ok(leanai_core::retrieval::EpisodicMemoryEntry {
            id: row.get(0)?,
            task_summary: row.get(1)?,
            relevant_paths: serde_json::from_str(&paths_str).unwrap_or_default(),
            key_findings: row.get(3)?,
            created_at_ms: row.get::<_, i64>(4)? as u64,
            expires_at_ms: row.get::<_, i64>(5)? as u64,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}
