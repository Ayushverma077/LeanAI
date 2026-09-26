# LeanAI Desktop

Turn a local repository into a **deterministic, inspectable context bundle** —
offline, with no API key, no account and no network access.

You pick a directory. LeanAI walks it, shows what it excluded and why, lets you
choose files, and produces one text bundle with a token estimate that is
honestly labelled. It also builds a `PROJECT_CONTEXT.md` index where every
section records the files and content hashes it was generated from, so a summary
never has to be taken on trust.

## Why the constraints

- **Deterministic before generative.** Discovery, filtering, bundling, token
  estimation and the context index need no model. Same revision + same selection
  + same options ⇒ byte-identical output and the same SHA-256.
- **The app has no network capability at all.** Not "disabled by default" —
  the Tauri capability manifest does not grant it. Neither is shell access.
- **Secrets are a policy, not a filter.** `.env`, keys and credential stores are
  unselectable by classification, so a folder-level "select all" cannot reach
  them. Including one takes a per-file confirmation you have to type.
- **Every token number carries its provenance.** A local `cl100k_base` count is
  labelled `estimate · cl100k_base · OpenAI-family`, because it is not what
  Anthropic, Gemini or a GGUF model will bill you.
- **No savings claim.** The benchmark harness exists, and it prints what it did
  *not* measure in every report.

## Quick start

```bash
npm install
npm run dev
```

Requires Node 20+ and a stable Rust toolchain (plus Xcode Command Line Tools on
macOS, or the MSVC build tools and WebView2 on Windows).

```bash
npm run check:all    # typecheck, lint, vitest, cargo test, fmt, clippy
npm run build        # packaged desktop build
```

## Layout

| Path                    | What lives there                                                                                                                  |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `crates/leanai-core/` | Every product rule: policy, scanning, classification, selection, bundling, tokens, secrets, git, context index, benchmark harness |
| `src-tauri/`          | Tauri app: typed commands, app state, SQLite migrations and repositories                                                          |
| `src/`                | React + TypeScript UI                                                                                                             |
| `docs/`               | ADRs, traceability matrix, threat model, user guide, runbooks                                                                     |
| `Instructions.md`     | The 0-to-N plan this repository implements                                                                                        |

Rules live in `leanai-core` so they are testable without a desktop app and
cannot be widened by adding a command. See [docs/architecture.md](docs/architecture.md).

## Status

Implemented and tested: Phases 0–5 of the plan, plus the parts of 11–12 that do
not need signing credentials. Local models, cloud providers and agents (Phases
6–10) are designed in ADRs 0007–0010 and are **not implemented**.

The honest, itemised version is in
[docs/project-status.md](docs/project-status.md).

## Documentation

- [User guide](docs/user-guide.md)
- [Architecture](docs/architecture.md)
- [Threat model and prompt-injection policy](docs/security-threat-model.md)
- [Requirements traceability](docs/traceability.md)
- [Benchmark methodology and results](docs/benchmark-methodology.md)
- [Fixture catalog](docs/fixture-catalog.md) · [Manual test plan](docs/manual-test-plan.md)
- [Release runbook](docs/runbooks/release.md) · [Support runbook](docs/runbooks/support.md)
- [Architecture decision records](docs/adr/)

## License

MIT — see [LICENSE](LICENSE).
