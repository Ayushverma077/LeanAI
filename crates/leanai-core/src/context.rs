use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::classify::FileClass;
use crate::error::Result;
use crate::inventory::Inventory;
use crate::project;
use crate::symbols::{self, RouteDeclaration};

/// Version 2 is the short format (ADR 0014). Stored documents with another
/// version are regenerated rather than shown.
pub const SCHEMA_VERSION: u32 = 2;
pub const DOCUMENT_NAME: &str = "PROJECT_CONTEXT.md";
/// First characters of the comment every rendered file carries. A file with it
/// was written by LeanAI; a file without it was not.
pub const FILE_MARKER: &str = "<!-- leanai.context/v";

/// The document's sections, in render order.
///
/// Kept short on purpose (ADR 0014): the file is what a person or a model reads
/// first, so every section is a few plain lines. Each section still records its
/// sources, content hashes, freshness and limitations in the stored document,
/// and the app shows them; they are not repeated in the file.
pub const SECTION_KEYS: &[(&str, &str)] = &[
    ("overview", "Overview"),
    ("structure", "Structure"),
    ("key_modules", "Key Modules"),
    ("api_contracts", "API Routes"),
    ("dependencies", "Dependencies"),
    ("workflows", "How to Run"),
    ("configuration", "Configuration"),
    ("known_issues", "Open TODOs"),
    ("notes_and_decisions", "Decisions"),
];

/// Sections that describe the file set as a whole. A new file can make them
/// wrong without changing any file they cite, so any addition makes them stale.
const STRUCTURAL_SECTIONS: &[&str] = &["overview", "structure"];

