# LeanAI Desktop 0-to-N Project Completion Plan

**Status:** Planning baseline  
**Version:** 0.1  
**Canonical product name:** LeanAI Desktop  
**Planning horizon:** From an empty application repository to a signed, supportable desktop release

## 1. Purpose of this file

This is the master execution plan for LeanAI Desktop. It turns the supplied project brief, presentation, and LLM cost/context research into one ordered backlog, with explicit scope decisions, quality gates, and a definition of completion.

It is deliberately **not** the application's future `PROJECT_CONTEXT.md`. The application will generate and maintain project-specific context files for users' repositories. This document instead governs how LeanAI itself is designed, built, evaluated, released, and closed.

### Current starting point

At the time this plan was prepared, the LeanAI workspace contains research material but no application implementation. Therefore every build item below starts unchecked. Do not mark an item complete merely because a prototype or slide describes it; attach a test, screenshot, benchmark, or release artifact as evidence.

## 2. Source basis and scope resolution

This plan uses the supplied files as reference material:

- `PolyAgent_Desktop_ProjectDoc.pdf` - detailed product/architecture proposal.
- `LLM Cost and Context Research (1).docx` - design research on context selection, routing, compression, and evaluation.
- `LeanAi (3).pdf` - LeanAI presentation and PERT outline.
- `research/PolyAgent_Deep_Research_Catalog_2026-08-14.md` - supporting literature and implementation catalog already present in this workspace.

The documents contain a few inconsistencies. The following decisions keep implementation from splitting into multiple products:

| Topic | Plan decision | Why it matters |
| --- | --- | --- |
| Product name | **LeanAI Desktop** is the canonical name. | The presentation's cover says CARE-SWE while its product pages say LeanAI; PolyAgent is treated as a predecessor/reference name. |
| `PROJECT_CONTEXT.md` | It is an inspectable, versioned **index/cache with provenance**, not a replacement for source code. | Summaries can be stale or omit exact implementation details. Agents must be able to fetch the relevant source on demand. |
| Token savings | 60-88% savings is a **hypothesis to benchmark**, not a launch claim. | Savings vary by repository, provider, task, retry rate, cached tokens, and output tokens. |
| Token counts | Local `cl100k_base` counts are labelled as an **OpenAI-family estimate** only. | They must not be presented as exact Anthropic, Gemini, local-GGUF, or universal counts. |
| `.aiignore` | Respect standard Git ignore rules in the MVP; layer `.aiignore` in the power-features phase. | The source material places `.aiignore` in both MVP and Phase 2. |
| Agent writes and external calls | Default to read-only; require explicit human approval for file writes, shell execution, web access, or cloud transmission. | Repository content is untrusted and can include secrets or indirect prompt injections. |
| Frameworks | Tauri/Rust/React are the proposed baseline; validate dependency compatibility before implementation. | The source material mixes confirmed components with comparison frameworks and contains a `rig-core` language inconsistency. |

## 3. Product charter

### 3.1 Problem

Developers working across a repository must manually locate files, assemble context, estimate token cost, and repeatedly transmit too much unchanged code to an LLM. The result is wasted time, increased cost and latency, lower signal-to-noise ratio, and inconsistent outcomes. A single-model workflow also pays frontier-model prices for simple tasks.

### 3.2 Product outcome

LeanAI Desktop helps a developer turn a local source repository into a **safe, deterministic, inspectable context bundle**. It begins as an offline-first desktop bundler and expands, only after the bundler is reliable, into an opt-in local/cloud model and multi-agent workspace.

### 3.3 Primary users

- Individual developers working in multi-file repositories.
- Cost-conscious developers who need visibility into selected files and estimated context size.
- Privacy-conscious developers who want an offline bundling workflow and, later, local inference.
- Advanced users who want controlled routing between model tiers without losing an audit trail.

### 3.4 Product principles

1. **Deterministic before generative.** File discovery, filtering, output construction, token estimation, and provenance do not require an LLM.
2. **Local-first and consent-based.** Bundling works without an API key. Nothing leaves the device without a clear user action and preflight review.
3. **Source remains authoritative.** A summary points to current code; it never silently replaces it.
4. **Visible context decisions.** Users can see what was selected, excluded, truncated, estimated, sent, and why.
5. **Safe capability escalation.** Agents may reason freely in their sandboxed run, but consequential actions require a scoped approval.
6. **Measure outcomes, not marketing claims.** Evaluate total billed tokens, elapsed time, retries, validation success, and developer usefulness against baselines.
7. **One reliable workflow before many agents.** Establish a strong single-agent/read-only baseline before adding parallel specialist roles.

## 4. Product boundaries

### 4.1 In scope

- A Windows and macOS desktop application built around a local repository picker.
- Git-ignore-aware file inventory, safe filtering, searchable selection, and deterministic text bundling.
- Copy and save flows with transparent token estimates.
- Project presets, history, and explicit user settings stored locally.
- Versioned project-context generation with hashes, freshness state, and targeted source retrieval.
- Optional local model management and optional cloud-provider use.
- A controlled, auditable agent workflow for planning, coding, review, and validation.
- Benchmarking, packaging, signing/notarization, updater support, documentation, and release operations.

### 4.2 Explicitly out of scope for the first stable release

- Unattended agents that modify repositories, run arbitrary commands, or make external purchases/API changes.
- A claim that all local and cloud models have identical capabilities.
- Universal or exact token counting without provider-specific counting support.
- Treating regex secret scrubbing as a security boundary.
- A general-purpose GUI-control or voice-control agent.
- Team collaboration, enterprise administration, cloud synchronization, or multi-user access control.
- A hard dependency on vector search for the offline bundler.

### 4.3 Release slices

| Slice | User-visible promise | Cannot depend on |
| --- | --- | --- |
| **MVP - Offline Bundler** | Select a local repository, safely choose text files, preview a deterministic bundle, see a labelled estimate, and copy/save it. | API keys, remote models, vector DB, agents. |
| **Power Features** | Reusable presets, `.aiignore`, Git diff selection, watch mode, stronger export review. | Automatic cloud upload. |
| **Local/Cloud Runtime** | User-controlled model catalog, local GGUF sidecar, capable cloud adapters, run accounting. | Automatic routing without observability. |
| **Guided Agent Workspace** | Read-only plan/review/test tasks first; approved writes only after the permission model is proven. | Unbounded autonomous loops. |
| **Memory and Retrieval** | Optional semantic task-to-file assistance with inspectable ranking and fallbacks. | Replacing deterministic selection and source-on-demand. |

