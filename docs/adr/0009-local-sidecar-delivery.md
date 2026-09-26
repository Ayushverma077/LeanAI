# ADR 0009: Local inference runs as a loopback sidecar, verified before activation

- **Status:** Accepted (implementation deferred to Phase 6)
- **Date:** 2026-09-06
- **Relates to:** FR-21, FR-22, NFR 7.1, 7.2, backlog 6.2-6.7

## Context

Local inference means shipping or fetching a native binary and running it as a
child process. Done carelessly this adds a network listener, an unsigned
executable and an orphan process to a desktop app whose main promise is safety.

## Decision

1. `llama-server` is packaged per supported architecture as a Tauri sidecar with
   recorded version, checksum, signature where available, and license metadata.
   It is covered by the app's own code signature and notarization.
2. The sidecar binds to **loopback only**, on an ephemeral port chosen at start.
   Exposing it on a network interface requires a deliberate setting and a
   warning; there is no default that listens beyond `127.0.0.1`.
3. Lifecycle is an explicit state machine: preflight (disk, memory, model
   compatibility) → spawn → health check → ready → stop, with recovery from
   unexpected exit. Ports in use, corrupt GGUFs, incompatible context settings
   and invalid paths are handled as named errors, not panics.
4. Child processes are tracked and terminated on app exit and on crash-recovery
   startup. An orphaned sidecar is a release blocker.
5. A sidecar failure never takes the app down: local inference is a feature that
   can be unavailable while bundling continues to work.
6. Model files are registered by the user or downloaded from an allowlisted URL
   with checksum verification; a model that fails verification cannot be
   activated.

## Consequences

- Packaging and signing are more complex and must start early in the release
  cycle (risk register: signing delays).
- Users on unsupported hardware get a clear "not available on this machine"
  rather than a process that starts and hangs.

## Alternatives considered

- **In-process inference via a Rust binding.** Rejected for the first release: a
  crash in the inference library would take the whole app down, losing the
  user's selection and settings.