/// Most items shown in one rendered list; the rest are counted, not listed.
const LIST_CAP: usize = 12;
/// Most files listed under Key Modules.
const MODULE_CAP: usize = 20;
/// Most exported names shown per key module.
const NAMES_PER_MODULE: usize = 5;

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
    /// Name of the project directory, used as the document title.
    #[serde(default)]
    pub project_name: String,
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
        let structural = STRUCTURAL_SECTIONS.contains(&section.key.as_str());
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
    /// Cap on TODO findings.
    pub max_todos: usize,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            max_analyzed_files: 1_500,
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
        section_overview(&root, inventory, &analysis, revision),
        section_structure(inventory, revision),
        section_key_modules(inventory, &analysis, revision),
        section_api_contracts(inventory, &analysis, revision),
        section_dependencies(&root, inventory, revision),
        section_workflows(&root, inventory, revision),
        section_configuration(inventory, &analysis, revision),
        section_known_issues(inventory, &analysis, options, revision),
        section_notes_and_decisions(&root, inventory, revision),
    ];

    let mut document = ContextDocument {
        schema_version: SCHEMA_VERSION,
        project_fingerprint: inventory.project_fingerprint.clone(),
        project_name: root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
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
    // Markers count only at the start of a comment, so the word "TODO" inside
    // a string, a regex or prose is not reported.
    let todo_pattern = regex::Regex::new(
        r"(?:^|\s)(?://+|#+|/\*+|\*|<!--|--|;+)\s*(TODO|FIXME|HACK|XXX|BUG)\b[:\s-]*(.{0,160})",
    )
    .unwrap();
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

/// `` `a`, `b`, `c` (+N more) ``: at most `cap` items, the rest counted.
fn capped_list<S: AsRef<str>>(items: &[S], cap: usize) -> String {
    let mut out = items
        .iter()
        .take(cap)
        .map(|item| format!("`{}`", item.as_ref()))
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > cap {
        out.push_str(&format!(" (+{} more)", items.len() - cap));
    }
    out
}

fn source(inventory: &Inventory, path: &str) -> Option<SourceRef> {
    inventory
        .get(path)
        .map(|entry| SourceRef::file(path, entry.content_hash.clone()))
}

fn read_text(root: &Path, path: &str) -> String {
    project::resolve_within_root(root, path)
        .ok()
        .and_then(|absolute| std::fs::read_to_string(absolute).ok())
        .unwrap_or_default()
}

/// Test code: left out of Key Modules and API Routes, which describe the
/// product itself.
fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let in_test_dir = lower
        .split('/')
        .rev()
        .skip(1)
        .any(|dir| matches!(dir, "test" | "tests" | "__tests__" | "spec" | "fixtures"));
    in_test_dir
        || name.contains(".test.")
        || name.contains(".spec.")
        || name.starts_with("test_")
        || name.ends_with("_test.go")
        || name.ends_with("_test.py")
        || name.ends_with("_test.rs")
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

const ENTRY_POINTS: &[&str] = &[
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

/// What the project is: the README's first paragraph, languages, entry points
/// and how many files the index covers.
fn section_overview(
    root: &Path,
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let mut refs = Vec::new();
    let mut body = String::new();
    let mut limitations = Vec::new();

    let readme = ["README.md", "README.rst", "README.txt", "readme.md"]
        .into_iter()
        .find(|candidate| inventory.get(candidate).is_some());
    match readme {
        Some(path) => {
            refs.extend(source(inventory, path));
            let text = read_text(root, path);
            // The first prose paragraph: skip headings, badges, HTML and fences.
            let paragraph: Vec<&str> = text
                .lines()
                .map(str::trim)
                .skip_while(|line| {
                    line.is_empty()
                        || line.starts_with('#')
                        || line.starts_with("[![")
                        || line.starts_with('<')
                        || line.starts_with("```")
                })
                .take_while(|line| !line.is_empty())
                .take(4)
                .collect();
            if paragraph.is_empty() {
                limitations.push(format!("`{path}` has no prose paragraph to quote."));
            } else {
                for line in paragraph {
                    body.push_str(&format!("> {line}\n"));
                }
                body.push('\n');
            }
        }
        None => limitations.push("No README was found, so there is no description.".to_string()),
    }

    let mut languages: BTreeMap<&str, usize> = BTreeMap::new();
    for entry in inventory.selectable() {
        *languages
            .entry(symbols::language_of(&entry.path))
            .or_insert(0) += 1;
    }
    let mut ranked: Vec<_> = languages
        .into_iter()
        .filter(|(language, _)| *language != "unknown")
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    if !ranked.is_empty() {
        let mix: Vec<String> = ranked
            .iter()
            .take(6)
            .map(|(language, count)| format!("{language} {count}"))
            .collect();
        body.push_str(&format!("- **Languages:** {}\n", mix.join(" · ")));
    }

    let entry_points: Vec<&str> = ENTRY_POINTS
        .iter()
        .copied()
        .filter(|path| inventory.get(path).is_some())
        .collect();
    body.push_str(&format!(
        "- **Entry points:** {}\n",
        if entry_points.is_empty() {
            "none at the usual paths".to_string()
        } else {
            capped_list(&entry_points, LIST_CAP)
        }
    ));
    refs.extend(
        entry_points
            .iter()
            .filter_map(|path| source(inventory, path)),
    );

    let skipped: Vec<String> = inventory
        .class_counts()
        .into_iter()
        .filter(|(label, _)| label != "text")
        .map(|(label, count)| format!("{count} {label}"))
        .collect();
    body.push_str(&format!(
        "- **Files:** {} used for context, of {} scanned{}\n",
        inventory.selectable().count(),
        inventory.stats.files_seen,
        if skipped.is_empty() {
            String::new()
        } else {
            format!(" (skipped: {})", skipped.join(", "))
        }
    ));

    limitations.push(
        "Entry points are matched by conventional file names, not read from the build configuration."
            .to_string(),
    );
    if inventory.stats.truncated {
        limitations.push(
            "The scan hit its file limit, so the project is only partly covered.".to_string(),
        );
    }
    if analysis.truncated {
        limitations.push("Symbol analysis stopped at the configured file cap.".to_string());
    }
    if !analysis.unanalyzed.is_empty() {
        limitations.push(format!(
            "{} could not be read and are left out.",
            plural(analysis.unanalyzed.len(), "file")
        ));
    }
    ContextSection::deterministic("overview", "Overview", body, refs, limitations, revision)
}

/// One folder in the structure tree, counting files used for context.
#[derive(Default)]
struct Folder {
    total: usize,
    direct: usize,
    children: BTreeMap<String, Folder>,
}

impl Folder {
    /// Follows single-child chains (`crates/` → `crates/leanai-core/`) so a
    /// folder that only wraps another is not shown on its own.
    fn collapse(mut path: String, mut folder: &Folder) -> (String, &Folder) {
        while folder.direct == 0 && folder.children.len() == 1 {
            let Some((name, child)) = folder.children.iter().next() else {
                break;
            };
            path = format!("{path}/{name}");
            folder = child;
        }
        (path, folder)
    }

    fn largest_first(&self) -> Vec<(String, &Folder)> {
        let mut out: Vec<(String, &Folder)> = self
            .children
            .iter()
            .map(|(name, folder)| Self::collapse(name.clone(), folder))
            .collect();
        out.sort_by(|a, b| b.1.total.cmp(&a.1.total).then(a.0.cmp(&b.0)));
        out
    }
}

/// Top-level folders and their main subfolders, with file counts.
fn section_structure(inventory: &Inventory, revision: &str) -> ContextSection {
    let mut tree = Folder::default();
    for entry in inventory.selectable() {
        let mut node = &mut tree;
        node.total += 1;
        let parts: Vec<&str> = entry.path.split('/').collect();
        for dir in &parts[..parts.len() - 1] {
            node = node.children.entry((*dir).to_string()).or_default();
            node.total += 1;
        }
        node.direct += 1;
    }

    let mut body = String::new();
    let top = tree.largest_first();
    for (path, folder) in top.iter().take(25) {
        body.push_str(&format!("- `{path}/` — {}\n", plural(folder.total, "file")));
        let children = folder.largest_first();
        if !children.is_empty() {
            let shown: Vec<String> = children
                .iter()
                .take(8)
                .map(|(name, child)| format!("`{name}/` {}", child.total))
                .collect();
            let more = if children.len() > 8 {
                format!(" (+{} more)", children.len() - 8)
            } else {
                String::new()
            };
            body.push_str(&format!("  - {}{more}\n", shown.join(" · ")));
        }
    }
    if top.len() > 25 {
        body.push_str(&format!("- …and {} more folders\n", top.len() - 25));
    }
    if tree.direct > 0 {
        body.push_str(&format!(
            "- {} at the top level\n",
            plural(tree.direct, "file")
        ));
    }
    if body.is_empty() {
        body.push_str("_No files are eligible for context._\n");
    }

    ContextSection::deterministic(
        "structure",
        "Structure",
        body,
        Vec::new(),
        vec!["Counts only files used for context; binary, generated and excluded files are left out.".to_string()],
        revision,
    )
}

/// The files that export the most names, one line each.
fn section_key_modules(
    inventory: &Inventory,
    analysis: &Analysis,
    revision: &str,
) -> ContextSection {
    let exported = |file: &symbols::FileSymbols| -> Vec<String> {
        file.symbols
            .iter()
            .filter(|symbol| symbol.exported)
            .map(|symbol| symbol.name.clone())
            .collect()
    };
    let mut ranked: Vec<(&symbols::FileSymbols, Vec<String>)> = analysis
        .files
        .iter()
        .filter(|file| !is_test_path(&file.path))
        .map(|file| (file, exported(file)))
        .filter(|(_, names)| !names.is_empty())
        .collect();
    ranked.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.path.cmp(&b.0.path)));

    let mut body = String::new();
    let mut refs = Vec::new();
    for (file, names) in ranked.iter().take(MODULE_CAP) {
        body.push_str(&format!(
            "- `{}` — {}\n",
            file.path,
            capped_list(names, NAMES_PER_MODULE)
        ));
        refs.extend(source(inventory, &file.path));
    }
    if ranked.len() > MODULE_CAP {
        body.push_str(&format!(
            "- …and {} more files that export names\n",
            ranked.len() - MODULE_CAP
        ));
    }
    if body.is_empty() {
        body.push_str("_No exported declarations were found._\n");
    }

    let mut limitations = vec![
        "Ranked by how many names a file exports, which is not the same as importance. Test files are left out.".to_string(),
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
            "No symbol extractor for: {}. Files in those languages are not listed here.",
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
    let routes: Vec<&RouteDeclaration> = analysis
        .routes
        .iter()
        .filter(|route| !is_test_path(&route.path))
        .collect();
    for route in routes.iter().take(25) {
        body.push_str(&format!(
            "- `{} {}` — `{}:{}`\n",
            route.method, route.route, route.path, route.line
        ));
    }
    if routes.len() > 25 {
        body.push_str(&format!("- …and {} more routes\n", routes.len() - 25));
    }
    let route_files: BTreeSet<&str> = routes.iter().map(|route| route.path.as_str()).collect();
    refs.extend(
        route_files
            .into_iter()
            .filter_map(|path| source(inventory, path)),
    );

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
        body.push_str(&format!(
            "- **Interface files:** {}\n",
            capped_list(&schema_files, LIST_CAP)
        ));
        refs.extend(
            schema_files
                .iter()
                .filter_map(|path| source(inventory, path)),
        );
    }
    if body.is_empty() {
        body.push_str("_No HTTP routes or interface files found._\n");
    }

    ContextSection::deterministic(
        "api_contracts",
        "API Routes",
        body,
        refs,
        vec![
            "Route detection covers common Express/Fastify, Flask/FastAPI and axum/actix patterns only. Routes built dynamically, and routes in test files, are not listed.".to_string(),
            "Request and response shapes are not inferred. Open the linked file for the actual contract.".to_string(),
        ],
        revision,
    )
}

