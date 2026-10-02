# ADR 0013: Risk-scaled apply gate and schema-constrained model replies

- **Status:** Accepted
- **Date:** 2026-09-27
- **Deciders:** project owner
- **Relates to:** FR-28, NFR 7.1, ADR 0010, ADR 0011

## Context

The prompt runner (`commands/prompt.rs`) lets a model propose edits, validates
them, and then either applies them or asks for approval. Two things about that
last step were weak.

1. **One switch decided everything.** `auto_apply` was a plain boolean. With it
   on, any change that passed the validator was written, including changes to
   lockfiles, CI workflows or migrations. With it off, a one-word label change
   waited for a click like everything else. The cost of a wrong change depends
   on what it touches, so a single threshold is wrong in both directions.
2. **Model replies were free text parsed as JSON.** A reply that was not a
   valid action cost a paid call, and two in a row escalated the run to a more
   expensive model (`FAILURES_BEFORE_ESCALATION`).

Both ideas were prompted by TypeSafe AI's Jev model, which returns typed
decisions with a confidence score and lets the caller set different thresholds
for acting automatically and asking a person. LeanAI adopts the *pattern*, not
the model: Jev is a hosted classifier that cannot write code, and its published
figures (including "0% hallucinations", which its authors say is not measured)
do not meet this project's rule that no claim appears without a measurement.

## Decision

1. **Apply gate** (`leanai_core::apply_gate`). After the validator passes a
   change, `apply_gate::decide` ranks it as `auto_apply`, `confirm` or
   `careful_review`, with a list of reasons.
   - *Careful review:* the change deletes a file; touches a lockfile, build or
     dependency manifest, CI, deployment, migration, ignore-rule or app
     permission file; the request reads as a question (`change_share` < 0.5);
     prompt risk ≥ 6/10; any validator warning; or a file read during the run
     contained text resembling instructions to the model.
   - *Confirm:* more than 3 files or 60 changed lines; unclear whether a
     change was requested at all (binary confidence < 0.5); prompt risk ≥ 3/10
     (auth, sessions, payments, migrations); or the model needed corrections
     (rejected replies or edits, failed validation, "insufficient").
   - *Auto-apply:* none of the above **and** the user turned on "Apply low-risk
     changes automatically". With the setting off, a clean change is `confirm`
     with no reasons.
   - A careful review cannot be applied until the user ticks "I've read these
     changes".
   - The decision is returned to the UI, written to the run trace
     (`apply_gate` step) and stored in the run's `policy` JSON, so its reasons
     can later be compared with what users approved or discarded.
2. **Intent confidence** (`routing::distribution_confidence`). The winning
   intent's share of the evidence is rescaled to `(k·p − 1)/(k − 1)`, so 0
   means evenly split and 1 means unanimous. The same formula with k = 2 turns
   `change_share` (evidence for edit, bug fix or refactor) into the "was a
   change requested?" confidence the gate uses. Both are heuristic scores from
   weighted cues and are documented as not calibrated.
3. **Schema-constrained replies** (`llm_protocol::action_schema`). Agent-mode
   calls send one flat JSON schema for the action: `response_format`
   (`json_schema`, strict) to OpenAI, llama-server and self-hosted servers,
   and `output_config.format` to Anthropic. Forced tool use is not used, because
   newer Claude models reject `tool_choice` `tool`/`any`. Direct (free-text)
   answers send no schema. If a server rejects the schema with a request error,
   the call is repeated once without it, that model is not sent the schema again
   in the run, and the user sees one warning. `parse_action` still validates
   every reply: the schema guarantees shape, not meaning.

## Consequences

- Auto-apply is safe to leave on for small edits; anything consequential still
  reaches a person, with the reason stated.
- The thresholds are policy constants in one module (`MAX_AUTO_FILES`,
  `MAX_AUTO_CHANGED_LINES`, `CONFIRM_RISK`, `CAREFUL_RISK`,
  `MIN_CHANGE_CONFIDENCE`). They are not tuned against data yet. The stored
  decisions make that possible later; until then they are deliberately
  conservative, and "Can you make the button blue?" will ask for confirmation.
- The flat schema makes every reply carry null fields for unused keys, a few
  dozen extra output tokens per turn.
- A server that does not support schemas costs one extra, fast, rejected
  request per model per run.
- `intent_confidence` values shown in the UI are lower than before for the same
  prompt, because the floor is now 0 instead of 1/10.

## Alternatives considered

- **Ask the model for a confidence number in its JSON.** Self-reported
  confidence from chat-tuned models is poorly calibrated; it would be a number
  that looks meaningful and is not.
- **Use token log-probabilities as confidence.** Possible for the local
  llama-server, not available uniformly across providers, and still
  uncalibrated. Deferred until there is data to calibrate against.
- **Call Jev (or a similar hosted classifier) for routing or gating.** Adds a
  paid network dependency to replace deterministic local code that runs in
  microseconds, and it cannot produce the edits themselves.
- **Refuse high-risk changes outright.** The validator already blocks changes
  that must never be offered (for example a high-confidence secret in a diff).
  Beyond that, a person reviewing the diff is the right escalation, not a
  refusal.
- **Per-provider union schemas.** Tighter (the schema would tie fields to the
  action), but OpenAI strict mode forbids a non-object root. One flat schema
  works everywhere and `parse_action` enforces the rest.
