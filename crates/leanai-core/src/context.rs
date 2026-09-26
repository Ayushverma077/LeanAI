use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::classify::FileClass;
use crate::error::Result;
use crate::inventory::Inventory;
use crate::project;
use crate::symbols::{self, RouteDeclaration, SymbolKind};

pub const SCHEMA_VERSION: u32 = 1;
pub const DOCUMENT_NAME: &str = "PROJECT_CONTEXT.md";

/// The section keys required by Instructions §8.3, in render order.
pub const SECTION_KEYS: &[(&str, &str)] = &[
    ("metadata", "Metadata and freshness"),
    ("quick_reference", "Quick Reference"),
    ("summary", "Project Summary"),
    ("directory_structure", "Directory Structure"),
    ("file_inventory", "File Inventory"),
    ("core_architecture", "Core Architecture"),
    ("key_modules", "Key Modules"),
    ("api_contracts", "API Contracts"),
    ("dependencies", "Dependencies"),
    ("workflows", "Workflows"),
    ("configuration", "Configuration"),
    ("known_issues", "Known Issues and TODOs"),
    ("notes_and_decisions", "Notes and Decisions"),
    ("agent_task_history", "Agent Task History"),
    ("source_references", "Source References and Change Impact"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Every source reference still hashes to the recorded value.
    Fresh,
    /// At least one source changed, or a dependent section changed.
    Stale,
    /// A source could not be checked (deleted, unreadable, never hashed).
    Unknown,
}

/// Who wrote a section. Model-written narrative is always labelled, and must
/// carry provenance before it is accepted (FR-18, backlog 5.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "generator", rename_all = "snake_case")]
pub enum Generator {
    /// Produced by LeanAI code with no model involved.
    Deterministic,
    /// Produced by a model the user explicitly approved.
    Model {
        provider: String,
        model: String,
        prompt_version: String,
    },
}

/// A pointer from a claim back to the code that supports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub path: String,
    /// SHA-256 of the file when the section was generated. `None` when the
    /// file was not hashable, which forces `Freshness::Unknown`.
    pub content_hash: Option<String>,
    /// Optional 1-based line span the claim came from.
    pub lines: Option<(usize, usize)>,
}