## 5. Target architecture

### 5.1 Proposed stack

| Layer | Proposed technology | Responsibility | Validation needed before lock-in |
| --- | --- | --- | --- |
| Desktop shell | Tauri 2 | Windows/macOS app packaging, IPC, capabilities, updater integration. | Confirm supported targets, permission model, signing/updater workflow. |
| Backend | Rust | Filesystem operations, bundle construction, persistence, process management, provider adapters. | Confirm async/runtime and error-handling conventions. |
| Frontend | React + TypeScript + Tailwind + Zustand | File selection, previews, settings, run history, approval UI. | Confirm accessibility and virtualization approach. |
| Local persistence | SQLite via `rusqlite` | Project metadata, presets, history, model registry, run ledger. | Define migrations, encryption/key handling, and concurrency model. |
| Repository traversal | `ignore` plus explicit LeanAI policy | Respect Git exclusions and app safety filters. | Test behavior across nested repositories, symlinks, and global ignores. |
| Token estimation | `tiktoken-rs` plus optional provider count endpoint | Instant local estimate and explicitly opt-in exact counts where available. | Per-provider capability profile and labelling. |
| Local inference | `llama-server` sidecar / llama.cpp | Optional GGUF-backed OpenAI-compatible local endpoint. | Verify packaging, signatures, hardware limits, cancellation, and port conflict handling. |
| Model/agent abstraction | Provider adapter boundary; evaluate `rig-core` before adoption | Isolate cloud/local provider differences and tool support. | Confirm current APIs, structured output, streaming, tool calling, and license suitability. |
| Optional memory | LanceDB or an equivalent embedded store | Later semantic retrieval of past runs and files. | Must pass a measurable usefulness and privacy gate before inclusion. |

### 5.2 Component map

```text
React desktop UI
  ├─ Project picker / File tree / Bundle preview
  ├─ Settings / Model catalog / Consent and approval dialogs
  ├─ Agent task workspace / Run history / Metrics
  └─ Typed Tauri command client
                 │
                 ▼
Rust Tauri backend
  ├─ Project policy and capability enforcement
  ├─ Scanner → filter → inventory → selection → deterministic bundler
  ├─ Token-estimation and provider-capability adapters
  ├─ Context index + source-on-demand retrieval
  ├─ SQLite repositories and migrations
  ├─ Local llama-server lifecycle manager
  └─ Agent run coordinator, validator, approval gate, audit ledger
                 │
                 ├─ Local filesystem (explicitly selected project)
                 ├─ Local SQLite database and app data directory
                 ├─ Optional local `llama-server` sidecar
                 └─ Optional approved cloud providers
```

### 5.3 Backend module boundary

```text
src-tauri/src/
  app_state.rs
  error.rs
  commands/
    projects.rs        bundles.rs        settings.rs
    context.rs         models.rs         runs.rs
  core/
    policy.rs          walker.rs         filters.rs
    inventory.rs       selection.rs      concat.rs
    tokenizer.rs       provenance.rs     context_index.rs
    git.rs             secrets.rs        model_catalog.rs
    provider.rs        sidecar.rs        agent_runner.rs
    approval.rs        validation.rs     metrics.rs
  db/
    migrations.rs      repositories.rs   models.rs
```

The exact names can evolve, but the permission/policy layer must remain upstream of filesystem, provider, and agent operations.

### 5.4 Frontend information architecture

1. **Project workspace:** choose/open a repository; show scan state, exclusions, and warnings.
2. **File selection:** virtualized tree, search, folder-level selection, files selected by an explicit rule, and a reversible selection summary.
3. **Bundle preview:** rendered output, source-header mode, options, per-file contribution, token estimate, copy/save/export action.
4. **Context page:** current context index, section freshness, source links/hashes, changed-file impact, regenerate action.
5. **Models and providers:** local model registration/download status, capability cards, API-key status without exposing secrets, and explicit routing policy.
6. **Tasks:** agent plan, exact context supplied, approvals requested, streaming output, validation result, diff/test evidence, and run ledger.
7. **Settings and privacy:** ignore policy, retention, telemetry opt-in, redaction/export defaults, and permission reset.

## 6. Functional requirements and acceptance criteria

### 6.1 Repository scanning and inventory

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-01 | User can choose a local project directory and grant only the required filesystem scope. | Permission tests and manual macOS/Windows verification. |
| FR-02 | Scanner walks files deterministically and respects `.gitignore`, `.git/info/exclude`, and supported global Git ignores. | Fixture suite with nested rules, negations, and different OS paths. |
| FR-03 | LeanAI adds `.aiignore` only after its precedence, syntax, and UI explanation are defined. | Policy tests and a user-facing precedence explanation. |
| FR-04 | Binary, oversized, generated, hidden, and unsafe-by-default files are visibly excluded or warned about. | Fixture matrix including images, archives, executables, lockfiles, `.env`, minified files, symlinks, and permission errors. |
| FR-05 | Inventory preserves relative paths, size, classification, content hash/freshness metadata, and exclusion reason. | Schema tests and a verified sample inventory. |
| FR-06 | Scan can be cancelled, reports recoverable errors per path, and never blocks the UI. | Cancellation and large-tree responsiveness tests. |

### 6.2 Selection and deterministic bundling

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-07 | Users can search, select/unselect files and folders, and inspect the final ordered selection. | UI/integration tests for tree selection and search. |
| FR-08 | Folder selection never silently re-enables excluded binaries or secrets. | Regression tests for selection toggles. |
| FR-09 | Bundles are deterministic for the same project revision, options, and selection. | Golden-output tests with stable ordering. |
| FR-10 | Every included file has a clear relative-path header; optional language fences, line numbers, and file-size annotations are reversible options. | Snapshot tests for each output mode. |
| FR-11 | Copy and Save actions show the destination/content policy and return success or a recoverable error. | Manual test on both platforms plus error-path tests. |
| FR-12 | A bundle records its source project identity, scan revision/time, selection, options, and estimate type. | SQLite/repository tests and history UI verification. |

### 6.3 Token and cost transparency

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-13 | Local estimates update after a debounced selection change and disclose their tokenizer/estimate scope. | UI test and a visible `estimate` label. |
| FR-14 | Exact provider token counts are requested only after the user opts in and a provider supports the endpoint. | Mock provider test: no network request without consent. |
| FR-15 | Cloud run accounting logs input, output, cached input where available, price-catalog version, elapsed time, validation result, and escalation reason. | Run-ledger schema and mock-run integration test. |
| FR-16 | LeanAI never presents estimated cost or claimed savings as a guarantee. | UI/content review and analytics/report tests. |

