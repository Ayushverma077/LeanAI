# ADR 0004: Every token number carries its provenance label

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-13, FR-14, FR-16, Instructions.md §2

## Context

LeanAI counts tokens locally with `cl100k_base`. That is the OpenAI BPE. It is
not how Anthropic, Google or a GGUF model tokenises, and the difference is not a
rounding error — it varies by language, code style and whitespace. Presenting a
bare number invites a user to plan a budget against a figure that will not match
their bill.

## Decision

1. A token figure is never rendered without its label. The label is produced by
   the backend (`EstimateKind::label`), not composed in the UI.
   - Local: `estimate · cl100k_base · OpenAI-family`
   - Provider count: `exact · <provider>/<model>`
   - Failure: `unavailable`
2. `EstimateKind` is a tagged enum that travels with the value through the
   bundle, the manifest, the database and the export preflight. There is no code
   path that produces a number without a kind.
3. Every bundle manifest whose estimate is not `provider_exact` carries the
   warning `token value is a local OpenAI-family estimate, not a provider count`.
4. An exact provider count is requested only after explicit consent and only
   where a provider offers a counting endpoint. When it is unavailable, LeanAI
   falls back to the estimate and says so; it never silently substitutes one for
   the other.
5. No product surface states a savings percentage until the benchmark gate in
   backlog 5.9 has produced repeatable results.

## Consequences

- The UI is slightly noisier: every number has a caption. That is the point.
- Adding a provider means adding a capability profile that declares whether it
  can count exactly, rather than assuming parity.

## Alternatives considered

- **Show a single "approximate" badge in a corner.** Rejected: users read the
  number, not the corner.
- **Estimate per model family with correction factors.** Rejected: a fabricated
  factor is a worse lie than an honestly labelled OpenAI count.
