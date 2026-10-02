# LeanAI changelog

A dated record of every change to LeanAI, newest first. Each entry says what
changed, why, where, and how it was verified, so any line of the product can be
traced back to the day it arrived.

## How to read and add entries

- **Times are India Standard Time (IST, UTC+05:30)**, 24-hour clock, the same
  zone as the Git history.
- For committed work, the time is the **commit time** from `git log` and the
  entry names the commit hash. For work that was not yet committed when it was
  logged, the time comes from file timestamps and the entry says
  *uncommitted*; add the hash once it is committed.
- **Every change gets an entry before it is committed**, using this template:

  ```markdown
  ## YYYY-MM-DD HH:MM IST — Short title

  - **Commit:** `abc1234` (or *uncommitted*)
  - **What:** the change, in plain words.
  - **Why:** the problem it solves or the decision behind it (link the ADR).
  - **Files:** the main files touched.
  - **Verified:** the tests or checks that prove it, with their result.
  ```

- The feature overview in `LEANAI_COMPLETE_CHANGELOG.md` and the phase table
  in `docs/project-status.md` summarise; this file is the timeline.

---

## 2026-09-27 15:17 IST — A conversation workspace for real tasks

- **Commit:** *uncommitted*
- **What:** Replaced the Ask/advanced-console split with project-scoped task
  conversations, new-task controls, retained drafts/results, bounded follow-up
  context, backend activity events, elapsed time, inline errors, change review,
  and allowed test commands with real output. Removed the sample agent console.
  Conversations remain available while navigating and last until app exit.
- **Why:** Tasks should run useful work through the model backend and support
  follow-ups instead of displaying a simulated agent trace.
- **Safety:** Prior requests inform context needs without granting edit intent
  to a follow-up. Pending changes require resolution before continuing. Project
  changes during execution and mismatched approval roots are rejected.
- **Files:** `src/pages/TasksPage.tsx`, `src/components/agents/PromptRunner.tsx`,
  `src/store/useTaskStore.ts`, IPC types/client, prompt and approval commands,
  prompt tests, and `docs/user-guide.md`.
- **Verified:** Typecheck, targeted ESLint and production Vite build passed;
  19 frontend tests and 12 backend prompt tests passed. Checked the responsive
  Tasks layout in the browser; backend runs used local mock model servers.

## 2026-09-27 14:29 IST — Remove the Agents section

- **Commit:** *uncommitted*
- **What:** Removed the Agents page, sidebar entry, command-palette shortcut,
  and Agent Fleet button, along with the page's unused roster and graph components.
- **Why:** The standalone Agents section is no longer wanted in the interface.
- **Files:** `src/App.tsx`, `src/store/useAppStore.ts`, navigation components,
  `src/components/context/CompressionHero.tsx`, removed Agents page/components,
  and `docs/architecture.md`.
- **Verified:** TypeScript typecheck passed; all 6 store tests passed;
  no references to the removed route or components remain in `src`.

## 2026-09-27 12:17 IST — Plainer "What the AI gets" header and project map controls

