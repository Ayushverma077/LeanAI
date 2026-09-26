# ADR 0012: Hybrid context retrieval and episodic task memory

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-10, FR-12, FR-29, FR-30, backlog 10.x

## Context

When developers give high-level task descriptions (e.g., "Refactor the BPE tokenizer to reduce allocations"), naive whole-project context stuffing consumes excessive tokens and risks prompt pollution. Conversely, pure vector embedding search requires heavy local neural model execution or external embeddings that violate zero-egress privacy principles.

## Decision

1. **Hybrid Deterministic + BM25 + Episodic Retrieval:**
   Task context retrieval utilizes a layered hybrid scoring function that operates entirely offline:
   - **User Pins (`score = 1.0`):** Explicit developer selections always take top priority.
   - **Git Working Tree Changes (`score = 0.8`):** Modified and staged files relevant to active development are prioritized.
   - **Symbol & Path Matches (`score = 0.6`):** Identifiers extracted from the task query matched against file names and directory trees.
   - **BM25 & Content Term Matches (`score = 0.5`):** Keyword frequencies in cached text representations.
   - **Semantic & Keyword Matches (`score = 0.4`):** Normalized similarity scoring.

2. **Zero-Egress by Default:**
   All retrieval and ranking algorithms are local and deterministic. No embeddings are sent to third-party endpoints unless explicitly opted-in by the developer with a configured cloud provider.

3. **Episodic Task Memory:**
   - Successful task executions record structured entries into `project_memory` (`id`, `task_summary`, `relevant_paths`, `key_findings`, `created_at_ms`, `expires_at_ms`).
   - Memories carry a configurable Time-To-Live (default 30 days) to prevent stale architectural assumptions from lingering.
   - Subsequent tasks query episodic memory to recall prior decisions and relevant file groupings without storing full conversational transcripts (per ADR 0006).

4. **Graceful Fallback:**
   If retrieval is disabled or encounters unindexed projects, the system gracefully falls back to explicit user-pinned files without crashing or halting the workflow.

## Consequences

- Token budgets are conserved by prioritizing high-signal files.
- Privacy and offline autonomy are preserved with zero external network dependencies.
- Past discoveries inform future agent tasks within the project.
