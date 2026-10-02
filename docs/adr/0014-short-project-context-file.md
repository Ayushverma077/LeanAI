# ADR 0014: A short PROJECT_CONTEXT.md; provenance stays in the app

- **Status:** Accepted
- **Date:** 2026-09-27
- **Deciders:** project owner
- **Relates to:** FR-17 to FR-20, ADR 0005 (amends how it is rendered), Instructions.md §8.3 (replaces its section list)

## Context

The generated `PROJECT_CONTEXT.md` followed the 15-section schema in
Instructions §8.3 and printed all of ADR 0005's provenance into the file. Every
section had a line with the full revision hash, a "Limitations" block and a
collapsible list of source files with hash prefixes. For this repository the
result was 1,343 lines (about 23,000 tokens), including a 224-line file table
and 559 lines of symbol lists. The project owner found it hard to read.

It is also what the prompt runner sends a model first, so every unneeded line
costs tokens on every run.

The information itself is not the problem. The problem is putting all of it in
the file. The stored `ContextDocument` already keeps each section's sources,
hashes, freshness and limitations, and the Context page already shows them.

## Decision

1. **Nine sections, each a few plain lines:** Overview (README's first
   paragraph, languages, entry points, file counts), Structure (top folders
   and their main subfolders, with counts), Key Modules (top 20 files by
   exported names, one line each with up to 5 names), API Routes,
   Dependencies (one line per manifest), How to Run (npm scripts and CI
   files), Configuration, Open TODOs and Decisions (each ADR with its title).
2. **Long lists are capped and counted** ("(+15 more)", "…and 63 more
   files"), never silently cut.
3. **The file carries no per-section provenance.** It has one revision line at
   the top and one closing note saying it is an index, how symbols are found,
   and where the sources, freshness and limits are (LeanAI → Context). A
   section that becomes stale is marked in the rendered text. A model-written
   section keeps its label (ADR 0005, point 6).
4. **Provenance is unchanged underneath.** Every section still records its
   sources with content hashes and its limitations; freshness is still
   recomputed on read; invalidation is still conservative (`overview` and
   `structure` go stale when any file is added); source-on-demand still works.
   ADR 0005's rules hold for the stored document and the app, not the file.
5. **Less noise:** test files are left out of Key Modules and API Routes, TODO
   markers count only at the start of a comment, and the ADR template is not
   listed as a decision.
6. **Schema version 2.** A stored index from version 1 is regenerated when the
   prompt runner needs it and is not shown on the Context page (which offers
   to generate a new one).

## Consequences

- For this repository the file goes from 1,343 lines (about 23,300 tokens) to
  114 lines (about 2,400 tokens).
- Someone reading only the file, outside LeanAI, no longer sees which files
  back each claim. They see a pointer to the app, the revision, and a clear
  statement that the file is an index and not the source.
- Instructions §8.3's section list is superseded. Dropped: Metadata (now the
  header and Overview), Quick Reference and Project Summary (merged into
  Overview), Directory Structure and Core Architecture (merged into
  Structure), File Inventory (the model searches or reads files instead),
  Agent Task History (always empty) and Source References (in the app).
- The prompt router's context levels use the new keys: minimal = overview and
  structure; sections = those plus key_modules; broad = those plus API routes,
  dependencies, how to run and configuration.
- Benchmark figures measured with the version 1 format are out of date.

## Alternatives considered

- **Keep the file and add a short summary at the top.** The file would stay
  heavy, and the model would still be sent the long sections.
- **Two files (short and full).** Two artifacts to keep in sync and explain;
  the full provenance already has a better home in the app.
- **Only raise the caps.** It shrinks the lists but keeps the repeated
  per-section boilerplate that made the file hard to scan.
