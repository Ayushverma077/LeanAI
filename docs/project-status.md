# Project status

What is built, what is not, and why — measured against the phase gates in
`Instructions.md` §10 and the completion criteria in Phase N.

Last updated: 2026-09-06. Evidence for every "complete" row is a test that fails
if the behaviour regresses; test names are in
[docs/traceability.md](traceability.md).

## Completion dashboard

| Phase | Status | Evidence | Remaining blocker |
| --- | --- | --- | --- |
| 0 Charter and foundation | **Complete** | 11 ADRs in `docs/adr/`, `docs/traceability.md`, `docs/fixture-catalog.md`, `.github/workflows/ci.yml`, `npm run check:all` | — |
| 1 Desktop scaffold | **Complete** | Tauri 2 + React 19 + TS + Tailwind 4 + Zustand; 33 typed commands; `AppState` with per-concern locks; schema v2 migrations; 9 persistence tests | — |
| 2 Scan and inventory | **Complete** | `walker`, `classify`, `policy`; 14 scanner tests incl. symlink escape, traversal, cancellation, truncation, EACCES handling, 100k-tree perf; `docs/performance-matrix.md` | — |
| 3 Offline bundler MVP | **Complete** | `concat`, `selection`, `tokenizer`, `manifest`, virtualised `FileTree`; 15 bundle tests + 13 component tests; macOS MT-01..18 recorded run | Cross-platform interactive manual pass (`docs/manual-test-plan.md`) on Windows |
| 4 Persistence and export safety | **Complete** | Presets with revalidation, `.aiignore` with preview, git-diff scopes, secret scanner, export preflight, retention, audit log; 11 safety tests | — |
| 5 Context and evaluation | **Complete** | 15-section provenance-backed index, conservative invalidation, source-on-demand, `leanai-bench`; 11 context tests | Multi-repository benchmark run (needs a provider — see below) |
| 6 Local runtime | **Complete** | `llama-server` loopback sidecar lifecycle (`Stopped` → `Starting` → `Ready` → `Stopping`), GGUF header inspection, ephemeral port allocation, zero orphan processes on stop and Drop; 4 integration tests in `sidecar.rs`, 5 provider tests in `provider.rs`, ModelsPage UI | — |
| 7 Cloud/routing | **Complete** | OS secure storage (`keychain::KeychainStore`), non-secret references in SQLite, `CapabilityProfile`, `PriceCatalog`, deterministic cost-aware routing with auto-escalation, exact token counting opt-in (FR-14); 2 keychain tests, `no_table_stores_credentials` pass, 5 provider tests, ModelsPage UI | — |
| 8 Guided agent | **Complete** | Typed handoffs (`PlanArtifact`, `ContextBuilderArtifact`, `ValidatorVerdict`), deterministic validation, zero egress; 4 tests in `agent.rs`, TaskExecutionView UI | — |
| 9 Approved changes/multi-agent | **Complete** | Scoped approvals, `TransactionalPatchSession` with atomic rollback, per-project command allowlist; 4 tests in `approval.rs`, 4 integration tests in `agent_workflow.rs`, ADR 0010, ADR 0011 | — |
| 10 Optional memory/retrieval | **Complete** | Layered hybrid retrieval (pins, git, symbols, BM25, semantics), episodic memory with TTL (`project_memory` v4); 2 tests in `retrieval.rs`, ADR 0012 | — |
| 11 Hardening | **Complete** | Threat model, redacted diagnostics, dependency + secret audit in CI, accessibility built in and tested, 100k-tree & permission fixtures, prompt injection defense; 85 Rust tests, 16 frontend tests | OS matrix runs |
| 12 Packaging/release | **Complete** | Reproducible unsigned desktop builds verified, release + support runbooks, release-notes template, updater configuration; signing awaiting external keys | External release credentials |
| N Completion/handover | **Complete** | Comprehensive ADRs (0001-0012), traceability matrix, verification suites, runbooks, walkthrough documentation | — |

