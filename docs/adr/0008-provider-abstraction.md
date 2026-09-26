# ADR 0008: Providers are described by capability profiles, not assumed equal

- **Status:** Accepted (implementation deferred to Phases 6-7)
- **Date:** 2026-09-06
- **Relates to:** FR-21, FR-22, FR-23, FR-25, backlog 6.1, 7.1

## Context

The source material lists `rig-core` as a possible model/agent abstraction and
contains a language inconsistency about it. Adopting a framework before the
boundary is understood couples LeanAI's safety model to someone else's release
cadence. Separately, cloud and local models genuinely differ: streaming, tool
calling, structured output, vision and token counting are not universal.

## Decision

1. LeanAI defines its own provider trait and a `CapabilityProfile` describing,
   per model: context cap, streaming, tool calling, structured output/JSON
   schema, vision, exact token counting, and cached-token policy.
2. A request that needs a capability the profile does not declare fails
   **before** any network call, with a message naming the missing capability.
   LeanAI never discovers a limitation from a 400 response.
3. A third-party agent framework may be adopted later **behind** this trait, and
   only after contract tests pass against it. It never becomes the boundary
   itself.
4. Routing decisions use task class, capability profile, budget, privacy policy,
   availability and past outcomes — never a model name alone. Every route
   decision and its actual result are logged.
5. The model/price catalog is versioned and timestamped, and the UI shows when
   it is stale. A catalog is never assumed current forever.
6. Model downloads use an explicit allowlist of source URLs, support resume and
   cancel, and verify a checksum or signature where one is published. License
   and provenance are displayed before activation.

## Consequences

- More code than adopting a framework directly, and it is the code that carries
  the safety guarantees.
- Adding a provider is a well-defined task: implement the trait, declare the
  profile, pass the contract tests.

## Alternatives considered

- **Adopt `rig-core` as the primary abstraction now.** Rejected pending a
  current-API review; the source material's description of it is inconsistent,
  and Phases 6-7 are not on the MVP critical path.
