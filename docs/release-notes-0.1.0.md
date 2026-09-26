# LeanAI Desktop 0.1.0 — draft release notes

Status: **draft**. This release has not been signed, notarized or published; see
[project status](project-status.md).

## What this release does

LeanAI Desktop turns a local repository into a deterministic, inspectable
context bundle. Everything runs on your device.

### Implemented

- **Safe project access.** One approved directory; every path resolved against
  it; symlinks are not followed and links pointing outside are reported.
- **Git-aware scanning.** `.gitignore`, `.git/info/exclude`, global excludes and
  `.aiignore`, with a documented precedence order. Deterministic ordering,
  cancellable, per-path error reporting, progress events.
- **Explainable exclusions.** Binary (by extension *and* content probe),
  generated, lockfile, hidden, oversized, unsupported encoding, unreadable and
  credential-sensitive — each with a reason shown in the UI.
- **Deterministic bundling.** Same revision + selection + options ⇒ identical
  bytes and hash. Headers, tree preamble, code fences, line numbers, size
  annotations, front matter, line-ending normalisation.
- **Honest token estimates.** Every figure labelled
  `estimate · cl100k_base · OpenAI-family`, with per-file contribution ranking.
- **Export review.** Clipboard and file exports both pass a preflight showing
  the file list, size, labelled estimate, graded secret findings with redacted
  excerpts, and where the data is going. Everything is written to an audit log.
- **Presets, git-diff selection, `.aiignore` with preview**, retention controls
  and history clearing.
- **Project context index.** Fifteen sections with per-section provenance,
  freshness, stated limitations and source-on-demand.
- **Accessibility.** Full keyboard operation, ARIA tree semantics, no meaning
  carried by colour alone.

### Experimental

- `leanai-bench`, the context-strategy benchmark harness. It reports
  deterministic metrics and prints what it did not measure.

## Known limitations

- The secret scanner is heuristic: it misses real secrets and flags harmless
  strings. It is a review aid, never a guarantee.
- Symbol extraction is lexical, not an AST parse. Context sections say so.
- Route detection covers common Express/Fastify, Flask/FastAPI and axum/actix
  patterns only.
- Token counts are OpenAI-family estimates and will differ from other providers'
  billing.
- Windows has been exercised through CI only, not through the manual test plan.
- Not signed or notarized; the OS will warn on first launch.

## Privacy

- No network capability is granted to the application at all.
- No account, no API key, no telemetry unless you opt in — and telemetry never
  contains source text, bundle text, file paths or credentials.
- Bundle history keeps metadata only by default; the text of your bundles is not
  retained unless you ask for it.
- The database has no column that could hold a credential.

## Performance

No token- or cost-savings percentage is claimed. The benchmark harness exists
and its single published run is documented as an anecdote, with an explicit list
of what it did not measure. See [benchmark methodology](benchmark-methodology.md).

## Not in this release

Local model runtime, cloud providers, model routing and the agent workspace are
designed (ADRs 0007-0010) and not implemented.
