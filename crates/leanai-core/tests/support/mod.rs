//! Fixture builders for the test corpus described in `docs/fixture-catalog.md`.
//!
//! Fixtures are built at test time rather than committed, because several of
//! them (`.env` files, binaries, symlinks, permission-restricted paths) cannot
//! be represented portably in a git checkout — and a fixture that the repo's
//! own `.gitignore` hides would silently stop testing anything.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tempfile::TempDir;

pub struct Fixture {
    pub dir: TempDir,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("temp dir"),
        }
    }

    pub fn root(&self) -> &Path {
        self.dir.path()
    }

    /// Writes a UTF-8 file, creating parent directories.
    pub fn file(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    /// Writes raw bytes, for binary and invalid-UTF-8 cases.
    pub fn bytes(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, contents).unwrap();
        path
    }

    pub fn dir_at(&self, relative: &str) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(unix)]
    pub fn symlink(&self, target: &Path, relative: &str) -> PathBuf {
        let path = self.dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, &path).unwrap();
        path
    }

    /// Initialises a git repository with one commit so `source_revision` and
    /// diff mode have something to work with.
    pub fn git_init(&self) -> git2::Repository {
        let repo = git2::Repository::init(self.dir.path()).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "LeanAI Fixture").unwrap();
            config
                .set_str("user.email", "fixture@example.invalid")
                .unwrap();
        }
        repo
    }

    pub fn git_commit_all(&self, repo: &git2::Repository, message: &str) -> git2::Oid {
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = repo.signature().unwrap();
        let parent = repo.head().ok().and_then(|head| head.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .unwrap()
    }

    #[cfg(unix)]
    fn restore_tree_permissions(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
                    Self::restore_tree_permissions(&path);
                } else {
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644));
                }
            }
        }
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // Restore write/execute permissions on all subdirectories and files
            // in case any were set to 0o000 by permission-restricted tests,
            // allowing TempDir to delete cleanly.
            Self::restore_tree_permissions(self.dir.path());
        }
    }
}

/// `small-app`: a minimal, well-formed source tree. The happy path.
pub fn small_app() -> Fixture {
    let fixture = Fixture::new();
    fixture.file(
        "README.md",
        "# Small App\n\nA tiny fixture project used by LeanAI tests.\n",
    );
    fixture.file(
        "package.json",
        r#"{
  "name": "small-app",
  "version": "1.0.0",
  "scripts": { "build": "tsc", "test": "vitest run" },
  "dependencies": { "express": "^4.19.2" },
  "devDependencies": { "typescript": "^5.4.0" }
}
"#,
    );
    fixture.file(
        "src/index.ts",
        r#"import express from "express";
import { greet } from "./greet";

export const app = express();

app.get("/health", (_req, res) => res.json({ ok: true }));
app.post("/greet/:name", (req, res) => res.send(greet(req.params.name)));

export function start(port: number): void {
  // TODO: read the port from configuration instead of the caller
  app.listen(port);
}
"#,
    );
    fixture.file(
        "src/greet.ts",
        r#"export interface Greeting {
  text: string;
}

export function greet(name: string): string {
  return `Hello, ${name}`;
}
"#,
    );
    fixture.file("src/util/format.ts", "export const VERSION = \"1.0.0\";\n");
    fixture
}

/// `nested-ignore`: nested `.gitignore` files with negations.
pub fn nested_ignore() -> Fixture {
    let fixture = Fixture::new();
    fixture.git_init();
    fixture.file(".gitignore", "*.log\nbuild/\n!keep.log\n");
    fixture.file("keep.log", "kept by a negation\n");
    fixture.file("drop.log", "ignored by *.log\n");
    fixture.file("src/app.ts", "export const app = 1;\n");
    fixture.file("src/.gitignore", "secret-notes.md\n");
    fixture.file("src/secret-notes.md", "ignored by the nested rule\n");
    fixture.file("src/public-notes.md", "not ignored\n");
    fixture.file("build/output.js", "console.log('generated');\n");
    fixture
}

/// `secrets`: credential-sensitive paths that must never be selectable by a
/// folder-level action.
pub fn secrets_repo() -> Fixture {
    let fixture = Fixture::new();
    fixture.file(
        "src/config.ts",
        "export const config = { region: \"eu-west-1\" };\n",
    );
    fixture.file(
        ".env",
        "DATABASE_URL=postgres://admin:hunter2@db.internal:5432/app\n",
    );
    fixture.file(".env.example", "DATABASE_URL=\n");
    fixture.file(
        "deploy/id_rsa",
        "-----BEGIN RSA PRIVATE KEY-----\nMIIEow==\n-----END RSA PRIVATE KEY-----\n",
    );
    fixture.file(
        "deploy/service-account.json",
        "{\"private_key\": \"-----BEGIN PRIVATE KEY-----\"}\n",
    );
    fixture.file(
        "src/leaky.ts",
        "const client = createClient(\"AKIAIOSFODNN7EXAMPLE\");\nconst gh = \"ghp_0123456789abcdefghijklmnopqrstuvwxyz\";\n",
    );
    fixture
}

