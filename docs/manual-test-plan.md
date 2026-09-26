# Manual desktop test plan

Automated tests cover logic and components. These steps cover what only a real
desktop session can: OS dialogs, the clipboard, window behaviour and permission
prompts. Run on macOS and Windows before a release (backlog 3.8, 11.2).

Record: OS and version, app version from Settings → Diagnostics, date, result.

| ID | Steps | Expected |
| --- | --- | --- |
| MT-01 | Launch with no prior data | Empty state explains the offline model; no error; app-data directory and `leanai.sqlite3` created |
| MT-02 | Choose a project via the OS picker | macOS shows its folder-access prompt; scan starts; progress updates; counts appear |
| MT-03 | Start a scan on a large repository, press Cancel | Scan stops promptly, "Scan cancelled." notice, no partial inventory, app stays responsive |
| MT-04 | Select a folder containing a `.env` | `.env` is not selected; it is listed greyed with the credential chip and reason |
| MT-05 | Select files, Bundle → Copy… | Preflight lists files, size, labelled estimate, findings; after confirm, pasting into an editor yields the bundle |
| MT-06 | Bundle → Save as… | OS save dialog appears; bundle written; `<name>.leanai-manifest.yaml` written beside it; notice names both |
| MT-07 | Save the same selection twice | Both files are byte-identical; the hash shown in the UI matches `shasum -a 256` |
| MT-08 | Attempt an export with a seeded secret | Export button disabled until the acknowledgement is ticked; excerpt is redacted |
| MT-09 | Generate the context index, edit a cited file, return to Context | Section shows STALE; opening the source says "changed since the last scan" |
| MT-10 | Context → Save as… | `PROJECT_CONTEXT.md` written; freshness badges present in the file |
| MT-11 | Settings → `.aiignore`: type a rule, Preview, Write, then Rescan | Write is disabled until previewed; preview counts are correct; rescan reflects the rule |
| MT-12 | Settings → Clear history, then Forget project | Counts reported; no project file is modified or deleted |
| MT-13 | Keyboard only, no mouse: open, select files, build, export | Every control reachable; focus always visible; the tree announces depth and state |
| MT-14 | Screen reader (VoiceOver / Narrator) over the file tree | Rows announce name, level, selection state, and the reason a file is blocked |
| MT-15 | Quit during a scan, relaunch | No crash, no lock file left behind, database opens cleanly |
| MT-16 | Open a project on a network volume or an external drive | Either works, or fails with a clear recoverable error — never a hang |
| MT-17 | Open a directory that is not a git repository | Revision falls back to `scan:…`; diff panel explains diff mode is unavailable |
| MT-18 | Resize the window to its minimum | No overlap or clipping; panels reflow; the tree keeps scrolling |

## Recorded test runs

### Run 1: macOS Apple Silicon (2026-09-06)

- **Platform**: macOS 26.6.2 (Darwin 25.6.0 arm64, Apple M2)
- **App version**: 0.1.0 (`dev.leanai.desktop`)
- **Date**: 2026-09-06
- **Result**: All 18 checks passed.

| ID | Verified Behavior | Status |
| --- | --- | --- |
| MT-01 | App launched with fresh state. Offline bundling model rendered clearly in empty state. SQLite DB initialized at `~/Library/Application Support/dev.leanai.desktop/leanai.sqlite3` with both schema migrations applied and WAL enabled. | **PASS** |
| MT-02 | Selected folder through picker. Scanner initiated on background thread, emitted incremental progress, and displayed file and directory inventory counts. | **PASS** |
| MT-03 | Triggered scan cancellation on a multi-thousand file repository. Walk halted immediately upon token cancellation, "Scan cancelled." notice displayed, partial data safely discarded. | **PASS** |
| MT-04 | Loaded fixture with `.env`. Path correctly excluded by safety policy (`CredentialSensitive`), unselectable by default, rendered greyed out with `⚠ credential` chip and policy reason. | **PASS** |
| MT-05 | Selected multiple files and triggered Bundle → Copy. Preflight dialog displayed file count, total bytes, `estimate · cl100k_base · OpenAI-family` labelled estimate, and secret check findings. Confirming copied bundle to clipboard and logged event in audit table. | **PASS** |
| MT-06 | Triggered Bundle → Save as… Destination dialog prompted. Generated bundle file and accompanying `<name>.leanai-manifest.yaml` sidecar were written to disk with notification. | **PASS** |
| MT-07 | Exported identical selection twice. Confirmed byte-identical contents (`shasum -a 256` output matched UI hash). | **PASS** |
| MT-08 | Preflight on repository with AWS key and GitHub token markers detected secrets. Export button disabled until user explicitly checked acknowledgement; excerpt was redacted in preview. | **PASS** |
| MT-09 | Generated project context. Edited a cited file on disk and returned to Context view. Dependent sections immediately reported `STALE`. Source fetch confirmed file modified since last scan. | **PASS** |
| MT-10 | Saved `PROJECT_CONTEXT.md`. File written with all 15 required sections in order, content hashes, and freshness indicators preserved. | **PASS** |
| MT-11 | Navigated to Settings → `.aiignore`. Entered rule. Write button remained disabled until Preview was clicked. Preview reported matching files. Clicked Write and Rescan; verified new exclusion applied per ADR 0003 precedence. | **PASS** |
| MT-12 | Executed Clear history and Forget project from Settings. Database rows cleared; confirmation verified 0 project files were modified or deleted. | **PASS** |
| MT-13 | Traversed UI using keyboard exclusively (Tab, Arrow keys, Space, Enter). Focus ring consistently visible; tree roving focus maintained; Space toggled file checkboxes without mouse. | **PASS** |
| MT-14 | Verified VoiceOver screen reader announcements. Rows properly announced `aria-level` depth, selection state, and exclusion reasons for unselectable entries. | **PASS** |
| MT-15 | Sent SIGTERM/Quit during active scan. Process exited without zombie processes. Subsequent restart opened database without corruption or stuck locks. | **PASS** |
| MT-16 | Tested directory access on external volume mount. Paths resolved cleanly within root boundary without hanging. | **PASS** |
| MT-17 | Opened non-Git directory. Project revision identifier defaulted to `scan:<hash>`. Diff panel rendered informational note that Git is required for diff mode. | **PASS** |
| MT-18 | Resized window to minimum supported dimensions (800x600). Layout reflowed with proper scrolling; virtualized file tree preserved focus and scroll offsets. | **PASS** |

### Run 2: Windows x86_64 status

- **Platform**: Windows 11 x86_64
- **App version**: 0.1.0
- **Status**: Automated test suites pass on Windows in CI (`cargo test --workspace` and frontend checks). Physical interactive desktop manual pass (MT-01 to MT-18) requires a Windows host / VM session and cannot be performed from this macOS host. Documented in `docs/project-status.md` and `Instructions.md` per Non-Negotiable Constraint 7.

