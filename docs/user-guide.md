# LeanAI Desktop user guide

## What it does

You pick a directory. LeanAI walks it, tells you what it found and what it
excluded and why, lets you choose files, and produces one deterministic text
bundle with a labelled token estimate that you copy or save.

It also builds `PROJECT_CONTEXT.md`: a Markdown index of the project where every
section records the files and content hashes it came from, so you can always
jump from a claim to the current source.

Scanning, bundling, token estimation and the context index run entirely on
your machine, with no API key, no account and no network request.

**Ask LeanAI** (Tasks page) goes one step further: you type a request and a
model answers it or proposes a change, getting only the context it asks for.
That needs a model, either one on your machine or a provider you configure,
and it is the only part of LeanAI that sends project text anywhere. See
[Ask LeanAI](#ask-leanai) and [Models](#models).

## First run

1. **Project → Choose a project directory.** LeanAI canonicalises the path and
   treats it as the only directory it may read.
2. The scan starts automatically. Progress is reported and you can cancel it;
   cancelling leaves nothing half-written.
3. **What the scan found** shows counts per classification. Anything that is not
   `text` is excluded by default and carries a reason.
4. LeanAI then builds the project map (`PROJECT_CONTEXT.md`), or reuses it if
   nothing changed, and opens **Tasks** so you can start asking. If the
   project already has a `PROJECT_CONTEXT.md` that needs your decision, it
   opens the **Context** page instead, where the alert is. If you move to
   another page while this runs, LeanAI leaves you there.

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

The **Context** page shows everything an AI would get in one place, under
**What the AI gets**:

1. **Project map** on top: a short summary of your project, also saved as
   `PROJECT_CONTEXT.md` (see [The context index](#the-context-index)). Untick
   **Include** on the map to leave it out.
2. **Selected files** below: the code of the files you ticked on the left.

The header says what **Copy for AI** will send and whether its size is a
problem, for example:

> Project map + 54 files
> **Medium** (~96k tokens) — fits in most AI chats; some free plans may cut it off.

| Size | Tokens (estimate) | Meaning |
| --- | --- | --- |
| **Small** | up to 30k | Fits in almost any AI chat |
| **Medium** | up to 120k | Fits in most AI chats; some free plans may cut it off |
| **Large** | more than 120k | Too big for many AI chats; pick fewer files |

These are rough guides, not promises about any one AI. With no files ticked,
Copy sends just the map, which is a good first message to any AI chat.

Options (project map, headers, tree preamble, code fences, line numbers, size
annotations, front matter, line-ending normalisation) each change the bytes, and the same
project revision plus the same selection and options always produces the same
bytes and the same SHA-256.

### Reading the token number

Every token figure is captioned `estimate · cl100k_base · OpenAI-family`. In the
**What the AI gets** header the size is shown in words; hover over the token
count to see the exact number and this caption.

That caption is the important part. `cl100k_base` is OpenAI's tokenizer. For
Anthropic, Gemini or a local GGUF model the real count will differ, sometimes
substantially. Use the number to compare *selections against each other*, not to
predict a bill.

**What is using the budget** ranks files by their share of the estimate, which
is usually how you find the one file that is eating your context window.

## Exporting

Copy and Save both go through the same review screen:

- the complete list of files being included, and whether the project map goes
  first (it is secret-scanned too),
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

The project map at the top of the **Context** page is `PROJECT_CONTEXT.md`.
LeanAI builds it when you open the project (or when you press **Refresh** on
the map, after which it opens **Tasks**) and saves it in the project, excluded
from Git. It is
short on purpose, with nine sections of a few plain lines each:

| Section | What it shows |
| --- | --- |
| Overview | The README's first paragraph, languages, entry points, how many files are covered |
| Structure | Top folders and their main subfolders, with file counts |
| Key Modules | The 20 files that export the most names, with up to 5 names each |
| API Routes | HTTP routes and interface files (`.proto`, GraphQL, OpenAPI) |
| Dependencies | One line per manifest (`package.json`, `Cargo.toml`, …) |
| How to Run | npm scripts and CI or task files |
| Configuration | Config files and the *names* of environment variables (never values) |
| Open TODOs | TODO/FIXME/HACK comments, with file and line |
| Decisions | Architecture decision records, each with its title |

Long lists show the first items and count the rest ("+15 more"). Test files are
left out of Key Modules and API Routes.

It is written by ordinary code, not by a model. The file itself stays plain.
On the **Context** page, each section's **Sources and limits** shows:

- **a freshness badge** (on the map as a whole, and on any out-of-date
  section): **Up to date** when every cited file still hashes to the recorded
  value, **Out of date** when something changed, **Can't check** when a source
  cannot be verified;
- **limitations** — what that section could not determine, so silence is never
  mistaken for absence;
- **sources** — click any one to fetch the current file. If it changed since the
  scan, the dialog says so.

Refreshing after edits is cheap. An out-of-date section is a prompt to press
**Refresh**, not a reason to distrust the app.

### If the project already has a PROJECT_CONTEXT.md

LeanAI only overwrites `PROJECT_CONTEXT.md` without asking when the file is
exactly what LeanAI last wrote. Otherwise it leaves the file alone, keeps its
map inside the app (Ask LeanAI and Copy still use it) and shows an alert:

| Alert | What it means | Your options |
| --- | --- | --- |
| *already exists in this project* | Someone else wrote the file, or it was edited after LeanAI wrote it | **Replace file** (then **Yes, replace it**) or **Keep mine** |
| *LeanAI can't write PROJECT_CONTEXT.md* | The file is tracked by Git, is a folder or link, or cannot be read | Dismiss. For a tracked file, `git rm --cached PROJECT_CONTEXT.md` lets LeanAI manage it |

## Ask LeanAI

**Tasks** opens a conversation workspace. Start a task or choose an existing
conversation on the left. Ask a question, describe a bug, or request a change;
you don't need to pick files or models. The sample advanced console has been
replaced by this real model workflow.

- Follow-up requests include compact excerpts from the last six completed turns.
  Source files are read again as needed. Use **New task** for a fresh conversation.
- Activity shows backend events as they arrive, with elapsed time.
- Review each proposed diff, then **Apply changes** or **Discard** before continuing.
- Expand **Checks** to run a command from the project's allowed list. Its actual
  output and pass/fail result stay with that turn; checks are not run automatically.
- Drafts and results survive switching pages and stay separate for each project.
  Conversations are kept in memory until the app closes; they are not restored
  after restarting the app.
- Errors appear in the conversation and failed requests return to the composer
  for editing or retrying. One operation runs at a time.

1. **Routing, locally.** LeanAI reads your request (no model involved) and
   works out the intent, how hard it is and how much of the project it needs.
   It then picks the cheapest available model that is strong enough.
2. **Minimum context.** The model first receives only the parts of
   `PROJECT_CONTEXT.md` the request needs, never the source files. It then asks
   for specific files, line ranges or searches. Every request is checked:
   paths must be inside the project, credential files are refused, and lines
   that look like secrets are redacted whenever the text goes anywhere other
   than the managed local model.
3. **Structured replies.** Each model turn is one JSON action (read files,
   search, edit, answer). Where the provider supports it, the reply is
   constrained to a schema, so it cannot come back malformed. If a server does
   not support schemas, LeanAI falls back to instructions only and tells you.
4. **Validation.** Edits become a diff that must apply to the current files
   exactly. The validator then checks the diff for paths outside the project
   and for secrets.
5. **The apply gate** decides how much review the change needs:

   | Outcome | When |
   | --- | --- |
   | **Applied automatically** | "Apply low-risk changes automatically" is on, and the change is small (≤ 3 files, ≤ 60 changed lines), clearly requested, touches nothing consequential and the run went cleanly. |
   | **Review** | More files or lines than that; unclear whether you asked for a change; the request mentions auth, sessions, payments or migrations; or the model needed corrections. |
   | **Review carefully** | The change touches a lockfile, a build or dependency manifest, CI, deployment, a migration, `.gitignore`/`.aiignore` or app permissions; deletes a file; was proposed for a request that reads as a question; mentions production data; triggered a validator warning; or a file read during the run contained text that looked like instructions to the model. You must tick "I've read these changes" before **Apply changes** is enabled. |

   The reasons are listed above the buttons. Nothing is written until you
   apply, and an applied change is transactional: if any file fails, every
   file is restored.
6. After an apply, `PROJECT_CONTEXT.md` is regenerated so it never describes
   code that is gone.

**How LeanAI handled this** (under each result) shows the model used, the
context sent versus the whole project, each file read, each escalation to a
stronger model and the gate's decision.

The intent percentage shown there is a heuristic from weighted cues, not a
calibrated probability. 0% means the cues were evenly split and 100% means they
all agreed.

## Models

**Models** lists what Ask LeanAI can use:

- **Local model runtime.** LeanAI starts `llama-server` with a GGUF file you
  choose, bound to `127.0.0.1` on a random port. Nothing leaves your machine.
- **Cloud providers** (Anthropic, OpenAI). API keys are stored in the OS
  keychain, never in LeanAI's database or logs.
- **Self-hosted models.** Any OpenAI-compatible server (Ollama, vLLM,
  LM Studio, llama.cpp) on this or another machine. Add it by URL, test the
  connection, and optionally store a key in the keychain. These cost nothing
  per token, so the router prefers them when they are strong enough.

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
| Project map says **Out of date** | Cited files changed since the map was made | Press **Refresh**, or open the source behind the claim |
| "This project is not a git repository" | Diff mode needs git | Select manually; revision falls back to a content hash |
| Export button is disabled | Secret findings need acknowledgement | Review the findings, then tick the box |

## What LeanAI does not do

- It does not send anything to a model unless you use Ask LeanAI or the agent
  console, and then only to the model the router picked from the ones you
  configured.
- It does not apply a model's change without validation, and never applies a
  consequential one without your review.
- It does not claim a token-savings percentage. See
  `docs/benchmark-methodology.md` for what has and has not been measured.

For what changed and when, see [`docs/CHANGELOG.md`](CHANGELOG.md).