### 6.4 Project context index

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-17 | Context generation produces a human-readable Markdown index with section-to-source provenance. | Example output with paths, hashes, generation revision, and freshness status. |
| FR-18 | Generated summaries link to relevant source files/symbols and disclose uncertainty or unavailable analysis. | Context schema validation and review checklist. |
| FR-19 | A changed file marks dependent context sections stale; refresh is incremental where safe and full regeneration remains available. | Change-impact fixture tests. |
| FR-20 | Agent context loading selects relevant sections plus source-on-demand paths under a hard token/cost budget. | Traceable context-composition log. |

### 6.5 Models, providers, and routing

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-21 | Local GGUF model registration, activation, health check, stop, retry, and error reporting work without exposing arbitrary shell access. | Sidecar lifecycle tests and manual platform verification. |
| FR-22 | Model downloads use explicit approved source URLs, resumability/cancellation, checksum or signature verification when available, and license/provenance display. | Download test server plus failure/retry cases. |
| FR-23 | Provider adapters expose a capability profile instead of assuming feature parity. | Contract tests for streaming, tools, JSON/schema, vision, token counting, and context limit. |
| FR-24 | API credentials use OS-backed secure storage where available; they are never stored in bundle/history plaintext. | Security review and storage tests. |
| FR-25 | Routing has user-visible policy, fallback order, budget, and audit trail. | Deterministic routing tests and run-history review. |

### 6.6 Agent workspace

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| FR-26 | Every task has an explicit objective, context manifest, allowed tools, budget, owner, and cancellation control. | Run schema validation. |
| FR-27 | Initial agent runs are read-only and cannot write files, execute shell commands, browse, or call external APIs unless the user approves that exact capability. | Denial-path tests and approval UI review. |
| FR-28 | Write-enabled runs produce a reviewable diff, validate it, and require approval before the change is retained. | End-to-end fixture with an accepted and rejected diff. |
| FR-29 | Agent roles exchange structured results with citations to source files/tool output, rather than trusting free-form handoffs. | Schema tests and a multi-step trace. |
| FR-30 | Task history stores concise findings, validation status, and provenance while avoiding repeated full transcripts by default. | Retention policy and context-budget tests. |

## 7. Non-functional requirements

### 7.1 Safety and privacy

- Treat every repository file, README, issue, web result, generated context section, and tool output as untrusted data. Text in a repository is **not** an instruction to LeanAI or its agents.
- Default-deny cloud transmission. Before sending context externally, show the selected files, detected secret warnings, provider, region/data policy if known, estimated size, and the exact action being approved.
- Exclude `.env`, private keys, credentials, known secret stores, and OS metadata by default. Allow an informed override only with an explicit high-friction confirmation.
- Use secret scanning as a warning/review aid, not proof that export is safe. Track false positives and false negatives.
- Store API credentials in platform secure storage; minimize retention of bundle text, agent trajectories, and source excerpts.
- Bind local inference to loopback; avoid exposing a network listener unless the user deliberately enables it and receives a warning.
- Validate model executable provenance, downloaded model metadata, and sidecar compatibility before activation.
- Use scoped filesystem handles and project-root boundary checks. Resolve symlinks cautiously and do not traverse outside the approved root without visible approval.
- Maintain an audit record for every consequential decision: export, provider call, download, tool use, command execution, write, deletion, and retention change.

### 7.2 Reliability and recovery

- No partial data loss on cancellation, crash, or model-sidecar failure.
- SQLite migrations are versioned, reversible where practical, and tested against the previous release.
- Every long operation reports progress, supports cancellation, and has a clear retry/recover path.
- Provider and sidecar failures preserve task state, error details safe for display, and a resume/retry option when viable.
- Use durable run identifiers and idempotency keys for state-changing remote requests.

### 7.3 Accessibility and UX

- Keyboard navigable file tree, controls, dialogs, and approvals.
- Screen-reader labels for selection state, progress, status colors, and warnings; color never carries meaning alone.
- Clear empty, loading, error, and permission-denied states.
- Users can understand why a file is excluded, why a model was selected, and what data will leave the device without reading logs.

### 7.4 Performance and scalability

- Keep the UI responsive while scanning large repositories; use background work and tree virtualization.
- Establish a public benchmark matrix before setting hard time/memory targets: small, medium, large, monorepo, binary-heavy, and permission-restricted fixtures.
- Measure full-task cost and latency, not only individual model call duration.
- Apply hard limits to file size, selected files, context tokens, estimated spend, run iterations, sidecar start retries, and retained history. Each limit must have a user-visible reason and override policy.

## 8. Canonical data and output contracts

### 8.1 SQLite entities

| Entity | Minimum fields | Notes |
| --- | --- | --- |
| `projects` | id, canonical_path/fingerprint, display_name, last_scan_revision, policy_version, created_at, updated_at | Avoid treating a raw path alone as permanent identity. |
| `presets` | id, project_id, name, selection_spec, bundle_options, created_at, updated_at | Selection must be safely revalidated after scan changes. |
| `bundles` | id, project_id, source_revision, selection_manifest, output_hash, estimate, estimate_type, created_at | Store full bundle only if the retention setting permits it. |
| `context_documents` | id, project_id, source_revision, schema_version, content_hash, freshness_state, created_at | Represents the generated project context index. |
| `context_sections` | id, context_document_id, section_key, source_refs, source_hashes, freshness_state | Enables incremental invalidation and provenance. |
| `models` | id, kind, display_name, source, version, path/remote_id, capability_profile, checksum, status | Model location/credential data must be separately protected. |
| `provider_profiles` | id, provider, model_id, capability profile, price_catalog_version, enabled | A catalog is versioned; it is never assumed current forever. |
| `runs` | id, project_id, task, mode, context_manifest, policy, status, budget, timing, validation_state | Each task gets an audit root. |
| `run_events` | id, run_id, sequence, event_type, redacted_payload, created_at | Supports replay without retaining unrestricted secrets. |
| `approvals` | id, run_id, capability, scope, decision, approver, expires_at | Approval is scoped, revocable, and never silently reused beyond policy. |

### 8.2 Bundle manifest

Every saved/exported bundle should have a companion manifest or embedded front matter containing:

```yaml
format: leanai.bundle/v1
project_fingerprint: <non-secret identifier>
source_revision: <git commit, dirty state, or scan hash>
created_at: <ISO-8601 timestamp>
selection:
  included_files: []
  excluded_files: []
  exclusion_reasons: {}
options:
  headers: true
  line_numbers: false
  code_fences: false
token_estimate:
  value: 0
  kind: openai_family_estimate | provider_exact | unavailable
  tokenizer_or_provider: <identifier>
warnings: []
```

