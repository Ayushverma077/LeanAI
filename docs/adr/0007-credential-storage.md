# ADR 0007: API credentials live in OS secure storage, never in app data

- **Status:** Accepted (implementation deferred to Phase 7)
- **Date:** 2026-09-06
- **Relates to:** FR-24, NFR 7.1, backlog 7.2

## Context

Phase 7 introduces cloud providers, which need API keys. A key in the SQLite
file, in a config file, or in an environment variable read at startup is a key
that ends up in backups, diagnostic bundles and support tickets.

## Decision

1. Credentials are stored in the platform keychain — macOS Keychain, Windows
   Credential Manager — under a per-provider service name. LeanAI stores only a
   non-secret reference (provider id, account label, "configured" flag) in
   SQLite.
2. A credential is read into memory at the moment of a request and is never
   written to a log, an error message, a run event, a bundle, a manifest or a
   telemetry payload.
3. Run events store redacted payloads. The `run_events.redacted_payload` column
   name is deliberate: it is a contract, not a description.
4. Disconnecting an account deletes the keychain entry and is testable: a test
   must assert the key is gone from the keychain, not merely that the UI shows
   "disconnected".
5. If the platform keychain is unavailable, LeanAI reports that cloud providers
   cannot be configured on this machine. It does not fall back to a file.

## Consequences

- Users may see an OS keychain prompt. That is the correct, familiar signal.
- Headless CI cannot exercise real credential storage; provider contract tests
  run against a mock provider instead.

## Alternatives considered

- **Encrypt keys in SQLite with a derived key.** Rejected: the derivation
  material has to live somewhere, which reduces to storing the key next to the
  ciphertext.