impl SourceRef {
    pub fn file(path: &str, hash: Option<String>) -> Self {
        Self {
            path: path.to_string(),
            content_hash: hash,
            lines: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSection {
    pub key: String,
    pub title: String,
    /// Markdown body.
    pub body: String,
    pub source_refs: Vec<SourceRef>,
    pub freshness: Freshness,
    pub generator: Generator,
    /// What this section could not determine. Rendered in the document so a
    /// reader never mistakes silence for absence (FR-18).
    pub limitations: Vec<String>,
    /// Revision the section was generated against.
    pub generated_revision: String,
    pub generated_at_ms: u64,
}

impl ContextSection {
    fn deterministic(
        key: &str,
        title: &str,
        body: String,
        source_refs: Vec<SourceRef>,
        limitations: Vec<String>,
        revision: &str,
    ) -> Self {
        let freshness = if source_refs.iter().any(|r| r.content_hash.is_none()) {
            Freshness::Unknown
        } else {
            Freshness::Fresh
        };
        Self {
            key: key.to_string(),
            title: title.to_string(),
            body,
            source_refs,
            freshness,
            generator: Generator::Deterministic,
            limitations,
            generated_revision: revision.to_string(),
            generated_at_ms: project::now_ms(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDocument {
    pub schema_version: u32,
    pub project_fingerprint: String,
    pub source_revision: String,
    pub generated_at_ms: u64,
    /// SHA-256 of the rendered markdown.
    pub content_hash: String,
    pub sections: Vec<ContextSection>,
}

impl ContextDocument {
    pub fn freshness(&self) -> Freshness {
        if self
            .sections
            .iter()
            .any(|s| s.freshness == Freshness::Stale)
        {
            Freshness::Stale
        } else if self
            .sections
            .iter()
            .any(|s| s.freshness == Freshness::Unknown)
        {
            Freshness::Unknown
        } else {
            Freshness::Fresh
        }
    }

    pub fn section(&self, key: &str) -> Option<&ContextSection> {
        self.sections.iter().find(|section| section.key == key)
    }

    pub fn stale_sections(&self) -> Vec<&ContextSection> {
        self.sections
            .iter()
            .filter(|section| section.freshness != Freshness::Fresh)
            .collect()
    }
}

/// Result of re-checking a document against a fresh inventory (FR-19).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeImpact {
    pub changed_files: Vec<String>,
    pub deleted_files: Vec<String>,
    pub added_files: Vec<String>,
    /// Section keys marked stale by those changes.
    pub stale_sections: Vec<String>,
    /// Section keys marked unknown because a source could not be verified.
    pub unverifiable_sections: Vec<String>,
}

/// Recomputes freshness for every section against the current inventory.
///
/// Conservative on purpose: if impact cannot be proven, the section is marked
/// stale rather than assumed fresh.
pub fn apply_change_impact(document: &mut ContextDocument, inventory: &Inventory) -> ChangeImpact {
    let mut impact = ChangeImpact::default();

    let previous: BTreeMap<&str, Option<&String>> = document
        .sections
        .iter()
        .flat_map(|section| section.source_refs.iter())
        .map(|source| (source.path.as_str(), source.content_hash.as_ref()))
        .collect();

    for (path, hash) in &previous {
        match inventory.get(path) {
            None => impact.deleted_files.push((*path).to_string()),
            Some(entry) => match (hash, &entry.content_hash) {
                (Some(before), Some(now)) if before.as_str() != now.as_str() => {
                    impact.changed_files.push((*path).to_string())
                }
                (_, None) => {}
                _ => {}
            },
        }
    }

    let known: BTreeSet<&str> = previous.keys().copied().collect();
    for entry in inventory.selectable() {
        if !known.contains(entry.path.as_str()) {
            impact.added_files.push(entry.path.clone());
        }
    }

    let changed: BTreeSet<&str> = impact
        .changed_files
        .iter()
        .chain(impact.deleted_files.iter())
        .map(String::as_str)
        .collect();

    for section in &mut document.sections {
        // A new file can invalidate any structural claim, so structural
        // sections go stale when the file set changes at all.
        let structural = matches!(
            section.key.as_str(),
            "directory_structure" | "file_inventory" | "core_architecture" | "source_references"
        );
        let touched = section
            .source_refs
            .iter()
            .any(|source| changed.contains(source.path.as_str()))
            || (structural && !impact.added_files.is_empty());
        let unverifiable = section.source_refs.iter().any(|source| {
            source.content_hash.is_none()
                || inventory
                    .get(&source.path)
                    .is_none_or(|entry| entry.content_hash.is_none())
        });

        section.freshness = if touched {
            impact.stale_sections.push(section.key.clone());
            Freshness::Stale
        } else if unverifiable {
            impact.unverifiable_sections.push(section.key.clone());
            Freshness::Unknown
        } else {
            Freshness::Fresh
        };
    }

    impact.changed_files.sort();
    impact.added_files.sort();
    impact.deleted_files.sort();
    impact
}

#[derive(Debug, Clone)]
pub struct GenerateOptions {
    /// Cap on files read for symbol extraction. Keeps generation bounded on
    /// large repositories.
    pub max_analyzed_files: usize,
    /// Cap on rows in the File Inventory table.
    pub max_inventory_rows: usize,
    /// Cap on TODO findings.
    pub max_todos: usize,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            max_analyzed_files: 1_500,
            max_inventory_rows: 400,
            max_todos: 200,
        }
    }
}

struct Analysis {
    files: Vec<symbols::FileSymbols>,
    routes: Vec<RouteDeclaration>,
    todos: Vec<(String, usize, String)>,
    env_vars: BTreeSet<String>,
    unanalyzed: Vec<String>,
    truncated: bool,
}

/// Generates the deterministic context document. No model is involved
/// (backlog 5.2); model-written sections are added afterwards and labelled.
pub fn generate(
    root: &Path,
    inventory: &Inventory,
    options: &GenerateOptions,
) -> Result<ContextDocument> {
    let root = project::canonical_root(root)?;
    let analysis = analyze(&root, inventory, options);
    let revision = &inventory.source_revision;

    let sections = vec![
        section_metadata(inventory, &analysis, revision),
        section_quick_reference(inventory, &analysis, revision),
        section_summary(&root, inventory, revision),
        section_directory_structure(inventory, revision),
        section_file_inventory(inventory, options, revision),
        section_core_architecture(inventory, &analysis, revision),
        section_key_modules(inventory, &analysis, revision),
        section_api_contracts(inventory, &analysis, revision),
        section_dependencies(&root, inventory, revision),
        section_workflows(&root, inventory, revision),
        section_configuration(inventory, &analysis, revision),
        section_known_issues(inventory, &analysis, revision),
        section_notes_and_decisions(inventory, revision),
        section_agent_task_history(revision),
        section_source_references(inventory, &analysis, revision),
    ];

    let mut document = ContextDocument {
        schema_version: SCHEMA_VERSION,
        project_fingerprint: inventory.project_fingerprint.clone(),
        source_revision: revision.clone(),
        generated_at_ms: project::now_ms(),
        content_hash: String::new(),
        sections,
    };
    document.content_hash = project::sha256_hex(render(&document).as_bytes());
    Ok(document)
}

fn analyze(root: &Path, inventory: &Inventory, options: &GenerateOptions) -> Analysis {
    let mut analysis = Analysis {
        files: Vec::new(),
        routes: Vec::new(),
        todos: Vec::new(),
        env_vars: BTreeSet::new(),
        unanalyzed: Vec::new(),
        truncated: false,
    };
    let todo_pattern =
        regex::Regex::new(r"\b(TODO|FIXME|HACK|XXX|BUG)\b[:\s-]*(.{0,160})").unwrap();
    let env_pattern = regex::Regex::new(
        r#"(?:process\.env\.([A-Z_][A-Z0-9_]*)|process\.env\[['"]([A-Z_][A-Z0-9_]*)['"]\]|os\.environ(?:\.get)?[\[(]\s*['"]([A-Z_][A-Z0-9_]*)['"]|std::env::var\(\s*"([A-Z_][A-Z0-9_]*)"|getenv\(\s*"([A-Z_][A-Z0-9_]*)")"#,
    )
    .unwrap();

    for entry in inventory.selectable() {
        if analysis.files.len() >= options.max_analyzed_files {
            analysis.truncated = true;
            break;
        }
        let Ok(absolute) = project::resolve_within_root(root, &entry.path) else {
            analysis.unanalyzed.push(entry.path.clone());
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&absolute) else {
            analysis.unanalyzed.push(entry.path.clone());
            continue;
        };

        let extracted = symbols::extract(&entry.path, &text);
        analysis
            .routes
            .extend(symbols::extract_routes(&entry.path, &text));

        for (index, line) in text.lines().enumerate() {
            if line.len() > 2_048 {
                continue;
            }
            if analysis.todos.len() < options.max_todos {
                if let Some(captures) = todo_pattern.captures(line) {
                    analysis.todos.push((
                        entry.path.clone(),
                        index + 1,
                        format!(
                            "{}: {}",
                            &captures[1],
                            captures[2].trim().trim_end_matches(['*', '/', '-']).trim()
                        ),
                    ));
                }
            }
            for captures in env_pattern.captures_iter(line) {
                if let Some(name) = captures.iter().skip(1).flatten().next() {
                    analysis.env_vars.insert(name.as_str().to_string());
                }
            }
        }
        analysis.files.push(extracted);
    }
    analysis
        .routes
        .sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));
    analysis
}

fn section_metadata(inventory: &Inventory, analysis: &Analysis, revision: &str) -> ContextSection {
    let counts = inventory.class_counts();
    let mut body = String::new();
    body.push_str(&format!(
        "- **Project fingerprint:** `{}`\n",
        inventory.project_fingerprint
    ));
    body.push_str(&format!("- **Source revision:** `{revision}`\n"));
    body.push_str(&format!("- **Context schema:** `v{SCHEMA_VERSION}`\n"));
    body.push_str(&format!(
        "- **Policy version:** `v{}`\n",
        inventory.policy_version
    ));
    body.push_str(&format!(
        "- **Files scanned:** {}\n",
        inventory.stats.files_seen
    ));
    body.push_str(&format!(
        "- **Files eligible for context:** {}\n",
        inventory.selectable().count()
    ));
    body.push_str(&format!(
        "- **Files analysed for symbols:** {}\n",
        analysis.files.len()
    ));
    body.push_str("\n**Classification breakdown**\n\n| Class | Files |\n| --- | ---: |\n");
    for (label, count) in counts {
        body.push_str(&format!("| {label} | {count} |\n"));
    }
    body.push_str(
        "\n> This document is an index over the source, not a replacement for it. Every claim below links to the file it came from; fetch the current source before relying on a detail.\n",
    );

    let mut limitations = vec![
        "Generated deterministically from file contents. No model reviewed these claims."
            .to_string(),
    ];
    if inventory.stats.truncated {
        limitations.push(
            "The scan hit its file limit, so the project is only partly represented.".to_string(),
        );
    }
    if analysis.truncated {
        limitations.push("Symbol analysis stopped at the configured file cap.".to_string());
    }
    ContextSection::deterministic(
        "metadata",
        "Metadata and freshness",
        body,
        Vec::new(),
        limitations,
        revision,
    )
}

fn section_quick_reference(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut refs = Vec::new();
    let mut body = String::new();

    let entry_candidates = [
        "src/main.rs",
        "src/lib.rs",
        "main.py",
        "src/index.ts",
        "src/index.tsx",
        "src/main.ts",
        "src/main.tsx",
        "src/App.tsx",
        "index.js",
        "app.py",
        "manage.py",
        "cmd/main.go",
        "main.go",
    ];
    let found: Vec<&str> = entry_candidates
        .into_iter()
        .filter(|candidate| inventory.get(candidate).is_some())
        .collect();

    body.push_str("**Likely entry points**\n\n");
    if found.is_empty() {
        body.push_str("_None of the conventional entry-point paths were found. Check the build configuration._\n");
    } else {
        for path in &found {
            body.push_str(&format!("- `{path}`\n"));
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }

    body.push_str("\n**Largest analysed files**\n\n");
    let mut by_size: Vec<_> = inventory.selectable().collect();
    by_size.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes).then(a.path.cmp(&b.path)));
    for entry in by_size.iter().take(5) {
        body.push_str(&format!(
            "- `{}` ({})\n",
            entry.path,
            crate::walker::human_bytes(entry.size_bytes)
        ));
    }

    let exported: usize = analysis
        .files
        .iter()
        .flat_map(|file| file.symbols.iter())
        .filter(|symbol| symbol.exported)
        .count();
    body.push_str(&format!("\n**Exported declarations found:** {exported}\n"));

    ContextSection::deterministic(
        "quick_reference",
        "Quick Reference",
        body,
        refs,
        vec!["Entry points are matched by conventional filename, not by parsing the build configuration.".to_string()],
        revision,
    )
}

fn section_summary(root: &Path, inventory: &Inventory, revision: &str) -> ContextSection {
    let mut refs = Vec::new();
    let mut body = String::new();
    let mut limitations = Vec::new();

    let readme = ["README.md", "README.rst", "README.txt", "readme.md"]
        .into_iter()
        .find(|candidate| inventory.get(candidate).is_some());

    match readme {
        Some(path) => {
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
            let text = project::resolve_within_root(root, path)
                .ok()
                .and_then(|absolute| std::fs::read_to_string(absolute).ok())
                .unwrap_or_default();
            let excerpt: Vec<&str> = text
                .lines()
                .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
                .take(6)
                .collect();
            body.push_str(&format!("_Extracted verbatim from `{path}`._\n\n"));
            if excerpt.is_empty() {
                body.push_str("_The README has no prose paragraphs to quote._\n");
            } else {
                for line in excerpt {
                    body.push_str("> ");
                    body.push_str(line.trim());
                    body.push('\n');
                }
            }
        }
        None => {
            body.push_str("_No README was found, so no project description could be extracted._\n");
            limitations.push("No README to summarise.".to_string());
        }
    }

    let mut languages: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in inventory.selectable() {
        *languages
            .entry(symbols::language_of(&entry.path))
            .or_insert(0) += 1;
    }
    let mut ranked: Vec<_> = languages
        .into_iter()
        .filter(|(lang, _)| *lang != "unknown")
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    body.push_str("\n**Language mix (by file count)**\n\n");
    for (language, count) in ranked.iter().take(8) {
        body.push_str(&format!("- {language}: {count}\n"));
    }

    limitations.push(
        "This section quotes the README rather than describing the code. Narrative summaries require an approved model run.".to_string(),
    );
    ContextSection::deterministic(
        "summary",
        "Project Summary",
        body,
        refs,
        limitations,
        revision,
    )
}

fn section_directory_structure(inventory: &Inventory, revision: &str) -> ContextSection {
    let mut directories: BTreeMap<String, (usize, u64)> = BTreeMap::new();
    for entry in &inventory.files {
        let directory = entry
            .path
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_string())
            .unwrap_or_else(|| ".".to_string());
        let slot = directories.entry(directory).or_insert((0, 0));
        slot.0 += 1;
        slot.1 += entry.size_bytes;
    }