- **Commit:** *uncommitted*
- **What:** The top of the Context page was reworded for people who don't
  know the jargon.
  - The header now reads **What the AI gets** and names the parts
    ("Project map + 54 files").
  - The size is shown in words, with one line of advice: **Small** (up to
    30k tokens, "fits in almost any AI chat"), **Medium** (up to 120k, "fits
    in most AI chats; some free plans may cut it off") or **Large** ("too big
    for many AI chats. Pick fewer files").
  - The token count is rounded ("~96k"). Hovering shows the exact estimate
    and its label (`estimate · cl100k_base · OpenAI-family`). KB is no longer
    shown in the header.
  - **Copy** became **Copy for AI** and is now the page's main (blue) button.
  - The map card drops the file name and token count from its title row. Its
    freshness reads **Up to date**, **Out of date** or **Can't check** instead
    of fresh/stale/unknown.
  - **Include** now sits on the map card itself, and **Regenerate** became
    **Refresh**.
  - A one-line explanation opens the card: "A short summary of your project.
    The AI reads it first, before your files. It is also saved in your
    project as PROJECT_CONTEXT.md."
  - The generated file's stale note now says "Refresh" too.
- **Why:** The owner asked to make this part simpler to use for everyone.
- **Files:** `src/pages/ContextBundlerPage.tsx` (`sizeGuide`, `shortNumber`,
  `FRESHNESS_WORDS`), `src/pages/ContextBundlerPage.test.tsx`,
  `crates/leanai-core/src/context.rs`, `docs/user-guide.md`,
  `docs/adr/0015-one-view-for-project-map-and-files.md` (update note).
- **Verified:** `npm run check:all` green: 170 Rust tests and 67 frontend
  tests (1 new, for the size wording; 3 updated for the new labels), plus
  typecheck, ESLint, `cargo fmt --check` and `cargo clippy -D warnings`. The
  app itself was not launched in this session.

## 2026-09-27 12:10 IST — Go to Tasks once the project map is ready

- **Commit:** *uncommitted*
- **What:**
  - **Opening or cloning a project** now runs: scan, then build the project
    map (or reuse it if it is fresh), then open **Tasks → Ask**. It used to
    land on the Context page straight after the scan. While the map builds,
    the Overview page shows "Building the project map… Tasks opens when it is
    ready" and the top-bar refresh icon spins.
  - **Regenerate** on the Context page, and **Yes, replace it** in the
    existing-file alert, also open Tasks when they finish.
  - LeanAI stays on the **Context** page when the existing `PROJECT_CONTEXT.md`
    needs a decision, so the alert is seen. It never moves you if you switched
    pages while the map was building, and if the map can't be built it falls
    back to the Context page.
  - Rebuilding a stale map in the background while you browse the Context
    page does *not* switch pages.
  - Map generation in the backend is now serialised by a lock
    (`AppState::context_gate`), and writing text identical to what is already
    in the file counts as safe, so two builds finishing close together can't
    raise a false "edited" alert.
- **Why:** The owner asked to move to Tasks when map generation completes, and
  chose to include opening a project.
- **Files:** `src/store/useAppStore.ts` (`prepareMapAndContinue`,
  `regenerateMap`, `preparingMap`, `scan({ stayOnPage })`),
  `src/pages/ContextBundlerPage.tsx`, `src/pages/OverviewPage.tsx`,
  `src/components/layout/TopBar.tsx`, `src-tauri/src/app_state.rs`,
  `src-tauri/src/commands/context.rs`, `crates/leanai-core/src/context_file.rs`,
  tests (`src/store/useAppStore.test.ts` new with 6 tests, 1 new page test),
  `docs/user-guide.md` (First run, step 4).
- **Verified:** `npm run check:all` green: 170 Rust tests and 66 frontend
  tests, plus typecheck, ESLint, `cargo fmt --check` and `cargo clippy -D
  warnings`. The app itself was not launched in this session.

## 2026-09-27 12:00 IST — Alert when PROJECT_CONTEXT.md already exists; never overwrite it silently

- **Commit:** *uncommitted*
- **What:** Before writing `PROJECT_CONTEXT.md`, LeanAI now checks what is
  already there. It writes without asking only when the file is missing or
  exactly what LeanAI last wrote (compared by SHA-256). Otherwise it leaves
  the file alone, keeps its map inside the app, and the Context page shows an
  alert:
  - **Written by someone else, or edited since:** **Replace file** (then
    **Yes, replace it**) or **Keep mine**.
  - **Tracked by Git, a folder or link, or unreadable:** the alert explains
    why LeanAI won't write it; for a tracked file it gives the
    `git rm --cached` command.

  Automatic refreshes (after Ask LeanAI applies a change, or when the Context
  page builds the map) never replace a file. Generating the map no longer
  fails when the file is tracked by Git; the map is still stored for the app.
- **Why:** The owner asked for an alert when the file already exists. Before
  this change, any untracked `PROJECT_CONTEXT.md`, including a hand-written
  one, was overwritten silently on regenerate and on every automatic refresh.
  See [ADR 0016](adr/0016-never-overwrite-an-existing-project-context-file.md).
- **Files:** `crates/leanai-core/src/context_file.rs` (`FileState`,
  `file_state`, `SaveOutcome`), `crates/leanai-core/src/context.rs`
  (`FILE_MARKER`), `src-tauri/src/commands/context.rs` (`file_info`,
  `generate_context` takes `replaceExisting`), `src-tauri/src/commands/bundles.rs`
  (`projectMapFile`), `src/pages/ContextBundlerPage.tsx` (`ContextFileAlert`),
  `src/store/useAppStore.ts`, `src/ipc/{client,types}.ts`, tests, and
  `docs/adr/0016-never-overwrite-an-existing-project-context-file.md` (new),
  `docs/user-guide.md`, `docs/security-threat-model.md` (new T17; also
  renumbered the T14/T15 rows added at 11:19, which clashed with the existing
  T14, to T15/T16), `docs/traceability.md`.
- **Verified:** `npm run check:all` green: 170 Rust tests and 59 frontend
  tests, plus typecheck, ESLint, `cargo fmt --check` and `cargo clippy -D
  warnings`. New tests: 3 in `context_file::tests` (2 existing tests
  rewritten for the new behaviour);
  `prompt_run.rs::a_project_context_file_leanai_did_not_write_is_never_overwritten`,
  which would have failed before this change; 3 alert tests in
  `ContextBundlerPage.test.tsx`.

## 2026-09-27 11:51 IST — Project map and files merged into one view with one Copy

- **Commit:** *uncommitted*
- **What:** The Context page no longer has separate **Bundle** and
  **PROJECT_CONTEXT.md** tabs. One view, **Context for AI**, shows the project
  map on top and the selected files below. The header says what Copy will
  send ("Project map + 3 files · 42 KB · ~11,000 tokens"). **Include project
  map** is on by default. Copy works with only the map when no files are
  picked. The map is shown as readable text (bold, lists, code) instead of
  raw Markdown. Each section has a **Sources and limits** disclosure.
  - The map is part of the bundle itself: it is hashed, counted in the
    estimate, recorded in the manifest, secret-scanned in the export review
    and noted in the audit log.
  - Also fixed: the page rebuilt the preview in a loop because its effect
    depended on the whole store object.
- **Why:** The owner wanted both in one place. The Copy button on the
  PROJECT_CONTEXT.md tab used to copy only the bundle, and could not copy the
  map at all. See [ADR 0015](adr/0015-one-view-for-project-map-and-files.md).
- **Files:** `crates/leanai-core/src/concat.rs` (`build_with_map`,
  `include_project_map`), `crates/leanai-core/src/manifest.rs`,
  `src-tauri/src/commands/bundles.rs`, `src/pages/ContextBundlerPage.tsx`,
  `src/components/context/MiniMarkdown.tsx` (new), `src/store/useAppStore.ts`,
  `src/ipc/types.ts`, tests in `crates/leanai-core/tests/bundle.rs`,
  `src/pages/ContextBundlerPage.test.tsx` (new) and
  `src/components/context/MiniMarkdown.test.tsx` (new), and
  `docs/adr/0015-one-view-for-project-map-and-files.md` (new),
  `docs/user-guide.md`, `docs/traceability.md`.
- **Verified:** `npm run check:all` green: 167 Rust tests (3 new) and 56
  frontend tests (5 new), plus typecheck, ESLint, `cargo fmt --check` and
  `cargo clippy -D warnings`. The loop test
  (`ContextBundlerPage.test.tsx::builds once per change instead of looping`)
  was checked against the old code and fails there. The app itself was not
  launched in this session.

## 2026-09-27 11:36 IST — Shorter, plainer PROJECT_CONTEXT.md

- **Commit:** *uncommitted*
- **What:** `PROJECT_CONTEXT.md` now has 9 short sections (Overview,
  Structure, Key Modules, API Routes, Dependencies, How to Run, Configuration,
  Open TODOs, Decisions), not 15 long ones. Each item is one line, and long
  lists show the first items and count the rest. The per-section revision
  lines, "Limitations" blocks and hashed source lists are no longer printed in
  the file. They are still recorded for every section and shown on the
  Context page, and freshness checks work as before. Test files are left out
  of Key Modules and API Routes, TODOs count only when they start a comment,
  and the ADR template is no longer listed as a decision. The format is now
  schema version 2; an old stored index is regenerated automatically when the
  prompt runner needs it.
- **Why:** The file was hard to read: 1,343 lines (about 23,300 tokens) for
  this repository. It is now 114 lines (about 2,400 tokens), which also makes
  every prompt run's initial context about 10 times smaller. See
  [ADR 0014](adr/0014-short-project-context-file.md), which amends how
  [ADR 0005](adr/0005-context-provenance.md) is rendered.
- **Files:** `crates/leanai-core/src/context.rs`,
  `crates/leanai-core/src/routing.rs` (context levels use the new section
  keys), `src-tauri/src/commands/context.rs` (schema-version check),
  `src/ipc/types.ts`, `crates/leanai-core/tests/context.rs`,
  `src-tauri/tests/prompt_run.rs` (test harness retries when a parallel test
  takes its port; this intermittent failure showed up in this run),
  `docs/adr/0014-short-project-context-file.md` (new),
  `docs/adr/0005-context-provenance.md`, `docs/user-guide.md`,
  `docs/project-status.md`, `docs/traceability.md`,
  `docs/benchmark-methodology.md`.
- **Verified:** `npm run check:all` green: 164 Rust tests (1 new:
  `context.rs::rendered_file_is_short_and_plain`; 5 updated for the new
  section keys) and 51 frontend tests, plus typecheck, ESLint, `cargo fmt
  --check` and `cargo clippy -D warnings`. `prompt_run.rs` passed 5 runs in a
  row after the harness fix.

## 2026-09-27 11:19 IST — Documentation brought up to date with the app

- **Commit:** *uncommitted*
- **What:** Added this changelog with the full history reconstructed from Git.
  Documented the "Ask LeanAI" prompt runner, model providers and the apply gate
  in the user guide and architecture notes. Corrected statements that the app
  has no network access at all; that was true of 0.1 but not since model
  providers and GitHub cloning were added. Updated the threat model,
  traceability matrix, project status and README to match.
- **Why:** The docs still described LeanAI 0.1 ("does not talk to any AI
  provider", "has no agents"), which is no longer accurate.
- **Files:** `docs/CHANGELOG.md` (new), `docs/user-guide.md`,
  `docs/architecture.md`, `docs/security-threat-model.md`,
  `docs/traceability.md`, `docs/project-status.md`, `README.md`,
  `LEANAI_COMPLETE_CHANGELOG.md`.
- **Verified:** Documentation only; statements checked against the code
  (`llm_client.rs`, `commands/git.rs`, `capabilities/default.json`).

## 2026-09-27 11:19 IST — Risk-scaled apply gate and schema-constrained replies

- **Commit:** *uncommitted*
- **What:**
  - **Apply gate.** A validated change is now ranked *apply automatically*,
    *confirm* or *careful review*, with reasons, instead of one on/off switch.
    Changes to lockfiles, build/dependency manifests, CI, deployment,
    migration, ignore-rule and permission files, and changes proposed for a
    request that reads as a question, always get a careful review. Large
    changes (more than 3 files or 60 lines), sensitive requests (auth,
    sessions, payments) and runs where the model needed corrections wait for a
    normal review. A careful review can only be applied after ticking "I've
    read these changes". The auto-apply toggle is now "Apply low-risk changes
    automatically".
  - **Intent confidence.** Rescaled so 0 means evidence is evenly split and 1
    means unanimous (`(k·p − 1)/(k − 1)`). New `changeShare` measures how much
    of the evidence points at changing code.
  - **Schema-constrained replies.** Agent turns send a JSON schema for the
    model's action (`response_format` for OpenAI, llama-server and self-hosted
    servers; `output_config.format` for Anthropic), so replies cannot be
    malformed. Servers that reject the schema get one retry without it and a
    single warning, and are not sent it again during that run.
- **Why:** Idea taken from TypeSafe AI's Jev: typed outputs plus different
  confidence thresholds for acting and asking. Only the pattern was adopted,
  not the model. See [ADR 0013](adr/0013-risk-scaled-apply-gate-and-structured-replies.md).
- **Files:** `crates/leanai-core/src/apply_gate.rs` (new),
  `crates/leanai-core/src/routing.rs`, `crates/leanai-core/src/llm_protocol.rs`,
  `crates/leanai-core/src/lib.rs`, `src-tauri/src/commands/prompt.rs`,
  `src-tauri/src/llm_client.rs`, `src-tauri/src/commands/models.rs`,
  `src-tauri/tests/prompt_run.rs`, `src/components/agents/PromptRunner.tsx`,
  `src/components/agents/PromptRunner.test.tsx`, `src/ipc/types.ts`.
- **Verified:** `npm run check:all` green: 163 Rust tests (12 new) and 51
  frontend tests (1 new), plus typecheck, ESLint, `cargo fmt --check` and
  `cargo clippy -D warnings`. The baseline before the change, at 11:10, was
  151 Rust and 50 frontend tests, all green. New tests include
  `apply_gate::tests::*` (5),
  `prompt_run.rs::gate_holds_back_build_file_changes_even_with_auto_apply_on`,
  `prompt_run.rs::server_without_schema_support_falls_back_to_instructions_once`
  and `PromptRunner.test.tsx::shows why a change was held back and gates a
  careful review`.

## 2026-09-22 17:43–18:36 IST — One-box prompt runner, capability routing and self-hosted models

- **Commit:** *uncommitted* (times from file timestamps)
- **What:**
  - **"Ask LeanAI" prompt runner** (`commands/prompt.rs`,
    `PromptRunner.tsx`, Tasks page). One text box. LeanAI analyses the prompt
    locally, picks the cheapest capable model and sends only the
    `PROJECT_CONTEXT` sections the budget allows. The model then asks for files,
    line ranges or searches through a validated JSON action protocol
    (`llm_protocol.rs`). Edits are find/replace, turned into a hashed patch
    proposal and validated, then applied transactionally or held for approval.
    `PROJECT_CONTEXT.md` is refreshed after an apply.
  - **Prompt analysis and routing** (`routing.rs`, +1,680 lines). Intent,
    complexity, context requirement, required model strengths, context budget
    levels, and an escalation chain that is only used on concrete failure
    evidence.
  - **Model catalog** (`catalog.rs`). Tiers (fast, balanced, powerful),
    per-model strengths, and self-hosted models (`custom/…`) that cost nothing
    per token and are preferred when sufficient.
  - **Provider client** (`llm_client.rs`). Anthropic, OpenAI, the managed
    llama-server sidecar and self-hosted OpenAI-compatible servers (Ollama,
    vLLM, LM Studio). Short `Retry-After` waits are honoured and `<think>`
    reasoning blocks are stripped.
  - **Self-hosted models panel** on the Models page: add, test and remove a
    server by URL, with an optional key stored in the OS keychain.
  - **Approvals bound to diffs** (`approval.rs`, `commands/agent.rs`). Hunks
    must match the base text exactly, and an approval cannot be replayed
    against a different proposal.
  - **Context rendering for models** (`context::render_sections`) within a
    token ceiling; `ensure_project_context` / `refresh_project_context`.
  - Removed the committed 29,562-line `project_context/project_context.md`.
- **Why:** Move from bundling context for a model to running a request against
  one while sending the minimum context, with every edit reviewed or validated.
- **Files:** `src-tauri/src/commands/prompt.rs`, `src-tauri/src/llm_client.rs`,
  `crates/leanai-core/src/llm_protocol.rs`, `crates/leanai-core/src/routing.rs`,
  `crates/leanai-core/src/catalog.rs`, `crates/leanai-core/src/approval.rs`,
  `crates/leanai-core/src/context.rs`, `src-tauri/src/commands/{agent,context,models}.rs`,
  `src/components/agents/PromptRunner.tsx`,
  `src/components/SelfHostedModelsPanel.tsx`, `src/pages/{TasksPage,ModelsPage}.tsx`,
  `src/ipc/{client,types}.ts`, `src-tauri/tests/prompt_run.rs`.
- **Verified:** At 2026-09-27 11:10 IST, 151 Rust and 50 frontend tests were
  green, including 7 end-to-end prompt runs against a scripted local model
  (`prompt_run.rs`).

## 2026-09-22 17:18 IST — Project-local PROJECT_CONTEXT.md and GitHub clone progress

- **Commit:** `03aa46e` "changed md logic"
- **What:** `PROJECT_CONTEXT.md` is saved in the project automatically, with a
  Git exclusion installed first; tracked files need explicit user action
  (`context_file.rs`). Added clone progress reporting (`CloneProgressView`) and
  Git URL normalisation (`gitUrl.ts`). Reworked the Context Bundler, Context
  and Preview pages. Also committed a generated `project_context.md`, since
  removed.
- **Files:** `crates/leanai-core/src/context_file.rs`,
  `src-tauri/src/commands/{context,git}.rs`, `src-tauri/src/app_state.rs`,
  `src/components/git/*`, `src/pages/{ContextBundlerPage,ContextPage,PreviewPage}.tsx`.
- **Verified:** `CloneProgressView.test.tsx`, `gitUrl.test.ts`.

## 2026-09-09 21:32 IST — Save button fix

- **Commit:** `acb1347` "fix save button"
- **What:** Fixed saving from the Context Bundler page.
- **Files:** `src/pages/ContextBundlerPage.tsx`.

## 2026-09-09 21:21 IST — Theme, project switcher and folder picker

- **Commit:** `c8149d8` "add theme"
- **What:** Light and dark theme (`theme.ts`, `index.css`), `ProjectSwitcher`
  and `FolderPicker` components with tests, a token gauge, and bundler and
  selection updates in core (`concat`, `selection`, `policy`, `classify`,
  `walker`). Added `LEANAI_COMPLETE_CHANGELOG.md` and
  `UI_REDESIGN_SUMMARY.md`.
- **Files:** `crates/leanai-core/src/{concat,selection,policy,classify,walker}.rs`,
  `src/components/{ProjectSwitcher,FolderPicker}.tsx`,
  `src/pages/ContextBundlerPage.tsx`, `src/theme.ts`.
- **Verified:** `FolderPicker.test.tsx`, `ProjectSwitcher.test.tsx`,
  `bundle.rs`, `scan.rs`.

## 2026-09-07 00:54 IST — GitHub integration and new logo

- **Commit:** `a1bc127` "some ui changes"
- **What:** GitHub token storage in the OS keychain (`GitAuthSettings`),
  repository listing and cloning (`CloneRepoModal`), pushing changes
  (`GitSyncModal`), remote detection in `gitinfo.rs`, and new app icons, logo
  and sidebar.
- **Files:** `src-tauri/src/commands/git.rs`, `crates/leanai-core/src/gitinfo.rs`,
  `src/components/git/*`, `src/components/layout/Sidebar.tsx`, `src-tauri/icons/*`.
- **Verified:** `git_remote.rs`, `keychain.rs`.

## 2026-09-06 19:42 IST — Windows build fix

- **Commit:** `b27cb11` "windows exe fix"
- **What:** Windows fixes for keychain access and a tester test.
- **Files:** `src-tauri/src/keychain.rs`, `crates/leanai-core/tests/tester.rs`.

## 2026-09-06 19:27 IST — Phases 8–10: guided agents, approvals, retrieval

- **Commit:** `96b43e6` "ui fix"
- **What:** Typed agent handoffs and the deterministic validator
  (`agent.rs`), scoped approvals and `TransactionalPatchSession`
  (`approval.rs`), hybrid retrieval and episodic memory (`retrieval.rs`), the
  Tester role with a command allowlist, and `TaskExecutionView`. Added ADRs 0011
  and 0012.
- **Files:** `crates/leanai-core/src/{agent,approval,retrieval}.rs`,
  `src-tauri/src/commands/agent.rs`, `src-tauri/src/db/*`, `src/components/agents/*`.
- **Verified:** `agent.rs`, `approval.rs`, `retrieval.rs`, `tester.rs`,
  `agent_workflow.rs`.

## 2026-09-06 14:48 IST — Phases 6–7: local runtime, providers and routing

- **Commit:** `15338e5` "ui"
- **What:** llama-server sidecar lifecycle with GGUF inspection, OS keychain
  credential storage, provider abstraction, price catalog and cost-aware
  routing, and the Models page.
- **Files:** `crates/leanai-core/src/{catalog,gguf,provider,routing}.rs`,
  `src-tauri/src/{keychain,sidecar_manager}.rs`,
  `src-tauri/src/commands/models.rs`, `src-tauri/src/db/migrations.rs`.
- **Verified:** `provider.rs`, `keychain.rs`, `sidecar.rs`.

## 2026-09-06 13:49 IST — Phases 0–5: foundation, scanner, bundler, context index

- **Commit:** `1d98fba` "main"
- **What:** The `leanai-core` crate (policy, walker, classify, inventory,
  selection, concat, manifest, tokenizer, secrets, gitinfo, aiignore, symbols,
  context, benchmark), the Tauri app with typed commands and SQLite
  migrations, the React UI, CI, and ADRs 0001–0010 with the full `docs/` set.
- **Files:** 116 files, including `crates/leanai-core/*`, `src-tauri/*`,
  `src/*`, `docs/*`, `.github/workflows/ci.yml`, `Instructions.md`.
- **Verified:** Core, scan, bundle, safety, context and persistence test
  suites.

## 2026-09-06 10:50–10:57 IST — Repository created

- **Commits:** `787f576` "first commit" (10:50), `5b544bb` "md file" (10:53),
  `25b6d0c` "Stop tracking research and tmp" (10:55), `ae0ae8c` "fix name"
  (10:57).
- **What:** README, the 0-to-N project plan and the PolyAgent research
  catalog. Research and temporary files were then untracked.