/// `binary-heavy`: binaries, oversized files and invalid UTF-8.
pub fn binary_heavy() -> Fixture {
    let fixture = Fixture::new();
    fixture.file("src/main.rs", "fn main() {}\n");
    fixture.bytes(
        "assets/logo.png",
        &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 1, 2, 3],
    );
    // No binary extension, but binary content: only the probe catches this.
    fixture.bytes("data/blob.dat", &[b'h', b'i', 0x00, 0xFF, 0xFE, 0x01]);
    fixture.bytes("data/invalid.txt", &[b'o', b'k', 0xC3, 0x28, b'!']);
    fixture.bytes("data/huge.txt", &vec![b'a'; 2 * 1_048_576]);
    fixture.file("package-lock.json", "{\"lockfileVersion\": 3}\n");
    // `dist/` is never walked at all; `public/` is, so it exercises the
    // generated-suffix rule rather than the skipped-directory rule.
    fixture.file("dist/bundle.min.js", "!function(){}();\n");
    fixture.file("public/app.min.js", "!function(){}();\n");
    // Build-tool output that no ignore file covers, plus a data file that is
    // small enough for the code limit but far too large to be useful context.
    fixture.file(
        "src-tauri/gen/schemas/desktop-schema.json",
        "{\"generated\": true}\n",
    );
    fixture.bytes("config/big-fixture.json", &vec![b'0'; 128 * 1024]);
    fixture.file("config/small.json", "{\"ok\": true}\n");
    fixture
}

/// `monorepo`: several packages, deeper nesting, mixed languages.
pub fn monorepo() -> Fixture {
    let fixture = Fixture::new();
    fixture.file("README.md", "# Monorepo\n\nTwo packages and a service.\n");
    fixture.file(
        "packages/web/src/App.tsx",
        "export function App() { return null; }\n",
    );
    fixture.file(
        "packages/web/package.json",
        "{\"name\":\"web\",\"dependencies\":{\"react\":\"^19.0.0\"}}\n",
    );
    fixture.file(
        "packages/core/src/lib.rs",
        "pub fn compute() -> u32 { 1 }\npub struct Engine;\n",
    );
    fixture.file(
        "packages/core/Cargo.toml",
        "[package]\nname = \"core\"\n\n[dependencies]\nserde = \"1\"\n",
    );
    fixture.file("services/api/main.py", "from fastapi import FastAPI\n\napp = FastAPI()\n\n@app.get(\"/items\")\ndef list_items():\n    return []\n");
    fixture.file(
        "services/api/requirements.txt",
        "fastapi==0.111.0\nuvicorn>=0.30\n",
    );
    fixture.file("node_modules/left-pad/index.js", "module.exports = 1;\n");
    fixture.file(".github/workflows/ci.yml", "name: ci\non: [push]\n");
    fixture
}

/// `permission-restricted`: an unreadable directory and an unreadable file to test
/// `ScanIssue` reporting under real EACCES conditions (backlog 11.2).
pub fn permission_restricted() -> Fixture {
    let fixture = Fixture::new();
    fixture.file("readable/main.ts", "export const ok = true;\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let unreadable_file = fixture.file("unreadable_file.ts", "secret content\n");
        std::fs::set_permissions(&unreadable_file, std::fs::Permissions::from_mode(0o000)).unwrap();

        let unreadable_dir = fixture.dir_at("unreadable_dir");
        fixture.file("unreadable_dir/nested.ts", "unreachable content\n");
        std::fs::set_permissions(&unreadable_dir, std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    #[cfg(not(unix))]
    {
        let unreadable_file = fixture.file("unreadable_file.ts", "secret content\n");
        let mut perms = std::fs::metadata(&unreadable_file).unwrap().permissions();
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(&unreadable_file, perms);
        fixture.file("unreadable_dir/nested.ts", "unreachable content\n");
    }
    fixture
}

/// `large-tree`: a synthetic repository with 100,000+ files to test
/// scanner performance, progress reporting and UI virtualisation (backlog 2.7, 11.2).
pub fn large_tree(count: usize) -> Fixture {
    let fixture = Fixture::new();
    let root = fixture.root().to_path_buf();
    let num_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4);
    let dirs_count = 100.max(num_threads);
    let files_per_dir = count / dirs_count;
    let remainder = count % dirs_count;

    for d in 0..dirs_count {
        std::fs::create_dir_all(root.join(format!("pkg_{d:03}"))).unwrap();
    }

    std::thread::scope(|s| {
        let chunk_size = dirs_count.div_ceil(num_threads);
        for t in 0..num_threads {
            let start_dir = t * chunk_size;
            let end_dir = ((t + 1) * chunk_size).min(dirs_count);
            let root_ref = &root;
            s.spawn(move || {
                for d in start_dir..end_dir {
                    let dir_path = root_ref.join(format!("pkg_{d:03}"));
                    let files_in_this_dir = if d == dirs_count - 1 {
                        files_per_dir + remainder
                    } else {
                        files_per_dir
                    };
                    for f in 0..files_in_this_dir {
                        let file_path = dir_path.join(format!("mod_{f:04}.ts"));
                        let _ = std::fs::write(&file_path, b"export const v = 1;\n");
                    }
                }
            });
        }
    });

    fixture
}
