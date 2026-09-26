# ADR 0002: Least-privilege Tauri capabilities; policy lives in Rust

- **Status:** Accepted
- **Date:** 2026-09-06
- **Relates to:** FR-01, NFR 7.1, backlog 1.4

## Context

Tauri can grant the webview broad filesystem, shell and HTTP access. The webview
renders content derived from repository files, which are untrusted. Any
capability granted to the frontend is, in effect, granted to anything that can
influence what the frontend renders.

## Decision

1. The window capability set (`src-tauri/capabilities/default.json`) grants only
   `core:default`, dialog open/save/message/confirm, and clipboard **write**.
   No shell plugin, no HTTP plugin, no ambient filesystem read or write scope.
2. Project files are never read through a filesystem capability. They are read
   by LeanAI's own commands, which resolve every path with
   `project::resolve_within_root` and refuse absolute paths, `..` traversal and
   symlinks that resolve outside the approved root.
3. The only write LeanAI performs inside a user's project is `.aiignore`, via a
   dedicated command with a fixed filename. Bundles and context documents are
   written to a path the user picked in the OS save dialog.
4. The CSP forbids remote script, style and connect sources. `assetProtocol` is
   disabled.
5. Policy decisions (classification, exclusion, limits, overrides) live in
   `leanai-core`, not in the frontend. The UI displays the policy the backend
   reports; it does not keep a second copy of the rules.

## Consequences

- A compromised or buggy frontend cannot read outside the project, run a
  command, or make a network request, because the capability simply is not
  granted.
- Adding a feature that needs a new capability is a visible, reviewable change
  to one JSON file, not an incidental import.
- Clipboard write is the one outbound path; it is gated behind the export
  preflight screen and recorded in the audit log.

## Alternatives considered

- **Grant `fs:allow-read-file` scoped to the project directory.** Rejected: the
  scope is set at grant time and cannot express LeanAI's per-file policy, so a
  credential file inside the scope would be readable by the webview.
