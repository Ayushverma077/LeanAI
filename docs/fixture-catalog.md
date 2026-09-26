# Test fixture catalog

Backlog item 0.5. Fixtures are **built at test time** by
`crates/leanai-core/tests/support/mod.rs` rather than committed, because several
of them cannot survive a git checkout: `.env` files and `node_modules` are hit
by this repository's own `.gitignore`, symlinks and permission bits do not
round-trip across platforms, and a committed 2 MB file would bloat every clone.
A fixture the repo hides is a fixture that silently stops testing anything.

| Fixture | Builder | What it exercises | Used by |
| --- | --- | --- | --- |
| `small-app` | `support::small_app()` | Happy path: README, `package.json`, TypeScript sources, an Express route, a TODO marker | scan ordering, bundling determinism, output modes, context sections |
| `nested-ignore` | `support::nested_ignore()` | Root `.gitignore` with a negation (`!keep.log`), a nested `src/.gitignore`, an ignored `build/` | git-ignore precedence and negation handling |
| `secrets` | `support::secrets_repo()` | `.env`, `.env.example`, an RSA private key, a service-account JSON, and code containing an AWS key id and a GitHub token | credential classification, folder-selection safety, override flow, secret scanner |
| `binary-heavy` | `support::binary_heavy()` | PNG (extension), extensionless binary with a NUL byte (content probe), invalid UTF-8, a 2 MB file, a lockfile, minified output in `public/` and in a skipped `dist/` | binary detection, size limits, encoding failures, generated/lockfile classes |
| `monorepo` | `support::monorepo()` | Three packages (TS/Rust/Python), four dependency manifests, a FastAPI route, a CI workflow, a `node_modules` tree | dependency extraction, route detection, workflow detection, skipped directories, limits |
| git-backed variants | `Fixture::git_init` / `git_commit_all` | Commit, dirty tree, staged/unstaged/untracked, diff against a base ref | revision identity, diff-mode scopes |
| symlink escape | `Fixture::symlink` (Unix) | A symlink to a directory outside the root and a symlink to a file outside the root | root-boundary enforcement |
| `permission-restricted` | `support::permission_restricted()` | Unreadable directory (`0o000`), unreadable file (`0o000`), real EACCES error reporting | `ScanIssue` error resilience, permission handling |
| `large-tree` | `support::large_tree(count)` | 100,000+ files across 100 directories, multi-threaded synthesis, sorting & memory | scanner performance, progress reporting, UI virtualisation (see [performance matrix](performance-matrix.md)) |

## Fixtures still to build

These belong to phases that are not implemented yet; each is named here so the
gap is visible rather than discovered later.

| Fixture | Needed for | Phase |
| --- | --- | --- |
| Prompt-injection corpus (instructions embedded in README, comments, test data) | Agent adversarial acceptance tests | 8.8 |
| Malicious model URL / corrupt GGUF | Download verification and sidecar failure paths | 6.4 |
| Prior-release database | Migration upgrade rehearsal from the previous version | 12.4 |

## Adding a fixture

1. Add a builder to `tests/support/mod.rs` returning a `Fixture`.
2. Add a row above saying what it exercises.
3. Reference it from at least one test that would fail if the behaviour
   regressed. A fixture with no assertion is dead weight.