    let mut body = String::from("| Directory | Files | Size |\n| --- | ---: | ---: |\n");
    for (directory, (count, bytes)) in directories.iter().take(200) {
        body.push_str(&format!(
            "| `{directory}` | {count} | {} |\n",
            crate::walker::human_bytes(*bytes)
        ));
    }
    let limitations = if directories.len() > 200 {
        vec![format!(
            "{} directories exist; the table shows the first 200 in path order.",
            directories.len()
        )]
    } else {
        Vec::new()
    };
    ContextSection::deterministic(
        "directory_structure",
        "Directory Structure",
        body,
        Vec::new(),
        limitations,
        revision,
    )
}

fn section_file_inventory(
    inventory: &Inventory,
    options: &GenerateOptions,
    revision: &str,
) -> ContextSection {
    let mut body =
        String::from("| Path | Class | Size | Content hash |\n| --- | --- | ---: | --- |\n");
    let mut shown = 0;
    for entry in &inventory.files {
        if shown >= options.max_inventory_rows {
            break;
        }
        body.push_str(&format!(
            "| `{}` | {} | {} | `{}` |\n",
            entry.path,
            entry.class.label(),
            crate::walker::human_bytes(entry.size_bytes),
            entry
                .content_hash
                .as_deref()
                .map(|hash| &hash[..12.min(hash.len())])
                .unwrap_or("—")
        ));
        shown += 1;
    }
    let mut limitations = Vec::new();
    if inventory.files.len() > shown {
        limitations.push(format!(
            "{} of {} files are listed. The full inventory is available in the app.",
            shown,
            inventory.files.len()
        ));
    }
    if inventory
        .files
        .iter()
        .any(|entry| entry.content_hash.is_none())
    {
        limitations.push("Files shown with `—` were not hashed (binary, oversized or unreadable); their freshness cannot be verified by content.".to_string());
    }
    ContextSection::deterministic(
        "file_inventory",
        "File Inventory",
        body,
        Vec::new(),
        limitations,
        revision,
    )
}

