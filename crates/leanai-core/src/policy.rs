use serde::{Deserialize, Serialize};

/// Version of the LeanAI safety policy. Persisted with every project and bundle
/// so a stored selection can be revalidated against the policy that produced it.
pub const POLICY_VERSION: u32 = 1;

/// Hard limits. Every limit has a user-visible reason (`LIMIT_REASONS`) and an
/// override policy, per NFR 7.4.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Limits {
    /// Files larger than this are classified `TooLarge` and are not selectable
    /// without an explicit override.
    pub max_file_bytes: u64,
    /// Separate, much lower ceiling for structured *data* files (JSON, YAML,
    /// XML, CSV…). A 300 KB schema or fixture is thousands of tokens of almost
    /// pure noise, and it passes a code-sized limit easily.
    ///
    /// Defaulted on deserialize so settings written before this field existed
    /// still load; without it serde would reject the old JSON and every stored
    /// preference would silently reset.
    #[serde(default = "default_max_data_file_bytes")]
    pub max_data_file_bytes: u64,
    /// Upper bound on files returned by a single scan.
    pub max_files_scanned: usize,
    /// Upper bound on files in one bundle.
    pub max_selected_files: usize,
    /// Upper bound on bundle output size.
    pub max_bundle_bytes: u64,
    /// Bytes read from the head of a file when probing for binary content.
    pub binary_probe_bytes: usize,
    /// Maximum directory depth walked below the project root.
    pub max_depth: usize,
}

fn default_max_data_file_bytes() -> u64 {
    65_536
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 1_048_576,
            max_data_file_bytes: 65_536,
            max_files_scanned: 200_000,
            max_selected_files: 5_000,
            max_bundle_bytes: 64 * 1_048_576,
            binary_probe_bytes: 8_192,
            max_depth: 64,
        }
    }
}

/// User-visible justification for each limit, surfaced in Settings.
pub const LIMIT_REASONS: &[(&str, &str)] = &[
    (
        "max_file_bytes",
        "Very large files are usually generated or vendored and dominate a context budget. Override per file after reviewing it.",
    ),
    (
        "max_data_file_bytes",
        "Large JSON/YAML/XML/CSV files are data, not context: they are extremely token-dense and rarely help a model. Override per file if you really need one.",
    ),
    (
        "max_files_scanned",
        "Bounds memory and keeps the UI responsive on monorepos. Narrow the project root or add ignore rules.",
    ),
    (
        "max_selected_files",
        "A selection this large will not fit any model context window and makes review impossible.",
    ),
    (
        "max_bundle_bytes",
        "Protects the clipboard, editor and provider request size.",
    ),
    (
        "binary_probe_bytes",
        "LeanAI never reads a whole file just to decide whether it is binary.",
    ),
    (
        "max_depth",
        "Guards against symlink cycles and pathological directory trees.",
    ),
];

/// Ignore-rule precedence. Documented in `docs/adr/0003-ignore-precedence.md`
/// and shown to the user in the exclusions panel.
///
/// Later entries win over earlier ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IgnoreSource {
    /// Built-in LeanAI safety filters (binaries, secrets, oversized files).
    LeanAiPolicy,
    /// Global git excludes (`core.excludesFile`).
    GitGlobal,
    /// `.git/info/exclude`.
    GitInfoExclude,
    /// `.gitignore` files, nearest-first.
    GitIgnore,
    /// `.aiignore` files, nearest-first. Layered in Phase 4.
    AiIgnore,
    /// An explicit user decision in the UI.
    UserOverride,
}

impl IgnoreSource {
    pub fn label(&self) -> &'static str {
        match self {
            IgnoreSource::LeanAiPolicy => "LeanAI safety policy",
            IgnoreSource::GitGlobal => "global gitignore",
            IgnoreSource::GitInfoExclude => ".git/info/exclude",
            IgnoreSource::GitIgnore => ".gitignore",
            IgnoreSource::AiIgnore => ".aiignore",
            IgnoreSource::UserOverride => "your choice",
        }
    }

    /// Precedence rank; higher wins.
    pub fn rank(&self) -> u8 {
        match self {
            IgnoreSource::GitGlobal => 0,
            IgnoreSource::GitInfoExclude => 1,
            IgnoreSource::GitIgnore => 2,
            IgnoreSource::AiIgnore => 3,
            IgnoreSource::LeanAiPolicy => 4,
            IgnoreSource::UserOverride => 5,
        }
    }
}

/// Paths that are never selectable by default because they commonly hold
/// credentials. Blocking these is a policy decision, not a scan result, so it
/// cannot be defeated by a folder-level "select all" (FR-08).
pub const SENSITIVE_PATH_PATTERNS: &[&str] = &[
    "**/.env",
    "**/.env.*",
    "!**/.env.example",
    "!**/.env.*.example",
    "**/.npmrc",
    "**/.pypirc",
    "**/.netrc",
    "**/_netrc",
    "**/.git-credentials",
    "**/.aws/credentials",
    "**/.aws/config",
    "**/.ssh/**",
    "**/.gnupg/**",
    "**/id_rsa",
    "**/id_dsa",
    "**/id_ecdsa",
    "**/id_ed25519",
    "**/*.pem",
    "**/*.key",
    "**/*.p12",
    "**/*.pfx",
    "**/*.keystore",
    "**/*.jks",
    "**/secrets.json",
    "**/secrets.yaml",
    "**/secrets.yml",
    "**/credentials.json",
    "**/service-account*.json",
    "**/.terraform/**",
    "**/*.tfstate",
    "**/*.tfstate.backup",
    "**/.kube/config",
    "**/.docker/config.json",
];

