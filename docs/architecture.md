# Architecture

## Shape

```text
React + TypeScript (src/)
  Overview · Context · Tasks (Ask LeanAI) · History · Models · Settings
  Zustand store · typed IPC client (src/ipc/client.ts)
                 │  invoke(command, { request })  →  typed response | AppError
                 ▼
Tauri application layer (src-tauri/)
  commands/   projects · bundles · context · git · models · prompt · agent
              settings · diagnostics
  llm_client  Anthropic · OpenAI · llama-server sidecar · self-hosted servers
  app_state   session, settings, cancellation, sidecar, keychain, DB handle
  db/         versioned migrations + repositories
                 │
                 ▼
leanai-core (crates/leanai-core/)
  policy → walker → classify → inventory → selection → concat → manifest
  tokenizer · secrets · gitinfo · aiignore · symbols · context · benchmark
  catalog · routing · llm_protocol · agent · approval · apply_gate · retrieval
                 │
                 ├─ the approved project root (boundary-checked)
                 └─ SQLite in the OS app-data directory
```

The webview holds no shell, HTTP or ambient filesystem capability (ADR 0002).
The Rust backend reaches the network only for calls to model providers you
configure (`llm_client`) and for GitHub operations you start, which run `git`,
`ssh` and `curl` as child processes (`commands/git.rs`). Scanning, bundling,
token estimation and the context index never use the network.

## Why the split

`leanai-core` holds every product rule: what may be scanned, how a file is
classified, what a folder selection may include, how bytes are ordered in a
bundle, how a token count is labelled, what a context section may claim.

The consequence is that the rules are testable without a desktop app — the
core crate's unit tests run in well under a second — and that adding a Tauri command cannot
accidentally widen them. A command that wanted to bypass `selection::resolve`
would have to reimplement classification, which is visible in review.

## The three boundaries that matter

**1. The project root.** `project::canonical_root` canonicalises the directory
the user picked. `project::resolve_within_root` is the only way to turn a
relative path from the UI, a preset, a manifest or a model response into an
absolute path; it rejects absolute paths, `..` components, and symlinks whose
target resolves outside the root. The scanner does not follow links by default,
and reports a link that points outside rather than silently omitting it.

**2. Selectability.** `FileClass::selectable_by_default` decides what a folder
selection may include. `FileClass::requires_explicit_override` decides what even
an explicit selection may not include without a per-file confirmation. Both live
in core; the UI renders the outcome and the reason.

**3. Export.** Every route out of the app — clipboard or file — goes through
`export_preflight` (file list, size, labelled estimate, secret findings,
destination note) and is recorded by `record_audit`. Clipboard write is the only
capability the frontend holds that sends data anywhere, and it only ever receives
text the backend produced after the preflight.

## Determinism

A bundle is a pure function of (revision, selection, options):

- the walk is single-threaded and the result is sorted by path;
- paths are normalised to `/` on every platform;
- line endings are normalised to `\n` by default;
- the selection is sorted and de-duplicated before building;
- the output is hashed, and the hash is stored with the bundle.

`bundles_are_byte_identical_across_runs` and
`selection_order_does_not_change_the_bundle` hold this in place.

## Concurrency

`AppState` guards each concern with its own `Mutex` and never holds a lock
across an `await`. Scanning and bundling clone their inputs and run on
`spawn_blocking`; the result is written back in a short critical section. Scan
cancellation is a shared `AtomicBool` checked between directory entries, so a
cancelled scan returns `Cancelled` rather than a partial inventory.

## Data

One SQLite file in the OS app-data directory, WAL mode, foreign keys on.
Migrations are append-only and each runs in its own transaction; a database from
a newer build is refused with a message rather than migrated backwards. Schema
v2 already contains the `runs` / `run_events` / `approvals` ledger, so exports
today are audited through the same tables agents will use in Phase 8.

There is no column anywhere for a credential, and a test fails the build if one
appears.

## Error handling

`CoreError` is the internal failure type; `AppError` is what crosses IPC, with a
stable `code`, a display-safe `message`, an optional `recovery` sentence and a
`retryable` flag. OS error strings are stripped of absolute paths before they
reach the UI or a diagnostic bundle. The frontend renders `recovery` next to
every error, so a failure always states the next action.

## Prompt execution

`commands/prompt.rs::run_prompt` is the one-box flow behind **Ask LeanAI**:

```text
analyze_prompt ─► plan_context_budget ─► select_model          (routing, no model)
      │
      ▼
render_sections (PROJECT_CONTEXT, capped) ─► model loop:
      read_files / search  ─► resolve_within_root, inventory checks, redaction
      edit                 ─► build_proposal (exact find/replace, hashed diff)
                           ─► validate_proposal (boundary, secrets, well-formed)
      answer / insufficient
      │
      ▼
apply_gate::decide ─► auto_apply  → TransactionalPatchSession → refresh context
                  └─► confirm / careful_review → ApprovalRequest (bound to hash)
```

- **The model is untrusted input.** Every action is parsed and validated by
  `llm_protocol::parse_action`. Agent turns also ask the provider to constrain
  the reply to `llm_protocol::action_schema()`; that guarantees shape, not
  meaning (ADR 0013).
- **Escalation needs evidence.** The loop moves to the next stronger model only
  on concrete failure: an unreachable or unavailable provider, repeated
  invalid replies or edits, or the model reporting the task is beyond it.
- **The apply gate** (`apply_gate.rs`) ranks each validated change as
  `auto_apply`, `confirm` or `careful_review` with reasons. Its thresholds are
  policy constants in that module, and every decision is stored with the run
  (ADR 0013).
- **Every run is recorded** in `runs` / `run_events`: the budget, retrievals,
  searches, metrics, routing analysis, escalations and apply decision.

## Extension points for later phases

| Phase | Where it plugs in |
| --- | --- |
| 6 Local runtime | A sidecar manager in `src-tauri`, behind the provider trait from ADR 0008 |
| 7 Cloud providers | Provider adapters + capability profiles; the export preflight already exists and is tested |
| 8-9 Agents | `runs` / `run_events` / `approvals` tables; `context::validate_model_section` is the citation-validation pattern the roles reuse |
| 10 Semantic memory | Ranking layer above `selection::resolve`; must beat the benchmark baseline before shipping |
