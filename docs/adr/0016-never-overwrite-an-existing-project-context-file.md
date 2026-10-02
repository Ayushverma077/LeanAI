# ADR 0016: Never overwrite a PROJECT_CONTEXT.md that LeanAI did not write

- **Status:** Accepted
- **Date:** 2026-09-27
- **Deciders:** project owner
- **Relates to:** FR-17, ADR 0005, ADR 0014, ADR 0015, NFR 7.1 (no silent data loss)

## Context

LeanAI saves its project map as `PROJECT_CONTEXT.md` in the project root. It
refused only when the file was tracked by Git. Any other existing file,
including one a team wrote by hand or one produced by another tool, was
overwritten without a word. That happened on **Regenerate**, and also
automatically: when Ask LeanAI refreshes the map after applying a change, and
when the Context page builds the map. The project owner asked for an alert
when the file already exists.

## Decision

1. **Classify the file before writing** (`context_file::file_state`):
   `missing`, `current` (byte-for-byte what LeanAI last wrote, by SHA-256 against
   the stored index's `content_hash`), `edited` (has LeanAI's
   `<!-- leanai.context/v…` marker but differs), `foreign` (no marker),
   `tracked_by_git`, `not_a_file` (folder or link) or `unreadable`.
2. **Write without asking only when nothing can be lost:** `missing` or
   `current`. `save_to_project` otherwise returns `SaveOutcome::Kept(state)`
   and touches nothing, not even `.gitignore`.
3. **The user decides for `edited` and `foreign`.** The Context page shows an
   alert naming the situation, with **Replace file** (then a confirmation,
   "Yes, replace it") or **Keep mine**. Only that explicit choice sends
   `replaceExisting: true`. `tracked_by_git`, `not_a_file` and `unreadable` are
   never written; the alert explains why and how to change it (for a tracked
   file, `git rm --cached PROJECT_CONTEXT.md`).
4. **The map keeps working either way.** It is always stored for the app, so
   Ask LeanAI and the Context page use LeanAI's current map even while the
   project file is kept.
5. Automatic paths (Ask LeanAI's refresh and the Context page's build) never
   replace a file; they only ever write `missing` or `current`.
6. Generating the map no longer fails when the file is tracked by Git; it
   stores the map and reports the file state instead.

## Consequences

- A hand-written or third-party `PROJECT_CONTEXT.md` can no longer be lost
  silently.
- LeanAI's own file, unchanged since it was written (including the older,
  longer format), is still updated silently, so everyday regeneration has no
  extra prompts.
- If LeanAI's database is reset while its file stays, the file shows as
  `edited` once, and the user chooses.
- **Keep mine** hides the alert until the page is opened again; the choice is
  not stored, so the alert comes back as a reminder that the project file
  differs from the map LeanAI uses.

## Alternatives considered

- **Back up and overwrite** (for example to `PROJECT_CONTEXT.md.bak`). This
  still changes the project without consent and leaves an extra file to
  explain.
- **Write LeanAI's map under a different name when the file exists.** Other
  tools and people look for `PROJECT_CONTEXT.md`; two similar files would be
  more confusing than one alert.
- **Alert on every write.** An unchanged LeanAI file needs no decision; asking
  every time would teach users to click through the alert.