/// One line per dependency manifest.
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

    for name in manifests {
        for entry in inventory
            .files
            .iter()
            .filter(|entry| entry.path == name || entry.path.ends_with(&format!("/{name}")))
        {
            refs.push(SourceRef::file(&entry.path, entry.content_hash.clone()));
            let names = parse_dependency_names(name, &read_text(root, &entry.path));
            if names.is_empty() {
                body.push_str(&format!("- `{}`: no dependencies parsed\n", entry.path));
            } else {
                body.push_str(&format!(
                    "- `{}` ({}): {}\n",
                    entry.path,
                    names.len(),
                    capped_list(&names, LIST_CAP)
                ));
            }
        }
    }
    if body.is_empty() {
        body.push_str("_No dependency manifest found._\n");
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

/// Declared scripts and automation: how to build, test and run the project.
fn section_workflows(root: &Path, inventory: &Inventory, revision: &str) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();

    if let Some(entry) = inventory.get("package.json") {
        let text = read_text(root, "package.json");
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(scripts) = value.get("scripts").and_then(|value| value.as_object()) {
                for (name, command) in scripts.iter().take(LIST_CAP) {
                    body.push_str(&format!(
                        "- `npm run {name}` — `{}`\n",
                        command.as_str().unwrap_or("").replace('`', "'")
                    ));
                }
                if scripts.len() > LIST_CAP {
                    body.push_str(&format!(
                        "- …and {} more npm scripts\n",
                        scripts.len() - LIST_CAP
                    ));
                }
                refs.push(SourceRef::file("package.json", entry.content_hash.clone()));
            }
        }
    }

    let automation: Vec<&str> = inventory
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
    if !automation.is_empty() {
        body.push_str(&format!(
            "- **Automation:** {}\n",
            capped_list(&automation, LIST_CAP)
        ));
        refs.extend(automation.iter().filter_map(|path| source(inventory, path)));
    }
    if body.is_empty() {
        body.push_str("_No scripts or automation files found._\n");
    }

    ContextSection::deterministic(
        "workflows",
        "How to Run",
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
    let mut body = String::new();
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
                || name == "dockerfile"
                || name.starts_with("docker-compose")
                || name == ".editorconfig"
                || name == "tauri.conf.json"
                || name.ends_with(".env.example")
        })
        .collect();
    if !config_files.is_empty() {
        body.push_str(&format!(
            "- **Config files:** {}\n",
            capped_list(&config_files, LIST_CAP)
        ));
        refs.extend(
            config_files
                .iter()
                .filter_map(|path| source(inventory, path)),
        );
    }
    if !analysis.env_vars.is_empty() {
        let names: Vec<&str> = analysis.env_vars.iter().map(String::as_str).collect();
        body.push_str(&format!(
            "- **Environment variables:** {}\n\n_Variable names only; LeanAI never reads or records their values._\n",
            capped_list(&names, 20)
        ));
    }
    if body.is_empty() {
        body.push_str("_No configuration files or environment variables found._\n");
    }

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
    options: &GenerateOptions,
    revision: &str,
) -> ContextSection {
    let mut body = String::new();
    for (path, line, note) in analysis.todos.iter().take(LIST_CAP) {
        body.push_str(&format!("- `{path}:{line}` — {note}\n"));
    }
    if analysis.todos.len() > LIST_CAP {
        body.push_str(&format!(
            "- …and {} more{}\n",
            analysis.todos.len() - LIST_CAP,
            if analysis.todos.len() >= options.max_todos {
                " (counting stopped at the limit)"
            } else {
                ""
            }
        ));
    }
    if body.is_empty() {
        body.push_str("_No TODO, FIXME, HACK, XXX or BUG markers found._\n");
    }
    let paths: BTreeSet<&str> = analysis
        .todos
        .iter()
        .map(|(path, _, _)| path.as_str())
        .collect();
    let refs = paths
        .into_iter()
        .take(80)
        .filter_map(|path| source(inventory, path))
        .collect();
    ContextSection::deterministic(
        "known_issues",
        "Open TODOs",
        body,
        refs,
        vec!["Only markers at the start of a comment are counted. An issue tracker is not consulted.".to_string()],
        revision,
    )
}

