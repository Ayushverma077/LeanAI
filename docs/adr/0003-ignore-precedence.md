# ADR 0003: Ignore-rule precedence, and `.aiignore` cannot re-expose a secret

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-02, FR-03, backlog 2.2, 4.2

## Context

The source material places `.aiignore` in both the MVP and Phase 2, and does not
say how it interacts with `.gitignore` or with LeanAI's own safety filters.
Gitignore syntax supports negation (`!pattern`), so precedence decides whether a
user's ignore file can *re-include* something LeanAI blocked.

## Decision

Rules are applied in this order; later wins.

| Rank | Source | Can hide files | Can re-include files |
| ---: | --- | --- | --- |
| 0 | global gitignore (`core.excludesFile`) | yes | yes, within git rules |
| 1 | `.git/info/exclude` | yes | yes, within git rules |
| 2 | `.gitignore`, nearest-first | yes | yes, within git rules |
| 3 | `.aiignore`, nearest-first | yes | yes, within git rules |
| 4 | **LeanAI safety policy** | yes | **no — overrides all of the above** |
| 5 | explicit per-file user override in the UI | — | yes, one file at a time |

`.aiignore` uses gitignore syntax and is layered above `.gitignore`, so it can
exclude more. It **cannot** unlock a path the safety policy blocks: a
`!.env` line in `.aiignore` still leaves `.env` unselectable, because
classification happens after ignore matching and is not expressible as a glob
negation.

The only way to include a blocked file is rank 5: a per-file confirmation in the
UI where the user re-reads the reason and types a confirmation phrase. A
folder-level "select all" never reaches rank 5.

`.aiignore` is previewed before it is written: the user sees exactly which files
it would newly exclude, which rules matched nothing, and which rules are
invalid.

## Consequences

- A user can make LeanAI stricter with a file in their repo, but never looser in
  a way that matters for secrets.
- Precedence is displayed in Settings, so the answer to "why is this file
  missing?" is available without reading logs.

## Alternatives considered

- **Treat `.aiignore` as the highest-priority source.** Rejected: it makes a
  committed file in a shared repository able to unlock every teammate's
  credential exclusions.
- **Apply `.aiignore` below `.gitignore`.** Rejected: a user could not exclude
  something git tracks, which is the main reason to want the file.