fn section_core_architecture(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    // Cluster by top-level directory and count cross-cluster imports. This is
    // a structural observation, not a design description.
    let mut clusters: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for file in &analysis.files {
        let cluster = file
            .path
            .split_once('/')
            .map(|(head, _)| head.to_string())
            .unwrap_or_else(|| "(root)".to_string());
        let slot = clusters.entry(cluster).or_insert((0, 0));
        slot.0 += 1;
        slot.1 += file.symbols.len();
    }

    let mut body = String::from(
        "Observed structure, derived from directory layout and declaration counts.\n\n| Area | Files analysed | Declarations |\n| --- | ---: | ---: |\n",
    );
    for (cluster, (files, declarations)) in &clusters {
        body.push_str(&format!("| `{cluster}/` | {files} | {declarations} |\n"));
    }

    let mut edges: BTreeMap<(String, String), usize> = BTreeMap::new();
    let cluster_names: BTreeSet<&String> = clusters.keys().collect();
    for file in &analysis.files {
        let from = file
            .path
            .split_once('/')
            .map(|(head, _)| head.to_string())
            .unwrap_or_else(|| "(root)".to_string());
        for import in &file.imports {
            let target = import.trim_start_matches("./").trim_start_matches("../");
            let head = target
                .split(['/', ':', '.'])
                .next()
                .unwrap_or("")
                .to_string();
            if head.is_empty() || head == from || !cluster_names.contains(&head) {
                continue;
            }
            *edges.entry((from.clone(), head)).or_insert(0) += 1;
        }
    }
    if !edges.is_empty() {
        body.push_str("\n**Import edges between areas**\n\n");
        let mut ranked: Vec<_> = edges.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        for ((from, to), count) in ranked.into_iter().take(25) {
            body.push_str(&format!("- `{from}` → `{to}` ({count} imports)\n"));
        }
    }

    let refs = analysis
        .files
        .iter()
        .filter_map(|file| {
            inventory
                .get(&file.path)
                .map(|entry| SourceRef::file(&file.path, entry.content_hash.clone()))
        })
        .take(50)
        .collect();

    ContextSection::deterministic(
        "core_architecture",
        "Core Architecture",
        body,
        refs,
        vec![
            "Derived from import statements and directory layout only. It describes how the code is arranged, not why.".to_string(),
            "Dynamic imports, dependency injection and runtime wiring are invisible to this analysis.".to_string(),
        ],
        revision,
    )
}

