# Threat model and prompt-injection policy

Scope: LeanAI Desktop 0.1 — the offline bundler and context index. Phases 6-10
(local models, cloud providers, agents) add threats that are designed for in
ADRs 0007-0010 but are not implemented, and are marked as such.

## Assets

1. The user's source code, including code they never intended to send anywhere.
2. Credentials inside the repository (`.env`, keys, tokens).
3. Credentials for AI providers (Phase 7; not yet stored).
4. The integrity of what LeanAI reports — a wrong claim about which files were
   included is itself a security failure.

## Trust boundaries

| Boundary | Trusted side | Untrusted side |
| --- | --- | --- |
| Project root | LeanAI's own code and settings | Every byte of the user's repository |
| IPC | Rust commands | The webview, and anything that influences what it renders |
| Export | Inside the app | Clipboard, saved files, any future provider |

**Repository content is data, never instruction.** A README, comment, test
fixture or vendored file that contains text addressed to a model or to LeanAI
carries no authority. Today this is enforced structurally: no component
interprets file content as a command, and no shell or network capability exists
to be steered. Phase 8 adds the agent-side rules in ADR 0010.

## Threats and mitigations

| # | Threat | Mitigation | Verified by |
| --- | --- | --- | --- |
| T1 | Path traversal or symlink escape reads a file outside the approved root | `resolve_within_root` rejects absolute paths, `..`, and links resolving outside; symlinks are not followed by default and are reported | `scan.rs::symlinks_never_escape_the_project_root`, `::path_resolution_rejects_traversal` |
| T2 | A credential file is swept into a bundle by "select all" | Credential classification precedes every other class and is not selectable by default; folder selection cannot include it at all | `bundle.rs::folder_selection_never_includes_blocked_files` |
| T3 | An ignore file re-includes a credential via `!` negation | LeanAI policy outranks every ignore source (ADR 0003) | `safety.rs::aiignore_cannot_reexpose_a_blocked_path` |
| T4 | A user exports a secret without noticing | Preflight shows the full file list, graded secret findings and a destination note; medium/high findings require a typed acknowledgement | `PreviewPage` preflight; `safety.rs::secret_findings_are_graded_and_redacted` |
| T5 | The secret scanner is mistaken for a guarantee | `SecretReport::DISCLAIMER` is displayed in every preflight and states that it can miss real secrets | `safety.rs` |
| T6 | A secret leaks through logs, errors or diagnostics | Findings store a redacted excerpt; OS errors are stripped of absolute paths; the diagnostic bundle contains no paths or source | `safety.rs::secret_findings_are_graded_and_redacted`, `walker::sanitize` |
| T7 | Injected instructions in repository content steer the app | No component executes file content; no shell or network capability is granted; context bodies are rendered as text | ADR 0002; capability manifest |
| T8 | A compromised webview reads arbitrary files | The webview has no filesystem capability; all reads go through boundary-checked commands | ADR 0002 |
| T9 | Remote code execution via loaded assets | CSP forbids remote script/style/connect; `assetProtocol` disabled; no CDN dependency | `tauri.conf.json` |
| T10 | Stale context leads to a wrong decision | Freshness recomputed on read; conservative invalidation; source-on-demand reports "changed since scan" | `context.rs::changed_files_invalidate_dependent_sections` |
| T11 | A model-written section fabricates a citation | `validate_model_section` rejects missing files, non-source files, missing hashes and changed hashes | `context.rs::model_sections_are_rejected_without_valid_citations` |
| T12 | Local database theft yields credentials | No credential column exists; keys will live in OS secure storage | `persistence.rs::no_table_stores_credentials`, ADR 0007 |
| T13 | Retained bundle text accumulates copies of source | Default retention is metadata-only; user can choose "keep nothing" | `persistence.rs::retention_controls_whether_bundle_text_is_stored` |
| T14 | Resource exhaustion on a hostile or huge tree | Bounded probe reads, file/selection/bundle/depth limits, cancellation, truncation reporting | `scan.rs::scan_reports_truncation_at_the_file_limit`, `bundle.rs::selection_limits_are_enforced` |

## Known limitations

Stated plainly, because pretending otherwise is the failure mode this document
exists to prevent.

- **The secret scanner is heuristic.** It matches known token formats and common
  assignment patterns. It will miss custom formats, encoded secrets and secrets
  split across lines, and it will flag harmless strings. It is a review aid.
- **Classification is conservative but not perfect.** A credential in an
  ordinarily-named source file is classified as source text; only the scanner
  and the user's own review catch it.
- **Content hashing proves change, not correctness.** A fresh section can still
  be a poor summary.
- **A user with an override can export anything.** That is the intended escape
  hatch; it is per-file, high-friction and audited, not prevented.
- **No supply-chain attestation yet.** Dependency audit runs in CI; reproducible
  builds and signature verification are Phase 12 work.

## Reporting a vulnerability

Open a private security advisory on the repository, or email the maintainers.
Please include the version from Settings → Diagnostics. Do not include source
files or credentials in the report.
