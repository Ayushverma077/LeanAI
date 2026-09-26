# ADR 0001: LeanAI Desktop is the canonical name

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** Instructions.md §2, backlog 0.1

## Context

The source material uses three names. The presentation cover says **CARE-SWE**
while its own product pages say **LeanAI**; the architecture proposal is written
for **PolyAgent Desktop**. Shipping under three names would split the app
identifier, the update feed, the support channel and the user's mental model.

## Decision

**LeanAI Desktop** is the product, repository, package and release name.

- Rust workspace: `leanai-core`, `leanai-desktop`.
- Bundle identifier: `dev.leanai.desktop`.
- Output format identifiers: `leanai.bundle/v1`, `leanai.context/v1`.
- Ignore file: `.aiignore`.

**PolyAgent** is treated as the predecessor design; its document remains a
reference for architecture but not for naming. **CARE-SWE** is recorded as an
academic-presentation title only. Neither appears in shipped UI, binaries or
release artifacts.

## Consequences

- One identifier means the updater, code signature and app-data directory stay
  stable across releases; changing any of them later would strand user data.
- Existing research documents keep their original names in `research/`, so the
  provenance of the design is still traceable.

## Alternatives considered

- **Keep PolyAgent for the engine, LeanAI for the UI.** Rejected: two names for
  one binary confuses support and telemetry for no benefit.
- **Adopt CARE-SWE.** Rejected: it appears only on a cover slide and describes
  the coursework, not the product.