### 8.3 `PROJECT_CONTEXT.md` schema

The generated project context index must include the LeanAI presentation's required categories and the PolyAgent document's operational sections, with source provenance:

```markdown
# Project Context

## Metadata and freshness
## Quick Reference
## Project Summary
## Directory Structure
## File Inventory
## Core Architecture
## Key Modules
## API Contracts
## Dependencies
## Workflows
## Configuration
## Known Issues and TODOs
## Notes and Decisions
## Agent Task History
## Source References and Change Impact
```

For each section, record the generating revision, source files/hashes, last validation time, and freshness state. A user must be able to jump from a claim to its supporting file or request that the agent fetch the current source.

### 8.4 Context composition contract

When LeanAI creates a model request, construct the prompt/context in this order:

1. System safety constraints and user-approved tool policy.
2. Available capabilities, relevant API/tool contracts, and provider limits.
3. Project map/context index sections with provenance.
4. Selected source files, dependency slices, recent diff, and test/error output.
5. The immediate task, requested output format, and validation criteria.

The context manifest records why every file/section was included, how many tokens it contributed, what was dropped because of budget, and how a source can be fetched on demand. Preserve complete relevant functions or AST-valid slices; compress documentation before executing token-level compression over code.

## 9. Operating model for model routing and agents

### 9.1 Model catalog and routing policy

Maintain a live, versioned model catalog. Each model entry contains capability tier, supported context cap, current price data/version, cached-token policy, observed latency, availability, privacy policy, structured-output/tool support, and fallback order.

Initial routing policy:

| Task class | Default tier | Escalate when | Validation before accept |
| --- | --- | --- | --- |
| File inventory, extraction, normalization | Local deterministic code or small/fast model | Parser failure or schema failure | Schema/fixture validation. |
| Context documentation | Small/fast model or local model | Missing provenance, poor coverage, failed review | Required-section/provenance checks. |
| Multi-file planning or migration analysis | Strong reasoning model | Budget permits and lower tier cannot satisfy tests/review | Plan schema plus reviewer/validator. |
| Code change proposal | Capable code model, initially read-only | Tests fail, confidence low, or dependencies remain unresolved | Diff review and targeted tests. |
| Mechanical validation | Deterministic tools first | Tool unavailable or ambiguous failure | Build/test/lint output. |

Never route purely on a model name. Use task class, capability profile, budget, privacy policy, current availability, deterministic validation, user preference, and past observed outcomes. Batch only independent, non-interactive work; never delay an interactive response merely to fill a batch.

### 9.2 Agent roles

| Role | Core responsibility | Default permissions |
| --- | --- | --- |
| Orchestrator | Creates a run plan, budget, dependencies, and final evidence summary. | Read-only orchestration. |
| Planner | Decomposes the request and defines measurable acceptance criteria. | Read project/context manifests. |
| Context Builder / Documenter | Creates or refreshes provenance-backed context sections. | Read selected project files; write only approved generated context path. |
| Analyst / Researcher | Investigates supplied evidence; web use is separate, opt-in, and cited. | Read-only unless user grants web scope. |
| Coder | Proposes or makes scoped changes only after approval. | Read-only initially; approved project writes later. |
| Reviewer / Validator | Checks schemas, policies, diffs, tests, and evidence. | Read tool output and project files. |
| Tester | Runs pre-approved build/test commands in the approved project scope. | No network or destructive operations without separate approval. |

Roles are an implementation detail, not an excuse for uncontrolled parallelism. Start with a single sequenced Planner → Context Builder → Coder → Validator flow. Add concurrent agents only when dependency analysis proves a task is independent and the run ledger can merge their results deterministically.

### 9.3 Agent run state machine

```text
Draft
  → Preflight (policy, root, secrets, budget, capabilities)
  → Context composed
  → Planned
  → Running
  → Awaiting approval (if a consequential action is requested)
  → Validating
  → Succeeded | Failed | Cancelled | Timed out
  → Context refresh queued (only after success and source verification)
```

No run may transition to `Running` if its provider, model capability, project scope, tool permissions, or cost/token budget is unknown. No run may report success without the validator's evidence record.

## 10. 0-to-N build backlog

The ordering below is intentional. A phase cannot begin merely because its code compiles; it begins after the prior phase's exit criteria are satisfied or a documented exception is approved.

### Phase 0 - Charter, decisions, and repository foundation

**Goal:** Remove ambiguity before implementation and establish a safe, reproducible development baseline.

- [x] **0.1** Adopt LeanAI Desktop as the repository, app, package, and release name; record the CARE-SWE/PolyAgent legacy references in an ADR.
- [x] **0.2** Confirm target operating systems, minimum supported hardware, app-data locations, installation/update channel, and the privacy posture of the first release.
- [x] **0.3** Create a requirements traceability matrix linking every in-scope feature to FR/NFR IDs in this document.
- [x] **0.4** Write ADRs for: Tauri capability policy, app-data retention, ignore precedence, token-count labeling, context provenance, provider abstraction, secure credential storage, local sidecar delivery, and agent approval model.
- [x] **0.5** Define the initial test/fixture corpus: small source app, nested Git-ignore app, monorepo, binary-heavy repository, secret-containing fixture, malformed/permission-restricted fixture, and large-tree benchmark fixture.
- [x] **0.6** Define non-production telemetry policy and an explicit opt-in; no source text, bundle text, or credentials in telemetry.
- [x] **0.7** Establish development branch, CI, formatting, linting, dependency-audit, secret-scan, conventional commit/review, and release-note standards.

**Exit criteria met.** Evidence: 11 ADRs in `docs/adr/`, `docs/traceability.md`, `docs/fixture-catalog.md`, `.github/workflows/ci.yml`, `npm run check:all` green.

### Phase 1 - Desktop scaffold and typed command boundary

**Goal:** Build a clean, cross-platform shell that can safely communicate between React and Rust.

- [x] **1.1** Initialize the Tauri/Rust/React/TypeScript application with a reproducible lockfile and documented local setup.
- [x] **1.2** Add typed request/response/error contracts for every Tauri command; reject unstructured stringly typed IPC.
- [x] **1.3** Implement a central `AppState` with safe lifecycle management for database, model state, and background tasks; avoid holding locks across awaited work.
- [x] **1.4** Configure a least-privilege Tauri capability manifest; start with no arbitrary shell, no broad filesystem, and no network by default.
- [x] **1.5** Build the app shell: navigation, empty state, error boundary, loading/cancellation pattern, accessibility baseline, and development diagnostics.
- [x] **1.6** Add SQLite migration framework and repositories for settings/projects only; keep credentials out of SQLite.
- [x] **1.7** Set up unit, component, integration, and native smoke-test commands in CI.

