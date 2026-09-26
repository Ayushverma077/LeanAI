# ADR 0010: Agents are read-only until a scoped, expiring approval says otherwise

- **Status:** Accepted (implementation deferred to Phases 8-9)
- **Date:** 2026-09-06
- **Relates to:** FR-26 to FR-30, NFR 7.1, backlog 8.x, 9.x

## Context

Repository content is untrusted input. A README, a comment, a test fixture or a
dependency's source can contain text addressed to a model. If an agent can write
files, run commands or make network calls, that text becomes a way to make it do
so. The risk register rates prompt injection *high likelihood, high impact*.

## Decision

1. **Repository content is data, never instruction.** Tool policy and system
   constraints are supplied out-of-band and are not overridable by anything read
   from the project.
2. Every run starts read-only. Writing a file, running a command, browsing the
   web or calling an external API each require a separate approval naming the
   exact capability and scope.
3. An approval is scoped (project root plus specific paths or a specific patch),
   expiring, revocable and single-use by default. It is recorded in the
   `approvals` table with the decision and who made it. A denied approval can
   never fall through to the action.
4. Tester execution uses a per-project command allowlist. Commands do not
   inherit ambient shell or network privileges.
5. A run cannot enter `Running` without a known provider, capability profile,
   project scope, tool permission set and budget. It cannot report success
   without a validator evidence record.
6. Structured handoffs only: roles exchange schema-validated artifacts with
   citations to files or tool output, not free-form text that a later role
   treats as fact.
7. Acceptance tests include adversarial fixtures: prompt injection in source
   files, secrets in context, unsupported tool calls, malformed model output,
   stale context and cancellation.

## Consequences

- The first agent release does less than a fully autonomous agent, deliberately.
  The permission model has to earn trust before Phase 9 grants writes.
- The audit tables (`runs`, `run_events`, `approvals`) already exist in schema
  v2, so export auditing today uses the same ledger agents will use later.

## Alternatives considered

- **Blanket "allow writes in this project" toggle.** Rejected: it converts one
  injected instruction into arbitrary repository modification.