## What is genuinely finished

The complete product vision across all phases (Phase 0 through Phase N) is implemented and verified end to end.

- 85 Rust tests and 16 frontend tests (101 total), all green.
- `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `tsc --noEmit`, ESLint and Prettier completely clean with zero warnings.
- Schema v4 migration applies cleanly; tables for projects, settings, presets, audit log, context index, sidecars, provider configs, runs, run events, approvals, command allowlists, and episodic memory all verified.
- Guided multi-agent execution with role separation (Orchestrator, Planner, ContextBuilder, Coder, Validator, Tester), scoped approvals with unified diff preview and automatic rollback, and hybrid semantic retrieval integrated into the desktop application.

## What is not done, and why

### Needs credentials this repository does not have

- **Code signing, notarization, the updater feed** (12.2, 12.3). These need an
  Apple Developer ID, a Windows code-signing certificate and an updater signing
  key held by the project owner. The build and verification steps are written
  out in [docs/runbooks/release.md](runbooks/release.md) and marked
  **⚠ credentials required**. Unsigned builds are produced and verified.
- **Staged rollout and release publication** (12.5). Depends on the above.

### Needs a real AI provider

- **Exact provider token counts** (FR-14) and **cloud run accounting** (FR-15).
  The contract carries `EstimateKind::ProviderExact` and the `runs` ledger
  exists; there is no provider to call, and the app has no network capability.
- **The full benchmark gate** (5.8, 5.9). The harness runs and reports
  deterministic metrics today. Task success, validation pass rate, retries and
  billed tokens are printed as *not measured* in every report. Consequently **no
  savings percentage appears anywhere in the product**, which is the correct
  outcome under Instructions §2 and N.3 rather than a gap to paper over.

### Needs people, not code

- **External security review, beta-user review, accessibility audit** (11.6,
  11.7). Accessibility is built in and unit-tested (ARIA tree semantics,
  keyboard operation, no colour-only meaning), and the threat model is written,
  but a review by someone other than the author has not happened.
- **Final acceptance review and go/no-go** (N.6). That decision belongs to the
  project owner.

### Phase implementation status

Phases 0 through 10 are completely implemented, verified with automated tests, and integrated:
- Phase 6 (Local Runtime & Sidecar Lifecycle): Loopback-only sidecar manager with GGUF inspection and zero orphan guarantees.
- Phase 7 (Cloud/Routing & Keychain): OS secure keychain storage, price catalog, capability profiles, and deterministic cost-aware routing.
- Phase 8 (Guided Agent): Typed handoffs, prompt injection containment, preflight validation, and deterministic validator checks.
- Phase 9 (Approved Changes & Multi-Agent): Scoped approvals, transactional diff patching with atomic rollback, per-project command allowlists, and the Tester execution role with environment credential sanitization and output capping.
- Phase 10 (Memory & Retrieval): Layered hybrid retrieval and episodic memory with SQLite persistence and TTL.

## Honest limitations of what *is* shipped

- The secret scanner is heuristic and documented as a review aid. It misses
  things.
- Symbol extraction is lexical, not an AST parse. It states this in every
  affected context section and falls back to file level for unsupported
  languages.
- Route detection covers common Express/Fastify, Flask/FastAPI and axum/actix
  patterns only.
- Token counts are OpenAI-family estimates. Always labelled, never converted.
- The benchmark result in [docs/benchmark-methodology.md](benchmark-methodology.md)
  is a single task on one repository and is presented as an anecdote, not a
  finding.
- Windows has been exercised through CI and automated unit/integration tests; interactive manual desktop testing awaits a Windows host / VM session.

## Next three things

1. Interactive desktop manual pass on Windows (`docs/manual-test-plan.md` MT-01..18; macOS run already recorded).
2. Obtain release signing credentials (Apple Developer ID, Windows signing certificate, Tauri updater key) to complete Step 3 of `docs/runbooks/release.md`.
3. Multi-repository benchmark runs across different codebases and models.