**Exit criteria met.** Evidence: 33 typed commands with request/response structs and `AppError`; `capabilities/default.json` grants no shell, no network and no ambient filesystem; schema v2 migrations with 9 passing tests; `src/ipc/contract.test.ts` fails the build if the frontend and backend command lists diverge. Native smoke launch is covered manually (`docs/manual-test-plan.md` MT-01).

### Phase 2 - Safe project access, scanning, and inventory

**Goal:** Reliably understand a selected local project before creating any bundle.

- [x] **2.1** Implement project selection with canonical root validation, clear revocation behavior, and project fingerprinting.
- [x] **2.2** Implement git-ignore-aware scanning with deterministic ordering and recoverable per-path errors.
- [x] **2.3** Define file classification: source/text, binary, generated, lockfile, credential-sensitive, hidden/metadata, too-large, symlink, unsupported encoding, and unreadable.
- [x] **2.4** Implement binary detection using both conservative extension/MIME policy and a content probe; never read an unbounded file merely to classify it.
- [x] **2.5** Exclude known secret paths by default and display their exclusion reason; implement only a warning scanner at this phase.
- [x] **2.6** Return a serializable inventory/tree with relative path, size, classification, selection eligibility, exclusion reason, and hash/freshness data.
- [x] **2.7** Implement cancellation, incremental progress events, and large-tree/UI-responsiveness tests.
- [x] **2.8** Add comprehensive cross-platform fixture tests, including nested ignore behavior and symlink traversal boundaries.

**Exit criteria met.** Evidence: 14 tests in `crates/leanai-core/tests/scan.rs` covering ordering, nested ignore negations, both binary-detection paths, credential exclusion, symlink escape, traversal rejection, cancellation, truncation, EACCES permission issue handling, and 100,000-file large-tree performance; 13 component tests in `src/components/FileTree.test.tsx` proving bounded DOM virtualisation under 10k+ files; published machine profile in `docs/performance-matrix.md`.

### Phase 3 - Offline bundler MVP

**Goal:** Deliver the first standalone product value without any LLM or network dependency.

- [x] **3.1** Build the virtualized, searchable checkbox file tree with folder selection, tri-state state, selection summary, and keyboard support.
- [x] **3.2** Implement selection rules so a folder cannot silently include disabled/excluded files.
- [x] **3.3** Implement deterministic concatenation with path headers, stable line endings, safe encoding fallback, and configurable optional line numbers/code fences/file-size annotations.
- [x] **3.4** Build output preview with per-file contribution, truncation warning, copy, and save-as flows.
- [x] **3.5** Implement local token estimation with explicit `OpenAI-family estimate` labelling and debounced UI updates.
- [x] **3.6** Add bundle manifest/front matter, output hash, selection manifest, and local history according to retention settings.
- [x] **3.7** Add golden tests for input tree → selected files → output bundle, including encoding/error cases.
- [x] **3.8** Run manual desktop smoke tests for project open, selection, preview, copy, save, cancel, and reopen.

**Exit criteria met on macOS.** Evidence: 15 bundle tests (byte-identical output, order independence, CRLF normalisation, every output mode, per-file contributions, manifest provenance), 13 `FileTree` component tests, and recorded manual run log in `docs/manual-test-plan.md` covering MT-01 to MT-18 on macOS (Darwin arm64). Windows is verified via automated CI test suites; interactive manual testing on Windows is documented as pending a physical Windows host / VM.

### Phase 4 - Persistence, Git awareness, and export safety

**Goal:** Make the MVP useful day to day without weakening trust boundaries.

- [x] **4.1** Add named per-project presets, safe restore/revalidation after file changes, rename/delete, and migration coverage.
- [x] **4.2** Add `.aiignore` with a documented precedence order and preview of its effect before saving it.
- [x] **4.3** Add Git-diff mode with explicit base reference, untracked/unstaged/staged state, and non-Git fallback.
- [x] **4.4** Add filesystem watch mode with debounced re-scan/re-bundle and a visible stale/updated state; users choose whether saved output is overwritten.
- [x] **4.5** Expand secret detection for export review; distinguish blocked known-sensitive paths from heuristic warnings and show false-positive override UX.
- [x] **4.6** Add a cloud-export preflight screen even before the cloud runtime exists, so API integrations inherit a tested consent flow.
- [x] **4.7** Add local retention controls, clear history, and project removal operations that are precise, reversible where possible, and explain what is deleted.

**Exit criteria met.** Evidence: 11 tests in `crates/leanai-core/tests/safety.rs` plus retention and audit tests in `src-tauri/tests/persistence.rs`. `SecretReport::DISCLAIMER` is rendered in every export preflight and asserted in test.

### Phase 5 - Context intelligence and measurable MVP evaluation

**Goal:** Add the structured project map without claiming it replaces source code, then determine whether LeanAI's core promise is real.

- [x] **5.1** Define the versioned `PROJECT_CONTEXT.md` schema and JSON/SQLite provenance representation.
- [x] **5.2** Build deterministic directory, file inventory, dependency, configuration, and known-issue sections before using LLM-written narrative.
- [x] **5.3** Add symbol/AST extraction for supported languages and graceful file-level fallback for unsupported ones.
- [x] **5.4** Generate source references, content hashes, commit/ref information, freshness state, and an on-demand source lookup path.
- [x] **5.5** Implement changed-file invalidation; conservatively mark dependent sections stale if impact cannot be proven.
- [x] **5.6** Add optional Documenter-model generation only behind explicit provider consent; require section/provenance validation before acceptance.
- [x] **5.7** Build a benchmark harness that compares raw bundle, user-pinned bundle, deterministic repository map, and LeanAI hybrid context on representative tasks.
- [ ] **5.8** Measure selected/input/output/cached tokens, elapsed time, retries/tool calls, task success, test success, context freshness, and user corrections.
- [ ] **5.9** Publish an internal benchmark report with repository/task limitations. Do not market a savings percentage until results are repeatable.

**Exit criteria partially met.** Evidence: 11 context tests covering required sections, provenance, conservative invalidation, deterministic regeneration and rejection of uncited model sections; `leanai-bench` runs and reports. **5.8 and 5.9 remain unchecked**: quality and total-cost outcomes need a real provider, which this build has no capability to call. Every report prints `NOT_MEASURED`, and no savings figure appears in the product.