fn section_key_modules(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut ranked: Vec<&symbols::FileSymbols> = analysis
        .files
        .iter()
        .filter(|file| !file.symbols.is_empty())
        .collect();
    ranked.sort_by(|a, b| {
        b.symbols
            .iter()
            .filter(|symbol| symbol.exported)
            .count()
            .cmp(&a.symbols.iter().filter(|symbol| symbol.exported).count())
            .then(a.path.cmp(&b.path))
    });

    let mut body = String::new();
    let mut refs = Vec::new();
    for file in ranked.iter().take(30) {
        body.push_str(&format!("### `{}`\n\n", file.path));
        for symbol in file
            .symbols
            .iter()
            .filter(|symbol| symbol.exported)
            .take(20)
        {
            body.push_str(&format!(
                "- `{}` — {} (`{}:{}`)\n",
                symbol.name,
                kind_label(symbol.kind),
                file.path,
                symbol.line
            ));
        }
        body.push('\n');
        if let Some(entry) = inventory.get(&file.path) {
            refs.push(SourceRef::file(&file.path, entry.content_hash.clone()));
        }
    }
    if body.is_empty() {
        body.push_str("_No exported declarations were extracted._\n");
    }

    let mut limitations = vec![
        "Declarations are found by line-oriented pattern matching, not by a parser. Macros, generated code and unusual formatting can be missed.".to_string(),
    ];
    let unsupported: BTreeSet<&str> = analysis
        .files
        .iter()
        .filter(|file| file.analysis_unavailable.is_some())
        .map(|file| file.language.as_str())
        .collect();
    if !unsupported.is_empty() {
        limitations.push(format!(
            "No symbol extractor for: {}. Files in those languages appear at file level only.",
            unsupported.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }
    ContextSection::deterministic(
        "key_modules",
        "Key Modules",
        body,
        refs,
        limitations,
        revision,
    )
}

fn section_api_contracts(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();
    if analysis.routes.is_empty() {
        body.push_str("_No HTTP route declarations matched the supported framework patterns._\n");
    } else {
        body.push_str("| Method | Route | Declared in |\n| --- | --- | --- |\n");
        for route in analysis.routes.iter().take(150) {
            body.push_str(&format!(
                "| {} | `{}` | `{}:{}` |\n",
                route.method, route.route, route.path, route.line
            ));
        }
        let paths: BTreeSet<&str> = analysis
            .routes
            .iter()
            .map(|route| route.path.as_str())
            .collect();
        for path in paths {
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }

    let schema_files: Vec<&str> = inventory
        .selectable()
        .map(|entry| entry.path.as_str())
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.ends_with(".proto")
                || lower.ends_with(".graphql")
                || lower.ends_with(".gql")
                || lower.contains("openapi")
                || lower.contains("swagger")
        })
        .collect();
    if !schema_files.is_empty() {
        body.push_str("\n**Interface definition files**\n\n");
        for path in &schema_files {
            body.push_str(&format!("- `{path}`\n"));
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }

    ContextSection::deterministic(
        "api_contracts",
        "API Contracts",
        body,
        refs,
        vec![
            "Route detection covers common Express/Fastify, Flask/FastAPI and axum/actix patterns only. Routes built dynamically are not listed.".to_string(),
            "Request and response shapes are not inferred. Open the linked file for the actual contract.".to_string(),
        ],
        revision,
    )
}

fn section_dependencies(root: &Path, inventory: &Inventory, revision: &str) -> ContextSection {
    let manifests = [
        "package.json",
        "Cargo.toml",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "Gemfile",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "composer.json",
    ];
    let mut body = String::new();
    let mut refs = Vec::new();
    let mut found = false;

    for name in manifests {
        for entry in inventory
            .files
            .iter()
            .filter(|entry| entry.path == name || entry.path.ends_with(&format!("/{name}")))
        {
            found = true;
            refs.push(SourceRef::file(&entry.path, entry.content_hash.clone()));
            body.push_str(&format!("### `{}`\n\n", entry.path));
            let text = project::resolve_within_root(root, &entry.path)
                .ok()
                .and_then(|absolute| std::fs::read_to_string(absolute).ok())
                .unwrap_or_default();
            let names = parse_dependency_names(name, &text);
            if names.is_empty() {
                body.push_str("_No dependency entries were parsed from this manifest._\n\n");
            } else {
                body.push_str(&format!("{} declared dependencies:\n\n", names.len()));
                for dependency in names.iter().take(80) {
                    body.push_str(&format!("- `{dependency}`\n"));
                }
                if names.len() > 80 {
                    body.push_str(&format!("- _…and {} more_\n", names.len() - 80));
                }
                body.push('\n');
            }
        }
    }
    if !found {
        body.push_str("_No recognised dependency manifest was found._\n");
    }

    ContextSection::deterministic(
        "dependencies",
        "Dependencies",
        body,
        refs,
        vec![
            "Names are read from manifests, not from a resolved lockfile, so versions in use may differ.".to_string(),
            "Transitive dependencies are not listed.".to_string(),
        ],
        revision,
    )
}

/// Extracts dependency names without a full parser for each format. Returns an
/// empty list rather than a guess when the format is not understood.
fn parse_dependency_names(manifest: &str, text: &str) -> Vec<String> {
    let mut names = BTreeSet::new();
    match manifest {
        "package.json" | "composer.json" => {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                for key in [
                    "dependencies",
                    "devDependencies",
                    "peerDependencies",
                    "require",
                    "require-dev",
                ] {
                    if let Some(map) = value.get(key).and_then(|value| value.as_object()) {
                        names.extend(map.keys().cloned());
                    }
                }
            }
        }
        "Cargo.toml" | "pyproject.toml" => {
            let mut in_dependencies = false;
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with('[') {
                    in_dependencies = trimmed.contains("dependencies");
                    continue;
                }
                if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((name, _)) = trimmed.split_once('=') {
                    let name = name.trim().trim_matches('"');
                    if !name.is_empty() {
                        names.insert(name.to_string());
                    }
                }
            }
        }
        "requirements.txt" => {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                    continue;
                }
                let name = trimmed
                    .split(['=', '<', '>', '!', '~', '[', ';', ' '])
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
        }
        "go.mod" => {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("require ") {
                    if let Some(rest) = trimmed.strip_prefix("require ") {
                        if let Some(name) = rest.split_whitespace().next() {
                            if name != "(" {
                                names.insert(name.to_string());
                            }
                        }
                    }
                } else if trimmed.contains('/') && trimmed.contains(" v") {
                    if let Some(name) = trimmed.split_whitespace().next() {
                        names.insert(name.to_string());
                    }
                }
            }
        }
        _ => {}
    }
    names.into_iter().collect()
}

