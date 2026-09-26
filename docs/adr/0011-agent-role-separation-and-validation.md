# ADR 0011: Structured multi-agent handoffs, deterministic validation, and transactional patch application

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-26 to FR-30, NFR 7.1-7.4, backlog 8.x, 9.x

## Context

Autonomous multi-agent systems often suffer from cascading hallucination, accidental secret exposure, out-of-scope edits, and broken state when a multi-file patch is partially applied. Per ADR 0010, agents operate in read-only mode until a scoped approval is granted. However, even with an approval mechanism, how roles communicate, how output is validated, and how file modifications are applied must be strictly deterministic and fail-safe.

## Decision

1. **Explicit Role Separation & Typed Handoffs:**
   - `Orchestrator`: Preflight checks (objective, project boundaries, privacy, token budget, tool capabilities).
   - `Planner`: Decomposes objective into structured subtasks with explicit definitions of success and target file scopes.
   - `ContextBuilder`: Selects relevant source files and builds a citation manifest with SHA-256 hashes and line ranges.
   - `Coder`: Generates unified diff proposals without modifying working tree files.
   - `Validator`: Evaluates the proposal against deterministic rules (zero leaked secrets, no path traversal outside project root, no hallucinated citations, within budget).
   - `Tester`: Executes commands only if present in the project allowlist.
   Handoffs between roles are schema-validated Rust structs, never free-form model text.

2. **Deterministic Validator as Gatekeeper:**
   - The Validator runs mechanically (compiled Rust algorithms) before any human approval prompt is surfaced.
   - If the patch touches blocked paths, contains potential secrets, or references non-existent files, validation fails immediately and no approval request is generated.

3. **Transactional File Modification (`TransactionalPatchSession`):**
   - File modification is atomic and transactional.
   - Prior to applying diffs, original contents of all affected files are captured in memory.
   - If any patch fails to apply or the user revokes/denies the action, an immediate rollback restores every file to its exact original byte state.
   - The working tree is never left in a partially modified state.

4. **Command Allowlist Policy:**
   - Tester command execution is restricted to an explicit project command allowlist.
   - Chained operators (`&&`, `||`, `;`, `|`, `` ` ``), subshells, and unlisted binaries are rejected at the parser level.

## Consequences

- Zero risk of unreviewed file corruption or secret leakage from model hallucinations.
- Diffs are always previewable in unified format before application.
- Clear audit trail in SQLite `runs`, `run_events`, and `approvals` tables.