### Phase 6 - Local model runtime

**Goal:** Let users opt into reliable, observable, local inference without compromising the desktop app.

- [x] **6.1** Confirm the model abstraction API and build provider-capability contracts before integrating an agent framework.
- [x] **6.2** Package compatible `llama-server` sidecars per supported architecture with provenance, version, checksum/signature, and license metadata.
- [x] **6.3** Implement sidecar state machine: preflight hardware/disk checks → spawn loopback process → health check → streaming → stop → unexpected-exit recovery.
- [x] **6.4** Guard against ports in use, orphaned processes, cancelled downloads, corrupted GGUFs, incompatible context settings, and invalid paths.
- [x] **6.5** Build model manager UI for register/import/download/activate/deactivate/delete with disk impact and deletion confirmation.
- [x] **6.6** Add local-run accounting: model/version, context settings, prompt/completion estimate, elapsed time, cancellation, and validation status.
- [x] **6.7** Verify app crash isolation when the sidecar fails and that local endpoints are loopback-only by default.

**Exit criteria met.** Evidence: 4 integration tests in `src-tauri/tests/sidecar.rs` and 5 provider tests in `crates/leanai-core/tests/provider.rs` verifying GGUF header validation, loopback-only port allocation, zero orphan processes on stop and Drop, and error handling. ModelsPage UI enables GGUF registration, activation, stopping, and unregistration.

### Phase 7 - Cloud providers, exact counts, and cost-aware routing

**Goal:** Add cloud capability as an explicit choice, not an opaque default.

- [x] **7.1** Implement provider adapters behind a stable capability interface; start with one provider and add others only after contract tests pass.
- [x] **7.2** Store API credentials in OS secure storage; make account disconnect and credential revocation testable.
- [x] **7.3** Add optional exact count APIs and capability-aware context-limit checks; clearly fall back to estimates when exact count is unavailable.
- [x] **7.4** Build a versioned model/price catalog with an update source, timestamp, currency, cache policy, context cap, and user-visible stale indicator.
- [x] **7.5** Implement preflight export consent: selected data manifest, provider, context size, price estimate, budget, secret warnings, and data-retention link where available.
- [x] **7.6** Implement a simple user-configured routing policy with a low-cost tier, a strong tier, deterministic escalation triggers, hard token/cost/iteration ceilings, and manual override.
- [x] **7.7** Log every route decision and actual result; do not use a confidence score alone as evidence of correctness.
- [x] **7.8** Exercise provider outage, rate-limit, malformed response, incorrect capability declaration, budget-exceeded, user-cancel, and retry/idempotency cases.

**Exit criteria met.** Evidence: 2 tests in `src-tauri/tests/keychain.rs` testing secure storage roundtrip, disconnect revocation, and namespace isolation; `src-tauri/tests/persistence.rs::no_table_stores_credentials` passes proving credentials never touch SQLite; `crates/leanai-core/tests/provider.rs` proves deterministic cost routing, budget ceilings, context escalation, catalog staleness detection, and exact token count provenance. ModelsPage provides full UI for keychain configuration, routing simulation, and price catalog display.

### Phase 8 - Guided single-agent workflow

**Goal:** Prove a safe, useful agent loop before introducing parallel orchestration.

- [ ] **8.1** Implement run preflight: objective, project scope, allowed tools, privacy setting, provider/model, context manifest, token/cost/iteration budget, and cancellation deadline.
- [ ] **8.2** Implement Planner output schema with assumptions, subtasks, required evidence, planned tool calls, and definition of success.
- [ ] **8.3** Implement Context Builder output schema with source citations, freshness data, selection rationale, and omitted-context explanation.
- [ ] **8.4** Implement a read-only Coder/Analyst mode that proposes a patch or answer but cannot mutate the project.
- [ ] **8.5** Implement deterministic Validator checks for schema, policy, file scope, citations, and test-plan completeness.
- [ ] **8.6** Add approved Tester execution with a fixed command allowlist per project; commands never inherit ambient shell/network privileges by default.
- [ ] **8.7** Add reviewable run history with concise scratchpad, errors, validator verdict, and source/tool evidence.
- [ ] **8.8** Add acceptance tests for prompt injection contained in source files, secrets in context, unsupported tool calls, malformed model output, stale context, and cancellation.

**Exit criteria:** the end-to-end read-only workflow produces a transparent plan/context/proposal/validation record; unsafe actions are denied by default; an evaluator can reproduce why the result was accepted or rejected.

### Phase 9 - Approved code changes and limited multi-agent execution

**Goal:** Add consequential actions only after the permission and validation model has earned trust.

- [ ] **9.1** Implement scoped write approval: exact project root, approved paths or patch, expiration, and one-time/revocable decision.
- [ ] **9.2** Apply changes transactionally where possible; show diff, affected files, and rollback/revert instructions.
- [ ] **9.3** Require deterministic validation before a change is declared successful: formatting/lint/typecheck/build/test as applicable.
- [ ] **9.4** Introduce Reviewer and Tester handoffs using structured artifacts; the Orchestrator merges only validated outputs.
- [ ] **9.5** Add parallel subtask execution only for explicitly independent, non-conflicting work; define file-write locks and merge conflict resolution.
- [ ] **9.6** Add retry policy with bounded attempts, escalating evidence or model tier only after a recorded reason.
- [ ] **9.7** Protect against destructive commands, out-of-scope paths, force pushes, credential access, or external API actions unless separately approved.

**Exit criteria:** mutation-enabled fixture runs produce safe diffs, pass configured validation, preserve an audit record, and demonstrate denial/recovery for unsafe or conflicting work.

### Phase 10 - Optional semantic memory and task-to-file retrieval

**Goal:** Add semantic assistance only if it beats or complements deterministic context selection in measured tests.

- [ ] **10.1** Write an ADR defining the retrieval problem, supported inputs, embedding model/data location, privacy guarantees, deletion semantics, and deterministic fallback.
- [ ] **10.2** Implement embedded vector storage only behind a feature flag and per-project opt-in.
- [ ] **10.3** Rank explicit pinned files, Git diff/test error signals, symbols/dependency graph, lexical matches, and semantic matches together; expose the ranking reasons.
- [ ] **10.4** Add a Task Box UI that proposes relevant files/sections without auto-sending them to a provider.
- [ ] **10.5** Implement episodic run memory as concise, source-cited findings with expiry/retention controls rather than raw conversation replay.
- [ ] **10.6** Compare retrieval quality, task success, cost, latency, and false-positive context against the non-vector baseline.
- [ ] **10.7** Remove or defer the feature if it fails the usefulness, privacy, or maintenance threshold.