/// Decision records, each with its own title.
fn section_notes_and_decisions(
    root: &Path,
    inventory: &Inventory,
    revision: &str,
) -> ContextSection {
    let mut body = String::new();
    let mut refs = Vec::new();
    let decision_files: Vec<&str> = inventory
        .selectable()
        .map(|entry| entry.path.as_str())
        .filter(|path| {
            let lower = path.to_ascii_lowercase();
            (lower.contains("/adr/") || lower.contains("decision") || lower.contains("/rfc"))
                && !lower.contains("template")
        })
        .collect();
    for path in decision_files.iter().take(20) {
        let text = read_text(root, path);
        let title = text
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .map(str::trim)
            .filter(|title| !title.is_empty());
        match title {
            Some(title) => body.push_str(&format!("- `{path}` — {title}\n")),
            None => body.push_str(&format!("- `{path}`\n")),
        }
        refs.extend(source(inventory, path));
    }
    if decision_files.len() > 20 {
        body.push_str(&format!(
            "- …and {} more decision files\n",
            decision_files.len() - 20
        ));
    }
    if body.is_empty() {
        body.push_str("_No architecture decision records found._\n");
    }
    ContextSection::deterministic(
        "notes_and_decisions",
        "Decisions",
        body,
        refs,
        vec!["Found by path convention; only each file's title is shown.".to_string()],
        revision,
    )
}

