# ADR 0005: The context index is a cache with provenance, never the source of truth

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-17, FR-18, FR-19, FR-20, Instructions.md §2

## Context

A generated project summary drifts. If an agent or a developer treats it as
authoritative, they act on code that no longer exists. The source material
positions `PROJECT_CONTEXT.md` as a headline feature, which makes the failure
mode more likely, not less.

## Decision

1. Every section records the files it was generated from, each with the
   SHA-256 of the file at generation time, plus the revision and timestamp.
2. Freshness is computed on read, not stored as a claim: opening the context
   page re-hashes against the current inventory and relabels each section
   `fresh`, `stale` or `unknown`.
3. Invalidation is conservative. If impact cannot be proven, the section is
   stale. Adding *any* file marks the structural sections stale, because a new
   file can invalidate a structural claim without changing a cited file.
4. Every section states what it could not determine. Silence never implies
   absence.
5. Source-on-demand is a first-class command: from any citation the user can
   fetch the current bytes, and the response says whether the file changed since
   the scan the claim was made against.
6. Sections are written by ordinary deterministic code. A model-written section
   is allowed only behind explicit provider consent, is labelled with provider,
   model and prompt version in the rendered document, and is rejected by
   `validate_model_section` unless every citation exists and still hashes to the
   recorded value.

## Consequences

- The document is longer and less confident-sounding than a marketing summary.
  That reflects what it actually knows.
- Regeneration is cheap and deterministic, so a stale document is a prompt to
  regenerate rather than a crisis.

## Alternatives considered

- **Store freshness as a flag updated by a file watcher.** Rejected: a missed
  event produces a document that claims to be fresh and is not. Recomputing on
  read cannot go stale silently.