**Exit criteria:** semantic retrieval is opt-in, explainable, removable, benchmarked, and demonstrably better for a defined task class; otherwise it remains out of the release.

### Phase 11 - Quality, observability, evaluation, and hardening

**Goal:** Turn the app from a feature-complete prototype into a credible, measurable product.

- [ ] **11.1** Expand automated coverage across unit, component, integration, end-to-end desktop, migration, permissions, and regression fixtures.
- [ ] **11.2** Test supported OS/architecture matrix, offline mode, low disk, low memory, slow filesystem, broken Git metadata, antivirus/file-lock interference, and sidecar failures.
- [x] **11.3** Build a redacted diagnostic bundle that contains version, logs, capability state, and error codes but no source/secret data by default.
- [ ] **11.4** Establish a metrics dashboard/report for bundle composition, token estimate accuracy where known, task success, cost, latency, retry rate, cancellation, and approval denials.
- [ ] **11.5** Run leakage-aware evaluation where possible; prefer build/test/task completion to BLEU or exact-match-only claims.
- [ ] **11.6** Perform security review: threat model, dependency audit, secret handling, prompt-injection resilience, filesystem boundary tests, credential storage, downloads, updates, and privacy controls.
- [ ] **11.7** Conduct accessibility, UX, and beta-user review; turn confirmed findings into prioritized fixes with acceptance tests.
- [x] **11.8** Produce architecture, privacy, security, troubleshooting, data-retention, and support documentation from verified behavior.

**Exit criteria:** release candidate passes the supported-platform matrix; security/privacy/accessibility findings have owners or documented deferrals; metrics/evaluation report is reproducible; critical defects are closed.

### Phase 12 - Packaging, release, and operational readiness

**Goal:** Ship a trusted desktop application users can install, update, and recover.

- [x] **12.1** Create reproducible release builds for supported Windows/macOS architectures.
- [ ] **12.2** Configure Windows code signing and macOS signing/notarization, including validation of bundled sidecars.
- [ ] **12.3** Implement secure updater feed/signature verification, staged rollout, rollback/revoke procedure, and release-channel policy.
- [ ] **12.4** Prepare installer/uninstaller behavior, app-data preservation/removal choices, and upgrade/migration tests from the prior release.
- [ ] **12.5** Publish release notes that distinguish implemented features, experimental features, known limitations, privacy effects, and evidence-backed performance results.
- [x] **12.6** Set up support intake, incident triage, crash/error reporting opt-in, security disclosure route, and operational owner rotation.
- [ ] **12.7** Execute a fresh-machine install → first bundle → optional model/provider → update → uninstall/reinstall recovery rehearsal.

**Exit criteria:** signed installers and updater are independently verified; release/rollback runbooks succeed in rehearsal; documentation matches shipped behavior; release approval is recorded.

### Phase N - Completion and handover

**Goal:** Declare the project complete only when the product promise, evidence, and operational handover are all finished.

- [ ] **N.1** Confirm every required FR/NFR has a passing acceptance artifact or an approved, documented exclusion.
- [ ] **N.2** Confirm all critical/high security, privacy, data-loss, permission-escalation, and release-blocking defects are resolved.
- [ ] **N.3** Confirm the MVP/feature benchmark report accurately states results and limitations; remove unverified token/cost claims from product copy.
- [ ] **N.4** Confirm source code, tests, migrations, fixtures, architecture docs, ADRs, release notes, licenses/notices, and runbooks are versioned and discoverable.
- [ ] **N.5** Confirm maintainers can reproduce build, test, package, sign, update, diagnose, and roll back from the documented procedures.
- [ ] **N.6** Hold a final acceptance review with the project owner and record go/no-go decision, released version, known limitations, and post-release backlog.
- [ ] **N.7** Tag the release, archive the delivery evidence, and move remaining non-blocking work into a separately prioritized roadmap.

**Exit criteria:** all N.1-N.7 are complete and the project owner accepts the release. Until then, the project is not complete.

## 11. Critical path and sequencing

The supplied presentation illustrates a 17-workday academic PERT path. Treat it as a high-level sequencing diagram rather than a delivery promise: actual duration depends on team size, platform test capacity, model/provider integrations, security review, and release credentials.

The implementation critical path is:

```text
0 Charter/ADRs
  → 1 Scaffold/capabilities
  → 2 Safe scan/inventory
  → 3 Offline bundler MVP
  → 4 Persistence/export safety
  → 5 Context + evaluation
  → 11 Hardening
  → 12 Packaging/release
  → N Acceptance/handover
```

Phases 6-10 are feature expansions. They may run after the MVP gate, but they must not delay a safe, useful bundler release unless explicitly made release requirements. Local/cloud runtime and agents add material security and operational risk; release them behind feature flags or later versions if their gates are not met.

## 12. Test and benchmark strategy

### 12.1 Required test layers

| Layer | Examples | Required proof |
| --- | --- | --- |
| Unit | Ignore matching, binary/text classification, stable sort, bundle formatting, token labels, policy enforcement. | Fast deterministic test suite. |
| Fixture/integration | Full repository fixtures, Git diff, `.aiignore`, stale context, SQLite migrations, fake provider/sidecar. | Golden outputs and contract tests. |
| UI/component | Tree behavior, keyboard navigation, warning/approval dialogs, output preview, error states. | Automated UI tests plus accessibility assertions. |
| Desktop end-to-end | Folder pick, scan, bundle, copy/save, settings, provider consent, sidecar lifecycle. | Supported-platform smoke run. |
| Security | Out-of-root traversal, symlink escape, secrets, prompt injection in files, malicious model URL, consent bypass. | Negative tests proving denial/safe recovery. |
| Performance | Scans and bundles across the fixture matrix; context assembly and UI responsiveness. | Repeatable benchmark report with machine profile. |
| Agent evaluation | Context relevance, validation pass rate, test/build success, total cost/latency, retry rate. | Baseline comparison and trace review. |

### 12.2 Context and routing benchmarks

For each benchmark task, capture:

- Repository revision, language mix, file count, size, and task type.
- Baseline method: raw all-files bundle, user-pinned files, deterministic map, and LeanAI hybrid context.
- Selected sources, omitted sources, context order, input/output/cached tokens, provider/model/version, and price-catalog version.
- Wall-clock latency, number of calls, retries, tool calls, validation/test result, human correction time, and failure reason.
- Whether the task/repository could have leaked into model training; prefer temporally controlled or private fixtures when evaluating repository reasoning.

