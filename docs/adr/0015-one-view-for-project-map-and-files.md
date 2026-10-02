# ADR 0015: The project map and the chosen files in one view, one copy

- **Status:** Accepted
- **Date:** 2026-09-27
- **Deciders:** project owner
- **Relates to:** FR-09 to FR-12, FR-17, ADR 0014, backlog 4.6 (export preflight)

## Context

The Context page had two tabs: **Bundle** (the code of the files you tick) and
**PROJECT_CONTEXT.md** (the project map). A model usually needs both: the map
to understand the project, and the code of the files the question is about.
That is the "LeanAI hybrid" strategy in the benchmark. The tabs split them
apart, and they had three problems:

- the one **Copy** button copied only the bundle, even on the map tab, and was
  disabled when no file was ticked, so the map could not be copied at all;
- the map tab showed each section's raw Markdown in a code box (`**bold**`,
  backticks, `>`), which was hard to read;
- the page's rebuild effect depended on the whole store object, which changes
  after every build, so the preview rebuilt in a loop.

## Decision

1. **One view, "Context for AI".** The project map sits on top and the chosen
   files below it, with a header that says exactly what will be copied
   ("Project map + 3 files · 42 KB · ~11,000 tokens").
2. **"Include project map" option, on by default** (`BundleOptions::
   include_project_map`, `serde(default)` so old presets load with it off).
   When on, the rendered `PROJECT_CONTEXT.md` becomes the first part of the
   bundle text, after any front matter and before the file tree, separated by
   `---`.
3. **The map is part of the bundle, not glued on in the UI.** It is inside the
   hashed text (`output_hash`), counted in the token estimate, recorded in the
   manifest options, secret-scanned in the export preflight and noted in the
   audit record (`projectMapIncluded`). `concat::build_with_map` does this;
   `concat::build` is unchanged for callers without a map.
4. **What you see is what is copied.** The backend uses the stored index,
   regenerating it first when missing, stale or in an older format, and
   returns that exact document with the preview. The preview text leaves the
   map out (it is shown above as readable sections) while every number still
   covers the whole bundle.
5. **Copy works with only the map** when no files are picked.
6. **The map is rendered, not shown as source.** A small renderer
   (`MiniMarkdown`) handles the subset the generator writes: lists, quotes,
   bold, code and notes. It builds React elements only, never HTML. Each
   section keeps a "Sources and limits" disclosure with its limitations and
   source files (source-on-demand).
7. The rebuild effect depends on the stable `buildBundle` action, which ends
   the rebuild loop.

## Consequences

- One place and one action for the common case. Turning the option off gives
  the old files-only bundle, byte for byte.
- A bundle with the map has a different hash from one without it; both are
  deterministic.
- Opening the Context page with the option on builds the map if the project
  has none yet, so the first open can take a moment on a large repository.
- `PROJECT_CONTEXT.md` is still saved in the project for Ask LeanAI and other
  tools; nothing about that changes.

## Update, 2026-09-27 12:17 IST: plainer labels

The view was reworded for people who don't know the jargon. The header is now
**What the AI gets**, with the parts ("Project map + 54 files") and a size in
words (Small up to 30k tokens, Medium up to 120k, Large beyond), each with
one line of advice. The exact estimate and its ADR 0004 label
(`estimate · cl100k_base · OpenAI-family`) appear when you hover over the token
count, and in full on the Copy review screen. **Copy for AI** is the page's
primary button. The map's **Include** checkbox and **Refresh** button (was
"Regenerate") sit on the map itself, whose freshness reads **Up to date**,
**Out of date** or **Can't check**.

## Alternatives considered

- **Keep two tabs and fix only the Copy button.** Still two places to look,
  and the common case (map plus files) still needs two copies.
- **Join the two texts in the UI when copying.** The map would bypass the
  export preflight, the hash, the manifest and the audit log.
- **Show the whole bundle, map included, as raw text.** Simplest, but that
  raw Markdown is what made the map hard to read.
