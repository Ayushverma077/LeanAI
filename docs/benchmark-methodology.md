# Benchmark methodology and results

Backlog 5.7-5.9. This document exists to keep an honest line between what has
been measured and what has not.

## What the harness does

`leanai-bench <suite.json>` assembles context for each task four ways and
reports files, tokens, bytes, assembly time, and precision/recall against a
human-declared set of relevant files.

| Strategy | Contents |
| --- | --- |
| `raw (all files)` | Every selectable file in the repository |
| `user-pinned` | Only the files a human declared relevant |
| `repo map only` | The deterministic context index, no file bodies |
| `LeanAI hybrid` | Context index followed by the pinned file bodies (the composition order from Instructions §8.4) |

Precision and recall are reported for the strategies that carry file bodies.
They are `n/a` for `repo map only`, because a map has no file-level selection to
score — reporting 0 % there would be misleading.

## Running it

```bash
cargo build --release -p leanai-core --bins
./target/release/leanai-bench suite.json --markdown report.md --json report.json
```

A suite is a JSON array:

```json
[
  {
    "id": "api-error-handling",
    "repository": "../fixtures/service",
    "task": "Add structured error responses to the HTTP layer",
    "taskClass": "multi_file_planning",
    "relevantFiles": ["src/http/router.ts", "src/http/errors.ts"],
    "leakageRisk": "controlled"
  }
]
```

`leakageRisk` matters: a public repository old enough to be in a model's
training data cannot tell you whether a context strategy helped, because the
model may already know the answer. Prefer `controlled` (private or newly
written) repositories for anything you intend to conclude from.

## What is deliberately not measured

Reproduced in every generated report:

- Task success — whether a model actually completed the task with this context.
- Validation and test pass rate after the model's proposal.
- Retries, tool calls and total wall-clock time for the whole task.
- Billed input, output and cached tokens, and therefore money.
- Provider-exact token counts. Every number is a local `cl100k_base` estimate.
- Answer quality, correctness, and the human correction time afterwards.

All six need a real provider run, which LeanAI 0.1 cannot do because it has no
network capability. Until they are measured on controlled repositories, **no
LeanAI surface may state a token- or cost-savings percentage** (Instructions §2,
N.3).

## Example run

Against this repository, one planning task, ground truth of three files:

| Strategy | Files | Tokens (est.) | Assembly | Precision | Recall |
| --- | ---: | ---: | ---: | ---: | ---: |
| raw (all files) | 72 | 319,241 | 6,321 ms | 4 % | 100 % |
| user-pinned | 3 | 5,532 | 103 ms | 100 % | 100 % |
| repo map only | 0 | 14,798 | 411 ms | n/a | n/a |
| LeanAI hybrid | 3 | 20,330 | 448 ms | 100 % | 100 % |

Machine: macOS / aarch64, 8 cores, policy v1, `cl100k_base`.

**How to read this, and how not to.** The hybrid context is about 6 % the size
of the raw bundle for this task. That is a real difference in assembled size and
assembly time. It is *not* evidence that LeanAI saves 94 % of anything:

- Ground truth was declared by the same person who chose the task. Pinning
  three files you already know is not the same as finding them.
- One task on one repository is an anecdote.
- The hybrid context costs ~3.7× the pinned bundle. Whether the map earns those
  tokens depends entirely on whether it improves the answer — which is exactly
  what is not measured here.
- Nothing here says the model would have produced a *correct* result from any of
  these contexts.

## Before publishing any performance claim

1. At least five tasks across at least three repositories, `leakageRisk:
   controlled`.
2. A real provider, with billed input/output/cached tokens recorded.
3. Task outcome judged by build/test success, not by similarity to a reference.
4. Repeated runs, with variance reported.
5. The full report published with its limitations, not a single percentage.

## Filesystem & UI Performance Matrix

For local deterministic filesystem walk timings, large-tree (100,000 files)
benchmarks, memory profiles, and virtualised UI responsiveness metrics, see
[docs/performance-matrix.md](performance-matrix.md).