/// Renders the document to `PROJECT_CONTEXT.md`: a title, one revision line,
/// the sections, and a closing note. Provenance stays in the stored document.
pub fn render(document: &ContextDocument) -> String {
    let mut out = String::new();
    if document.project_name.is_empty() {
        out.push_str("# Project Context\n\n");
    } else {
        out.push_str(&format!("# Project Context: {}\n\n", document.project_name));
    }
    out.push_str(&format!(
        "{FILE_MARKER}{} revision={} -->\n\n",
        document.schema_version, document.source_revision
    ));
    out.push_str(&format!(
        "Revision {} · built by LeanAI from the files themselves, not by a model.\n\n",
        describe_revision(&document.source_revision)
    ));

    for (key, title) in SECTION_KEYS {
        let Some(section) = document.section(key) else {
            continue;
        };
        out.push_str(&format!("## {title}\n\n"));
        if section.freshness == Freshness::Stale {
            out.push_str("> **Stale:** files this section was built from have changed. Refresh the map in LeanAI before relying on it.\n\n");
        }
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
    }
    out.push_str("---\n\n_This file is an index, not a replacement for reading the code. Symbols are found by pattern matching, not a full parse. Each section's sources, freshness and limits are shown in LeanAI under Context._\n");
    out
}

/// `git:<sha>[+dirty]` as a short commit plus a plain note; anything else
/// (a content-hash revision) shortened.
fn describe_revision(revision: &str) -> String {
    match revision.strip_prefix("git:") {
        Some(rest) => {
            let (sha, dirty) = match rest.strip_suffix("+dirty") {
                Some(sha) => (sha, true),
                None => (rest, false),
            };
            format!(
                "`{}`{}",
                &sha[..7.min(sha.len())],
                if dirty { " + uncommitted changes" } else { "" }
            )
        }
        None => format!("`{}`", &revision[..20.min(revision.len())]),
    }
}

/// Renders only the requested sections, without provenance lists, within a
/// token ceiling. This is what a model receives as initial project context:
/// the full document is an index to draw from, never something sent whole.
///
/// Returns the rendered text and its local token estimate. Sections past the
/// ceiling are cut at a line boundary and marked, so the model knows to ask
/// for sources instead of assuming the index is complete.
pub fn render_sections(
    document: &ContextDocument,
    keys: &[String],
    max_tokens: usize,
) -> (String, usize) {
    let mut out = String::new();
    let mut used = 0usize;
    for key in keys {
        let Some(section) = document.section(key) else {
            continue;
        };
        let stale = if section.freshness == Freshness::Stale {
            " (stale: verify against source)"
        } else {
            ""
        };
        let heading = format!("## {}{stale}\n\n", section.title);
        let heading_tokens = crate::tokenizer::estimate(&heading).value;
        if used + heading_tokens >= max_tokens {
            break;
        }
        out.push_str(&heading);
        used += heading_tokens;

        let mut truncated = false;
        for line in section.body.trim_end().lines() {
            let line_tokens = crate::tokenizer::estimate(line).value + 1;
            if used + line_tokens > max_tokens {
                truncated = true;
                break;
            }
            out.push_str(line);
            out.push('\n');
            used += line_tokens;
        }
        if truncated {
            out.push_str(
                "… (truncated to fit the context budget; search or read files for detail)\n",
            );
            break;
        }
        out.push('\n');
    }
    let total = crate::tokenizer::estimate(&out).value;
    (out, total)
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