/// Directories never walked. Cuts scan cost and avoids app-internal noise.
pub const ALWAYS_SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    ".DS_Store",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".venv",
    "venv",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".gradle",
    ".idea",
    ".vs",
    "vendor",
    "Pods",
    "DerivedData",
];

/// Lockfiles: text, but rarely useful context and very token-expensive.
/// Excluded by default with a reason; the user may re-enable one explicitly.
pub const LOCKFILE_NAMES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "bun.lockb",
    "Cargo.lock",
    "poetry.lock",
    "Pipfile.lock",
    "uv.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
    "gradle.lockfile",
    "packages.lock.json",
];

/// Extensions treated as binary without a content probe.
pub const BINARY_EXTENSIONS: &[&str] = &[
    "png",
    "jpg",
    "jpeg",
    "gif",
    "bmp",
    "ico",
    "icns",
    "tiff",
    "webp",
    "avif",
    "heic",
    "svgz",
    "mp3",
    "wav",
    "flac",
    "ogg",
    "m4a",
    "aac",
    "mp4",
    "mov",
    "avi",
    "mkv",
    "webm",
    "wmv",
    "zip",
    "gz",
    "tgz",
    "bz2",
    "xz",
    "zst",
    "7z",
    "rar",
    "tar",
    "jar",
    "war",
    "ear",
    "exe",
    "dll",
    "so",
    "dylib",
    "a",
    "lib",
    "o",
    "obj",
    "bin",
    "dat",
    "class",
    "pyc",
    "pyo",
    "pdb",
    "wasm",
    "pdf",
    "doc",
    "docx",
    "xls",
    "xlsx",
    "ppt",
    "pptx",
    "odt",
    "ods",
    "sqlite",
    "sqlite3",
    "db",
    "mdb",
    "ttf",
    "otf",
    "woff",
    "woff2",
    "eot",
    "psd",
    "ai",
    "sketch",
    "fig",
    "blend",
    "gguf",
    "safetensors",
    "onnx",
    "pt",
    "pth",
    "ckpt",
    "h5",
    "npy",
    "npz",
    "parquet",
    "arrow",
    "dmg",
    "iso",
    "img",
];

/// Path fragments that mark generated or vendored output.
pub const GENERATED_PATH_MARKERS: &[&str] = &[
    "/node_modules/",
    "/dist/",
    "/build/",
    "/target/",
    "/coverage/",
    "/.next/",
    "/__generated__/",
    "/generated/",
    // Build-tool output directories. `src-tauri/gen/` alone accounts for ~43%
    // of this repository's own token count if it is left selectable.
    "/gen/",
    "/.gen/",
    "/gen_/",
    "/obj/",
    "/bin/debug/",
    "/bin/release/",
    "/.dart_tool/",
    "/.terraform/",
    "/vendor/",
    "/third_party/",
];

/// Path fragments that mark tests. Excluded from the "source code" recipe:
/// useful when you are working *on* the tests, noise when you are not.
pub const TEST_PATH_MARKERS: &[&str] = &[
    "/test/",
    "/tests/",
    "/__tests__/",
    "/spec/",
    "/specs/",
    "/e2e/",
    "/__mocks__/",
    "/testdata/",
    "/fixtures/",
    "/__snapshots__/",
    "/cypress/",
    "/playwright/",
];

/// Filename patterns that mark a test file wherever it lives.
pub const TEST_FILE_MARKERS: &[&str] = &[
    ".test.",
    ".spec.",
    "_test.",
    "_spec.",
    ".snap",
    "conftest.py",
];

/// Documentation and example paths. Excluded from the "source code" recipe.
pub const DOCS_PATH_MARKERS: &[&str] = &[
    "/docs/",
    "/doc/",
    "/examples/",
    "/example/",
    "/samples/",
    "/benches/",
    "/benchmark/",
    "/benchmarks/",
    "/.github/",
    "/website/",
];

/// Extensions treated as structured data rather than source. These are held to
/// `Limits::max_data_file_bytes` instead of `max_file_bytes`.
pub const DATA_EXTENSIONS: &[&str] = &[
    "json",
    "yaml",
    "yml",
    "xml",
    "csv",
    "tsv",
    "ndjson",
    "jsonl",
    "plist",
    "svg",
    "geojson",
    "graphql",
    "sql",
    "po",
    "pot",
    "properties",
];

/// Filename suffixes that mark minified or map output.
pub const GENERATED_SUFFIXES: &[&str] = &[
    ".min.js",
    ".min.css",
    ".min.mjs",
    ".map",
    ".bundle.js",
    ".chunk.js",
    "-lock.json",
    ".pb.go",
    ".pb.cc",
    ".pb.h",
    "_pb2.py",
    ".g.dart",
    ".freezed.dart",
    ".generated.ts",
];

/// The full policy applied to one project scan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub version: u32,
    pub limits: Limits,
    /// Respect `.gitignore`, `.git/info/exclude` and global git excludes.
    pub respect_git_ignore: bool,
    /// Layer `.aiignore` on top of git ignore rules (Phase 4).
    pub respect_ai_ignore: bool,
    /// Include dotfiles that are not otherwise excluded.
    pub include_hidden: bool,
    /// Follow symlinks. Off by default: a symlink can point outside the root.
    pub follow_symlinks: bool,
    /// Treat lockfiles as excluded-by-default.
    pub exclude_lockfiles: bool,
    /// Treat generated/minified output as excluded-by-default.
    pub exclude_generated: bool,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            version: POLICY_VERSION,
            limits: Limits::default(),
            respect_git_ignore: true,
            respect_ai_ignore: true,
            include_hidden: false,
            follow_symlinks: false,
            exclude_lockfiles: true,
            exclude_generated: true,
        }
    }
}