fn section_workflows(root: &Path, inventory: &Inventory, revision: &str) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();

    let ci_files: Vec<&str> = inventory
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .filter(|path| {
            path.starts_with(".github/workflows/")
                || path.ends_with(".gitlab-ci.yml")
                || path.ends_with("azure-pipelines.yml")
                || path.starts_with(".circleci/")
                || *path == "Makefile"
                || *path == "justfile"
                || *path == "Taskfile.yml"
        })
        .collect();

    body.push_str("**Automation and task files**\n\n");
    if ci_files.is_empty() {
        body.push_str("_None found._\n");
    } else {
        for path in &ci_files {
            body.push_str(&format!("- `{path}`\n"));
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }

    if let Some(entry) = inventory.get("package.json") {
        let text = project::resolve_within_root(root, "package.json")
            .ok()
            .and_then(|absolute| std::fs::read_to_string(absolute).ok())
            .unwrap_or_default();
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) {
                body.push_str("\n**npm scripts**\n\n| Script | Command |\n| --- | --- |\n");
                for (name, command) in scripts {
                    body.push_str(&format!(
                        "| `{}` | `{}` |\n",
                        name,
                        command.as_str().unwrap_or("").replace('|', "\\|")
                    ));
                }
                refs.push(SourceRef::file("package.json", entry.content_hash.clone()));
            }
        }
    }

    ContextSection::deterministic(
        "workflows",
        "Workflows",
        body,
        refs,
        vec!["Lists declared automation only. Whether these workflows currently pass is not checked here.".to_string()],
        revision,
    )
}

