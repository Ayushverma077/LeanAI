# LeanAI Desktop user guide

## What it does

You pick a directory. LeanAI walks it, tells you what it found and what it
excluded and why, lets you choose files, and produces one deterministic text
bundle with a labelled token estimate that you copy or save.

It also builds `PROJECT_CONTEXT.md`: a Markdown index of the project where every
section records the files and content hashes it came from, so you can always
jump from a claim to the current source.

All of this runs on your machine. There is no API key, no account and no network
request — the app is built without a network capability at all.

## First run

1. **Project → Choose a project directory.** LeanAI canonicalises the path and
   treats it as the only directory it may read.
2. The scan starts automatically. Progress is reported and you can cancel it;
   cancelling leaves nothing half-written.
3. **What the scan found** shows counts per classification. Anything that is not
   `text` is excluded by default and carries a reason.

## Choosing files

The **Files** tab is a virtualised tree. It stays responsive on large
repositories because only the visible rows are rendered.

- Checkbox on a folder selects every *selectable* file beneath it. A folder
  checkbox can never pull in a credential file, a binary, or anything else the
  policy blocks — that is enforced in the backend, not just hidden in the UI.
- Blocked files show a labelled chip (`⚠ credential`, `binary`, `too large`…)
  and their reason on hover. **Include anyway** opens a per-file confirmation
  where you re-read the reason and type a phrase.
- Search filters by path and expands the tree to the matches.
- Everything is keyboard operable: arrows to move, ← / → to collapse and expand,
  Space or Enter to toggle, Home / End to jump.

**Select from git changes** picks files by change scope — staged, unstaged,
untracked, or everything that differs from a base ref you name. Files that are
changed but excluded by policy are reported in the count, not silently added.

**Presets** store the *rule* (files, folders, overrides), not a frozen list. When
you reopen a project each preset is re-resolved against the current scan and
tells you how many paths no longer exist or are now blocked.

## The bundle

The **Bundle** tab shows exactly the text that will be copied or saved.

Options — headers, tree preamble, code fences, line numbers, size annotations,
front matter, line-ending normalisation — each change the bytes, and the same
project revision plus the same selection and options always produces the same
bytes and the same SHA-256.

### Reading the token number

Every token figure is captioned `estimate · cl100k_base · OpenAI-family`.

That caption is the important part. `cl100k_base` is OpenAI's tokenizer. For
Anthropic, Gemini or a local GGUF model the real count will differ, sometimes
substantially. Use the number to compare *selections against each other*, not to
predict a bill.

**What is using the budget** ranks files by their share of the estimate, which
is usually how you find the one file that is eating your context window.

## Exporting

Copy and Save both go through the same review screen:

- the complete list of files being included,
- size and the labelled estimate,
- secret-scan findings, graded high / medium / low, with the matched value
  redacted,
- a sentence describing where the data is going.

If the scan flags anything medium or high, you must tick an acknowledgement
before the export button enables. **The scan is a review aid, not a
guarantee** — it misses real secrets and flags harmless strings. Read the file
list.

Saving a bundle also writes `<name>.leanai-manifest.yaml` next to it, containing
the project fingerprint, revision, selection, options, estimate kind and
warnings. The fingerprint contains no path, so the manifest is safe to share.

## The context index

**Context → Generate context index** builds `PROJECT_CONTEXT.md` from the scan:
metadata, quick reference, summary, directory structure, file inventory,
architecture, key modules, API contracts, dependencies, workflows,
configuration, known issues, decisions, agent task history and source
references.

It is written by ordinary code, not by a model. Every section carries:

- **a freshness badge** — `fresh` when every cited file still hashes to the
  recorded value, `stale` when something changed, `unknown` when a source cannot
  be verified;
- **limitations** — what that section could not determine, so silence is never
  mistaken for absence;
- **sources** — click any one to fetch the current file. If it changed since the
  scan, the dialog says so.

Regenerating after edits is cheap. A stale section is a prompt to regenerate, not
a reason to distrust the app.

## Settings

- **Privacy** — history retention (metadata-only by default, so LeanAI does not
  accumulate copies of your source), export confirmation, telemetry (off).
- **Scan policy** — ignore sources, lockfile/generated/hidden handling, symlink
  following (off), maximum file size.
- **Ignore precedence** — the full ordering, plus why each limit exists.
- **.aiignore** — edit it, **preview the effect** (which files it would newly
  exclude, which rules match nothing, which are invalid), then write it. The
  write button stays disabled until you have previewed.
- **Local data** — bundle history and one-click clear.
- **Audit log** — every export and project write, in order.
- **Diagnostics** — versions and granted capabilities, with no paths, source or
  credentials, so it is safe to paste into a support request.

## Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| A file is missing from the tree | An ignore rule or the safety policy excluded it | Check Settings → Ignore precedence; search for the path — excluded files still appear, greyed, with a reason |
| "the scan hit the file limit" | `max_files_scanned` reached | Raise it in Settings, or open a narrower directory |
| A folder checkbox will not include a file | It is credential-sensitive, binary, oversized or unreadable | Use **include anyway** on the individual file, if you are sure |
| Preset says "N now blocked" | Files it referenced became excluded (policy change, or the file changed) | Re-select and save the preset again |
| Context says STALE | Cited files changed since generation | Regenerate, or open the source behind the claim |
| "This project is not a git repository" | Diff mode needs git | Select manually; revision falls back to a content hash |
| Export button is disabled | Secret findings need acknowledgement | Review the findings, then tick the box |

## What LeanAI 0.1 does not do

- It does not talk to any AI provider. Model routing, cloud adapters and local
  inference are designed (ADRs 0007-0009) but not implemented.
- It has no agents. The audit ledger they will use already exists.
- It does not do semantic search over your repository.
- It does not claim a token-savings percentage. See
  `docs/benchmark-methodology.md` for what has and has not been measured.