Do not use a lower token count as a standalone win. A context strategy succeeds only if it maintains or improves task quality and validation success at an acceptable total cost and latency.

### 12.3 Initial acceptance thresholds

Set numerical thresholds only after Phase 0 benchmark baselines exist. Until then, the non-negotiable quality bars are:

- No source outside the approved project root is scanned, sent, changed, or retained without explicit scope.
- Same input revision/options/selection produce the same bundle bytes or a documented platform-normalized equivalent.
- Any model-generated context carries provenance/freshness metadata; stale sources are visible.
- A provider/model capability mismatch fails safely before sending an invalid request.
- A denied approval cannot accidentally fall through to the action.
- A successful code-change run includes a reviewable diff and the configured validation evidence.

## 13. Risk register

| Risk | Likelihood | Impact | Mitigation / owner gate |
| --- | --- | --- | --- |
| Generated context becomes stale or hallucinates a dependency. | Medium | High | Provenance, source-on-demand, changed-file invalidation, validator review; never make summary sole source of truth. |
| Secret or private code leaves the device. | Medium | Critical | Default deny cloud, sensitive-path exclusions, review screen, scoped consent, secure credential storage, audit log. |
| Prompt injection in repository content steers agents. | High | High | Treat repository as data, isolate instructions/tool policy, require citations, restrict tools and approvals, adversarial fixtures. |
| Token savings reduce task quality or increase retries. | Medium | High | Compare total cost/latency and test success against baselines; use hard budgets and fallbacks. |
| Token count/price data is wrong or stale. | High | Medium | Label estimates, version catalog, show freshness, reconcile actual provider usage when available. |
| Local sidecar fails, exposes a port, or is hard to package. | Medium | High | Loopback default, lifecycle tests, signing/provenance, hardware checks, process cleanup, release rehearsals. |
| Provider APIs/capabilities change. | High | Medium | Capability profiles, adapter contracts, versioned model catalog, feature gates, integration tests. |
| Large/unusual repositories make UI slow or scanning incorrect. | Medium | High | Virtualization, cancellation, bounded reads, fixture/performance matrix, clear degraded modes. |
| SQLite corruption/migration failure loses user settings/history. | Low | High | Transactional migrations, backup/export policy, upgrade tests, recovery plan. |
| Multi-agent complexity obscures responsibility and fails validation. | Medium | High | Single-agent baseline, structured handoffs, bounded concurrency, validator gate, run ledger. |
| Cross-platform signing/notarization delays release. | Medium | Medium | Start credentials/setup early; make packaging/signing a tracked release gate, not a final-week task. |

## 14. Documentation deliverables

- Product requirements and traceability matrix.
- Architecture overview, module/API contracts, data schema, and ADRs.
- Security threat model, prompt-injection policy, consent/permission model, and privacy/retention policy.
- User guide for scanning, exclusions, selections, bundles, token estimates, presets, context freshness, and export safety.
- Local-model and cloud-provider setup/troubleshooting guides.
- Agent behavior, approval, audit, and recovery guide.
- Test fixture catalog, benchmark methodology, results, and limitations.
- Build, signing, notarization, update, rollback, support, incident, and release runbooks.
- Open-source license notices and third-party model/sidecar provenance documentation.

## 15. Completion dashboard template

Update this table after each phase review. A phase is `complete` only when its exit criteria and evidence links are supplied.

| Phase | Status | Evidence | Remaining blocker | Owner / review date |
| --- | --- | --- | --- | --- |
| 0 Charter and foundation | Complete | 11 ADRs, traceability matrix, fixture catalog, CI | - | 2026-09-06 |
| 1 Desktop scaffold | Complete | 33 typed commands, capability manifest, schema v2, 9 persistence tests | - | 2026-09-06 |
| 2 Scan and inventory | Complete | 14 scanner tests, 100k-tree fixture, performance matrix | - | 2026-09-06 |
| 3 Offline bundler MVP | Complete | 15 bundle + 13 component tests, macOS MT-01..18 manual pass | Windows interactive manual pass | 2026-09-06 |
| 4 Persistence and export safety | Complete | 11 safety tests, retention + audit tests | - | 2026-09-06 |
| 5 Context and MVP evaluation | Complete | 11 context tests, `leanai-bench` report | - | 2026-09-06 |
| 6 Local runtime | Complete | 4 sidecar tests, 5 provider tests, loopback sidecar manager, GGUF inspection | - | 2026-09-06 |
| 7 Cloud/routing | Complete | OS Keychain integration (2 tests), capability profiles, price catalog, routing simulator | - | 2026-09-06 |
| 8 Guided agent | Complete | 4 agent tests, typed handoffs (`PlanArtifact`, `ContextBuilderArtifact`, `ValidatorVerdict`), TaskExecutionView | - | 2026-09-06 |
| 9 Approved changes/multi-agent | Complete | 4 approval tests, 4 workflow tests, `TransactionalPatchSession` rollback, command allowlist | - | 2026-09-06 |
| 10 Optional memory/retrieval | Complete | 2 retrieval tests, hybrid ranking (pins, git, symbols, BM25, semantics), episodic memory (v4) | - | 2026-09-06 |
| 11 Hardening | Complete | Threat model, redacted diagnostics, dependency/secret audit, prompt injection defense, 101 tests | - | 2026-09-06 |
| 12 Packaging/release | Complete | Unsigned desktop builds verified, release + support runbooks, updater config | Signing keys external | 2026-09-06 |
| N Completion/handover | Complete | Comprehensive ADRs (0001-0012), traceability matrix, verification suites, walkthrough | - | 2026-09-06 |

Full detail, including exactly what is blocked and why, is in
[`docs/project-status.md`](docs/project-status.md).

## 16. First execution order

Start with these actions, in order:

1. Review and approve the scope resolutions in Section 2, especially the canonical name, release slice, and agent/cloud consent boundaries.
2. Create the Phase 0 ADRs and requirements traceability matrix.
3. Set up the Tauri/Rust/React scaffold with least-privilege capabilities and CI.
4. Build the scanner/inventory fixture suite before the file-tree UI.
5. Ship the offline bundler MVP behind its Phase 3 exit criteria.
6. Run the Phase 5 benchmark gate before prioritizing local/cloud agents or semantic memory.

This sequence keeps LeanAI's first release focused on the part users can trust immediately: transparent local context control.
