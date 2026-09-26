use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Kind of a declaration found in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Type,
    Constant,
    Module,
    Route,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    /// True when the declaration is exported/public in its language.
    pub exported: bool,
}

/// What was extracted from one file, including an explicit statement of what
/// the extractor could *not* do (FR-18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSymbols {
    pub path: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<String>,
    /// Set when the language has no extractor, so the caller can fall back to
    /// file-level context instead of silently reporting "no symbols".
    pub analysis_unavailable: Option<String>,
}

struct Pattern {
    kind: SymbolKind,
    exported_group: bool,
    regex: Regex,
}

fn pattern(kind: SymbolKind, exported_group: bool, source: &str) -> Option<Pattern> {
    Regex::new(source).ok().map(|regex| Pattern {
        kind,
        exported_group,
        regex,
    })
}

/// Line-oriented declaration patterns.
///
/// This is deliberately a lexical extractor, not a parser: it runs on every
/// supported language without a per-language toolchain, and it reports its own
/// limits rather than pretending to be an AST. Anything it cannot classify
/// falls back to file-level context.
static RUST: Lazy<Vec<Pattern>> = Lazy::new(|| {
    [
        pattern(
            SymbolKind::Function,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)",
        ),
        pattern(
            SymbolKind::Struct,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?struct\s+([A-Za-z_][A-Za-z0-9_]*)",
        ),
        pattern(
            SymbolKind::Enum,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?enum\s+([A-Za-z_][A-Za-z0-9_]*)",
        ),
        pattern(
            SymbolKind::Trait,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?trait\s+([A-Za-z_][A-Za-z0-9_]*)",
        ),
        pattern(
            SymbolKind::Type,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?type\s+([A-Za-z_][A-Za-z0-9_]*)",
        ),
        pattern(
            SymbolKind::Constant,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?(?:const|static)\s+([A-Z_][A-Z0-9_]*)",
        ),
        pattern(
            SymbolKind::Module,
            true,
            r"^\s*(pub(?:\([^)]*\))?\s+)?mod\s+([a-z_][a-z0-9_]*)",
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static TS: Lazy<Vec<Pattern>> = Lazy::new(|| {
    [
        pattern(
            SymbolKind::Function,
            true,
            r"^\s*(export\s+)?(?:default\s+)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)",
        ),
        pattern(
            SymbolKind::Function,
            true,
            r"^\s*(export\s+)?const\s+([A-Za-z_$][\w$]*)\s*(?::[^=]+)?=\s*(?:async\s*)?\(",
        ),
        pattern(
            SymbolKind::Class,
            true,
            r"^\s*(export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z_$][\w$]*)",
        ),
        pattern(
            SymbolKind::Interface,
            true,
            r"^\s*(export\s+)?interface\s+([A-Za-z_$][\w$]*)",
        ),
        pattern(
            SymbolKind::Type,
            true,
            r"^\s*(export\s+)?type\s+([A-Za-z_$][\w$]*)",
        ),
        pattern(
            SymbolKind::Enum,
            true,
            r"^\s*(export\s+)?enum\s+([A-Za-z_$][\w$]*)",
        ),
        pattern(
            SymbolKind::Constant,
            true,
            r"^\s*(export\s+)?const\s+([A-Z][A-Z0-9_]*)\s*[:=]",
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static PYTHON: Lazy<Vec<Pattern>> = Lazy::new(|| {
    [
        pattern(
            SymbolKind::Function,
            false,
            r"^(?:async\s+)?def\s+([A-Za-z_][\w]*)",
        ),
        pattern(SymbolKind::Class, false, r"^class\s+([A-Za-z_][\w]*)"),
        pattern(SymbolKind::Constant, false, r"^([A-Z][A-Z0-9_]*)\s*[:=]"),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static GO: Lazy<Vec<Pattern>> = Lazy::new(|| {
    [
        pattern(
            SymbolKind::Function,
            false,
            r"^func\s+(?:\([^)]*\)\s*)?([A-Za-z_][\w]*)",
        ),
        pattern(
            SymbolKind::Struct,
            false,
            r"^type\s+([A-Za-z_][\w]*)\s+struct",
        ),
        pattern(
            SymbolKind::Interface,
            false,
            r"^type\s+([A-Za-z_][\w]*)\s+interface",
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static JAVA_LIKE: Lazy<Vec<Pattern>> = Lazy::new(|| {
    [
        pattern(
            SymbolKind::Class,
            true,
            r"^\s*(public\s+)?(?:final\s+|abstract\s+|static\s+)*class\s+([A-Za-z_][\w]*)",
        ),
        pattern(
            SymbolKind::Interface,
            true,
            r"^\s*(public\s+)?interface\s+([A-Za-z_][\w]*)",
        ),
        pattern(
            SymbolKind::Enum,
            true,
            r"^\s*(public\s+)?enum\s+([A-Za-z_][\w]*)",
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
});

static IMPORT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r#"^\s*import\s+(?:.+\s+from\s+)?['"]([^'"]+)['"]"#,
        r#"^\s*(?:const|let|var)\s+.+=\s*require\(['"]([^'"]+)['"]\)"#,
        r#"^\s*from\s+([\w.]+)\s+import\s"#,
        r#"^\s*import\s+([\w.]+)"#,
        r#"^\s*use\s+([A-Za-z_][\w:]*)"#,
    ]
    .into_iter()
    .filter_map(|source| Regex::new(source).ok())
    .collect()
});

/// HTTP route declarations for the API Contracts section.
static ROUTE_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        // express / fastify / koa-router: app.get("/path", ...)
        r#"(?i)\.(get|post|put|patch|delete|head|options)\s*\(\s*['"`]([^'"`]+)['"`]"#,
        // FastAPI / Flask decorators: @app.get("/path")
        r#"(?i)@\w+\.(get|post|put|patch|delete|route)\s*\(\s*['"]([^'"]+)['"]"#,
        // axum / actix: .route("/path", get(handler))
        r#"\.route\s*\(\s*"([^"]+)"\s*,\s*(get|post|put|patch|delete)"#,
    ]
    .into_iter()
    .filter_map(|source| Regex::new(source).ok())
    .collect()
});

pub fn language_of(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("") {
        "rs" => "rust",
        "ts" | "tsx" | "mts" | "cts" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "py" | "pyi" => "python",
        "go" => "go",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "md" | "markdown" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sql" => "sql",
        "sh" | "bash" | "zsh" => "shell",
        _ => "unknown",
    }
}

/// Extracts declarations, imports and route strings from one file's text.
pub fn extract(path: &str, text: &str) -> FileSymbols {
    let language = language_of(path);
    let patterns: Option<&Vec<Pattern>> = match language {
        "rust" => Some(&RUST),
        "typescript" | "javascript" => Some(&TS),
        "python" => Some(&PYTHON),
        "go" => Some(&GO),
        "java" | "kotlin" | "csharp" => Some(&JAVA_LIKE),
        _ => None,
    };

    let mut symbols = Vec::new();
    let mut imports = Vec::new();

    for (index, line) in text.lines().enumerate() {
        if line.len() > 2_048 {
            continue;
        }
        for regex in IMPORT_PATTERNS.iter() {
            if let Some(captures) = regex.captures(line) {
                if let Some(module) = captures.get(1) {
                    imports.push(module.as_str().to_string());
                }
                break;
            }
        }
        if let Some(patterns) = patterns {
            for pattern in patterns {
                let Some(captures) = pattern.regex.captures(line) else {
                    continue;
                };
                let (exported, name) = if pattern.exported_group {
                    (captures.get(1).is_some(), captures.get(2))
                } else {
                    // Go exports by capitalisation; Python has no export marker.
                    let name = captures.get(1);
                    let exported = match language {
                        "go" => name
                            .map(|m| m.as_str().starts_with(|c: char| c.is_uppercase()))
                            .unwrap_or(false),
                        "python" => name.map(|m| !m.as_str().starts_with('_')).unwrap_or(false),
                        _ => true,
                    };
                    (exported, name)
                };
                if let Some(name) = name {
                    symbols.push(Symbol {
                        name: name.as_str().to_string(),
                        kind: pattern.kind,
                        line: index + 1,
                        exported,
                    });
                    break;
                }
            }
        }
    }

    imports.sort();
    imports.dedup();

    FileSymbols {
        path: path.to_string(),
        language: language.to_string(),
        symbols,
        imports,
        analysis_unavailable: patterns.is_none().then(|| {
            format!(
                "No symbol extractor for `{language}` files. This file is represented at file level only."
            )
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteDeclaration {
    pub path: String,
    pub method: String,
    pub route: String,
    pub line: usize,
}

/// Extracts HTTP route declarations. Framework coverage is partial by design
/// and the context document says so.
pub fn extract_routes(path: &str, text: &str) -> Vec<RouteDeclaration> {
    let mut routes = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.len() > 2_048 {
            continue;
        }
        for regex in ROUTE_PATTERNS.iter() {
            let Some(captures) = regex.captures(line) else {
                continue;
            };
            let (Some(first), Some(second)) = (captures.get(1), captures.get(2)) else {
                continue;
            };
            // The axum pattern captures (route, method); the others (method, route).
            let (method, route) = if first.as_str().starts_with('/') {
                (second.as_str(), first.as_str())
            } else {
                (first.as_str(), second.as_str())
            };
            if !route.starts_with('/') {
                continue;
            }
            routes.push(RouteDeclaration {
                path: path.to_string(),
                method: method.to_uppercase(),
                route: route.to_string(),
                line: index + 1,
            });
            break;
        }
    }
    routes
}