fn section_configuration(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut body = String::from("**Configuration files**\n\n");
    let mut refs = Vec::new();
    let config_files: Vec<&str> = inventory
        .files
        .iter()
        .map(|entry| entry.path.as_str())
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
            name.ends_with(".config.js")
                || name.ends_with(".config.ts")
                || name.ends_with(".config.mjs")
                || name.starts_with("tsconfig")
                || name == "vite.config.ts"
                || name == "dockerfile"
                || name.starts_with("docker-compose")
                || name == ".editorconfig"
                || name == "tauri.conf.json"
                || name.ends_with(".env.example")
        })
        .take(100)
        .collect();
    if config_files.is_empty() {
        body.push_str("_None found._\n");
    } else {
        for path in &config_files {
            body.push_str(&format!("- `{path}`\n"));
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }

    body.push_str("\n**Environment variables referenced in code**\n\n");
    if analysis.env_vars.is_empty() {
        body.push_str("_None detected._\n");
    } else {
        for name in analysis.env_vars.iter().take(120) {
            body.push_str(&format!("- `{name}`\n"));
        }
    }
    body.push_str(
        "\n> Variable **names** are listed. LeanAI never reads or records their values.\n",
    );

    ContextSection::deterministic(
        "configuration",
        "Configuration",
        body,
        refs,
        vec!["Only direct `process.env` / `os.environ` / `std::env::var` style accesses are detected. Indirect configuration loading is not.".to_string()],
        revision,
    )
}

fn section_known_issues(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();
    if analysis.todos.is_empty() {
        body.push_str("_No TODO, FIXME, HACK, XXX or BUG markers were found._\n");
    } else {
        body.push_str("| Location | Note |\n| --- | --- |\n");
        for (path, line, note) in &analysis.todos {
            body.push_str(&format!(
                "| `{}:{}` | {} |\n",
                path,
                line,
                note.replace('|', "\\|")
            ));
        }
        let paths: BTreeSet<&str> = analysis
            .todos
            .iter()
            .map(|(path, _, _)| path.as_str())
            .collect();
        for path in paths.into_iter().take(80) {
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }
    ContextSection::deterministic(
        "known_issues",
        "Known Issues and TODOs",
        body,
        refs,
        vec!["Source-code markers only. An issue tracker is not consulted.".to_string()],
        revision,
    )
}

fn section_notes_and_decisions(inventory: &Inventory, revision: &str) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();
    let decision_files: Vec<&str> = inventory
        .selectable()
        .map(|entry| entry.path.as_str())
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            lower.contains("/adr/") || lower.contains("decision") || lower.contains("/rfc")
        })
        .take(100)
        .collect();
    if decision_files.is_empty() {
        body.push_str("_No architecture decision records were found._\n");
    } else {
        for path in &decision_files {
            body.push_str(&format!("- `{path}`\n"));
            if let Some(entry) = inventory.get(path) {
                refs.push(SourceRef::file(path, entry.content_hash.clone()));
            }
        }
    }
    ContextSection::deterministic(
        "notes_and_decisions",
        "Notes and Decisions",
        body,
        refs,
        vec!["Files are listed by path convention; their contents are not summarised.".to_string()],
        revision,
    )
}

fn section_agent_task_history(revision: &str) -> ContextSection {
    ContextSection::deterministic(
        "agent_task_history",
        "Agent Task History",
        "_No agent runs have been recorded for this project._\n\nCompleted runs append a concise, source-cited finding here. Full transcripts are not stored by default.\n".to_string(),
        Vec::new(),
        Vec::new(),
        revision,
    )
}

