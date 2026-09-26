# ADR 0006: App data location, retention defaults and what is never stored

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** NFR 7.1, FR-12, backlog 0.2, 4.7

## Context

LeanAI handles the user's source code. Anything it retains becomes a second
copy of that code, in a location the user did not choose, that outlives the
action that created it.

## Decision

1. All app data lives in the OS application-data directory reported by Tauri
   (`~/Library/Application Support/dev.leanai.desktop` on macOS,
   `%APPDATA%\dev.leanai.desktop` on Windows) in a single SQLite file,
   `leanai.sqlite3`, in WAL mode.
2. **Default retention is metadata-only.** A bundle's history row keeps the
   output hash, counts, estimate and the file list — not the bundle text. The
   user may opt into `full_text`, or into keeping nothing.
3. Credentials are never stored in SQLite. The schema has no column for them,
   and a test (`no_table_stores_credentials`) fails the build if one appears.
   API keys belong in OS secure storage (ADR 0007).
4. Telemetry is off until opted in and may never contain source text, bundle
   text, file paths or credentials.
5. The diagnostic bundle contains versions, capability state and counts only.
   OS error strings are stripped of absolute paths before they are stored or
   displayed.
6. "Forget project" removes LeanAI's own records and says exactly what was
   removed, including that no file in the user's project was touched.

## Consequences

- History is less useful by default (you cannot re-copy an old bundle) in
  exchange for not accumulating copies of the user's code.
- One SQLite file makes backup, export and deletion a single, explainable
  operation.

## Alternatives considered

- **Default to storing bundle text for convenience.** Rejected: the convenience
  is small and the retained data is the user's entire selected source.