fn section_source_references(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut body = String::from(
        "Every section above records the files it was generated from, together with the content hash at generation time. When a hash no longer matches, the section is marked **stale** and the claim must be re-verified against the current source.\n\n",
    );
    body.push_str(&format!(
        "- Files hashed at generation: {}\n",
        inventory
            .files
            .iter()
            .filter(|entry| entry.content_hash.is_some())
            .count()
    ));
    body.push_str(&format!(
        "- Files without a content hash (freshness unverifiable): {}\n",
        inventory
            .files
            .iter()
            .filter(|entry| entry.content_hash.is_none())
            .count()
    ));
    if !analysis.unanalyzed.is_empty() {
        body.push_str(&format!(
            "- Files that could not be read during analysis: {}\n",
            analysis.unanalyzed.len()
        ));
    }
    body.push_str("\n**Source-on-demand**\n\nAsk LeanAI for `<path>` at the recorded revision to retrieve the exact bytes a claim was based on, rather than trusting the summary.\n");

    ContextSection::deterministic(
        "source_references",
        "Source References and Change Impact",
        body,
        Vec::new(),
        Vec::new(),
        revision,
    )
}

fn kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Type => "type",
        SymbolKind::Constant => "constant",
        SymbolKind::Module => "module",
        SymbolKind::Route => "route",
    }
}

/// Renders the document to `PROJECT_CONTEXT.md`.
pub fn render(document: &ContextDocument) -> String {
    let mut out = String::new();
    out.push_str("# Project Context\n\n");
    out.push_str(&format!(
        "<!-- leanai.context/v{} revision={} -->\n\n",
        document.schema_version, document.source_revision
    ));
    out.push_str("> Generated by LeanAI Desktop. This is an index over the source with provenance, **not** a replacement for reading the code.\n\n");

    for (key, title) in SECTION_KEYS {
        let Some(section) = document.section(key) else {
            continue;
        };
        out.push_str(&format!("## {title}\n\n"));
        out.push_str(&format!(
            "`{}` · generated at revision `{}` · source refs: {}\n\n",
            freshness_badge(section.freshness),
            section.generated_revision,
            section.source_refs.len()
        ));
        if let Generator::Model {
            provider,
            model,
            prompt_version,
        } = &section.generator
        {
            out.push_str(&format!(
                "> Written by `{provider}/{model}` (prompt `{prompt_version}`) and accepted after provenance validation.\n\n"
            ));
        }
        out.push_str(section.body.trim_end());
        out.push_str("\n\n");
        if !section.limitations.is_empty() {
            out.push_str("**Limitations**\n\n");
            for limitation in &section.limitations {
                out.push_str(&format!("- {limitation}\n"));
            }
            out.push('\n');
        }
        if !section.source_refs.is_empty() {
            out.push_str("<details><summary>Sources</summary>\n\n");
            for source in &section.source_refs {
                out.push_str(&format!(
                    "- `{}` @ `{}`\n",
                    source.path,
                    source
                        .content_hash
                        .as_deref()
                        .map(|hash| &hash[..12.min(hash.len())])
                        .unwrap_or("unhashed")
                ));
            }
            out.push_str("\n</details>\n\n");
        }
    }
    out
}

fn freshness_badge(freshness: Freshness) -> &'static str {
    match freshness {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "STALE — re-verify against source",
        Freshness::Unknown => "unknown freshness",
    }
}

/// Validates a model-written section before it is accepted (backlog 5.6).
///
/// A section is rejected unless every source it cites exists in the current
/// inventory and hashes to the recorded value.
pub fn validate_model_section(
    section: &ContextSection,
    inventory: &Inventory,
) -> std::result::Result<(), Vec<String>> {
    let mut problems = Vec::new();
    if matches!(section.generator, Generator::Deterministic) {
        problems.push(
            "section is labelled deterministic but was submitted for model validation".to_string(),
        );
    }
    if section.source_refs.is_empty() {
        problems.push("model-written sections must cite at least one source file".to_string());
    }
    for source in &section.source_refs {
        match inventory.get(&source.path) {
            None => problems.push(format!(
                "cited file `{}` is not in the project",
                source.path
            )),
            Some(entry) => {
                if entry.class != FileClass::SourceText {
                    problems.push(format!(
                        "cited file `{}` is not selectable source text",
                        source.path
                    ));
                }
                match (&source.content_hash, &entry.content_hash) {
                    (Some(cited), Some(actual)) if cited != actual => problems.push(format!(
                        "cited file `{}` changed since the section was written",
                        source.path
                    )),
                    (None, _) => problems.push(format!(
                        "citation for `{}` has no content hash",
                        source.path
                    )),
                    _ => {}
                }
            }
        }
    }
    if section.body.trim().is_empty() {
        problems.push("section body is empty".to_string());
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// Stable hash of a document's semantic content, used to detect real changes
/// between regenerations (timestamps excluded).
pub fn semantic_hash(document: &ContextDocument) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"leanai.context.semantic/v1");
    for section in &document.sections {
        hasher.update(section.key.as_bytes());
        hasher.update(section.body.as_bytes());
        for source in &section.source_refs {
            hasher.update(source.path.as_bytes());
            if let Some(hash) = &source.content_hash {
                hasher.update(hash.as_bytes());
            }
        }
    }
    project::hex(&hasher.finalize())
}
