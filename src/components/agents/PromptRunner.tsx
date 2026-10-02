import { useEffect, useRef, useState } from "react";

import { api, toAppError } from "../../ipc/client";
import type { PromptRunResponse, ResolveApprovalResponse } from "../../ipc/types";
import { useAppStore } from "../../store/useAppStore";
import { needsReview, projectTaskKey, useTaskStore, type TaskTurn } from "../../store/useTaskStore";
import { AlertTriangleIcon, CheckCircleIcon, SparklesIcon } from "../icons";
import { Button, Chip, Toggle, formatNumber } from "../primitives";

const INTENT_LABELS: Record<string, string> = {
  CODE_EDIT: "Code edit",
  BUG_FIX: "Bug fix",
  REFACTOR: "Refactor",
  EXPLANATION: "Explanation",
  CREATIVE_WRITING: "Creative writing",
  GENERAL_QA: "General question",
  RESEARCH: "Research",
  SUMMARIZATION: "Summary",
  ARCHITECTURE: "Architecture",
  OTHER: "Other",
};

const STARTERS = [
  [
    "Understand this project",
    "Explain how this project is structured and where its main behavior lives.",
  ],
  [
    "Find a bug",
    "Inspect this project for a concrete bug. Explain the cause and propose a focused fix.",
  ],
  ["Improve the code", "Find a small, useful refactor in this project and propose the change."],
];

export function PromptRunner() {
  const { project, setRoute } = useAppStore();
  const { tasks, activeByProject, createTask, selectTask, editTask, send, resolve } =
    useTaskStore();
  const projectId = project?.id ?? null;
  const projectTasks = tasks.filter((task) => task.projectId === projectId);
  const active = projectTasks.find(
    (task) => task.id === activeByProject[projectTaskKey(projectId)],
  );
  const busy = tasks.some((task) =>
    task.turns.some((turn) => turn.state === "running" || turn.applying || turn.testing),
  );
  const reviewing = active?.turns.some(needsReview) ?? false;
  const scrollArea = useRef<HTMLDivElement>(null);
  const followBottom = useRef(true);
  const [, tick] = useState(0);

  useEffect(() => {
    if (!busy) return;
    const timer = setInterval(() => tick((value) => value + 1), 1000);
    return () => clearInterval(timer);
  }, [busy]);
  useEffect(() => {
    followBottom.current = true;
    if (scrollArea.current) scrollArea.current.scrollTop = 0;
  }, [active?.id]);
  useEffect(() => {
    if (scrollArea.current && followBottom.current && active?.turns.length)
      scrollArea.current.scrollTop = scrollArea.current.scrollHeight;
  }, [active?.id, active?.turns]);

  const setDraft = (draft: string) => editTask(active?.id ?? createTask(projectId), { draft });

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 lg:flex-row">
      <aside
        className="flex shrink-0 flex-col rounded-lg border border-ink-800 bg-ink-900/40 lg:w-52"
        aria-label="Task conversations"
      >
        <div className="flex items-center justify-between gap-2 p-3">
          <h1 className="text-sm font-semibold text-ink-100">Tasks</h1>
          <Button size="xs" onClick={() => createTask(projectId)}>
            + New task
          </Button>
        </div>
        <div className="flex max-h-40 gap-1 overflow-auto px-2 pb-2 lg:max-h-none lg:flex-1 lg:flex-col">
          {projectTasks.map((task) => {
            const last = task.turns.at(-1);
            const status =
              last?.state === "running"
                ? "Working…"
                : last?.applying
                  ? "Applying…"
                  : last?.testing
                    ? "Testing…"
                    : last?.error
                      ? "Needs attention"
                      : task.turns.some(needsReview)
                        ? "Review changes"
                        : last
                          ? "Completed"
                          : "Draft";
            return (
              <button
                type="button"
                key={task.id}
                onClick={() => selectTask(task)}
                aria-current={active?.id === task.id ? "page" : undefined}
                className={`min-w-36 rounded-md p-2 text-left lg:min-w-0 ${active?.id === task.id ? "bg-ink-800 text-white" : "text-ink-400 hover:bg-ink-800/50"}`}
              >
                <span className="block truncate text-xs font-medium">{task.title}</span>
                <span
                  className={`mt-1 block text-[10px] ${status === "Review changes" ? "text-warn" : "text-ink-500"}`}
                >
                  {status}
                </span>
              </button>
            );
          })}
          {!projectTasks.length && (
            <p className="px-1 text-xs text-ink-500">Your tasks will appear here.</p>
          )}
        </div>
        <p className="px-3 pb-3 text-[10px] text-ink-500">
          Conversations stay here until the app closes.
        </p>
      </aside>

      <section
        className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-lg border border-ink-800 bg-ink-900/40"
        aria-label="Current task"
      >
        <header className="flex flex-wrap items-center justify-between gap-2 border-b border-ink-800 px-4 py-3">
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold text-ink-100">
              {active?.title ?? "What would you like to work on?"}
            </h2>
            <p className="mt-0.5 text-[11px] text-ink-500">
              {project?.displayName ?? "General conversation"} · Automatic model selection
            </p>
          </div>
          <Button size="xs" variant="ghost" onClick={() => setRoute("models")}>
            Models
          </Button>
        </header>

        <div
          ref={scrollArea}
          onScroll={() => {
            const area = scrollArea.current;
            if (area)
              followBottom.current = area.scrollHeight - area.scrollTop - area.clientHeight < 100;
          }}
          className="min-h-0 flex-1 space-y-6 overflow-y-auto p-4"
        >
          {!active?.turns.length && (
            <div className="mx-auto flex max-w-2xl flex-col items-center py-10 text-center">
              <SparklesIcon size={24} className="mb-4 text-ink-400" />
              <h3 className="text-lg font-semibold text-ink-100">
                Start with what you want to accomplish
              </h3>
              <p className="mt-2 max-w-md text-xs leading-relaxed text-ink-400">
                Ask about your code, describe a bug, or request a change. LeanAI finds relevant
                files and shows proposed edits for review.
              </p>
              <div className="mt-6 grid w-full gap-2 sm:grid-cols-3">
                {STARTERS.map(([label, prompt]) => (
                  <button
                    key={label}
                    type="button"
                    onClick={() => setDraft(prompt!)}
                    disabled={!project}
                    className="rounded-lg border border-ink-750 bg-ink-950/50 p-3 text-left text-xs text-ink-300 hover:border-ink-500 disabled:opacity-40"
                  >
                    {label}
                  </button>
                ))}
              </div>
              {!project && (
                <Button className="mt-4" size="sm" onClick={() => setRoute("overview")}>
                  Open a project for coding tasks
                </Button>
              )}
            </div>
          )}

          {active?.turns.map((turn) => (
            <article key={turn.id} className="mx-auto max-w-4xl space-y-3">
              <div className="ml-auto max-w-[90%] rounded-lg border border-ink-750 bg-ink-800/70 px-4 py-3">
                <p className="mb-1 text-[10px] font-medium text-ink-500">You</p>
                <p className="whitespace-pre-wrap text-sm text-ink-100 select-text">
                  {turn.prompt}
                </p>
              </div>
              {turn.state === "running" && (
                <div role="status" className="rounded-lg border border-ink-800 p-3">
                  <p className="text-xs text-ink-200">
                    <span className="mr-2 inline-block size-2 animate-pulse rounded-full bg-brand" />
                    Working · {Math.floor((Date.now() - turn.startedAt) / 1000)}s
                  </p>
                  <p className="mt-1 text-[11px] text-ink-400">
                    {turn.progress.at(-1)?.detail ?? "Preparing your request…"}
                  </p>
                </div>
              )}
              {turn.progress.length > 0 && !turn.result && (
                <details className="text-xs text-ink-400">
                  <summary className="cursor-pointer">
                    Activity · {turn.progress.length} events
                  </summary>
                  <ol className="mt-2 space-y-2 border-l border-ink-700 pl-3">
                    {turn.progress.map((step, index) => (
                      <li key={index}>{step.detail}</li>
                    ))}
                  </ol>
                </details>
              )}
              {turn.error && (
                <div
                  role="alert"
                  className="rounded border border-danger/40 bg-danger/10 p-3 text-xs text-danger"
                >
                  <p className="font-semibold">{turn.error.message}</p>
                  {turn.error.recovery && <p className="mt-1">{turn.error.recovery}</p>}
                  {turn.state === "failed" && (
                    <Button
                      className="mt-2"
                      size="xs"
                      disabled={busy}
                      onClick={() => setDraft(turn.prompt)}
                    >
                      Use this request again
                    </Button>
                  )}
                </div>
              )}
              {turn.result && (
                <ResultCard
                  result={turn.result}
                  applyResult={turn.applyResult ?? null}
                  applying={turn.applying ?? false}
                  disabled={busy}
                  onResolve={(approved) => void resolve(active.id, turn.id, approved)}
                />
              )}
              {turn.result && project && !needsReview(turn) && (
                <TaskVerification
                  key={`${project.id}-${turn.id}`}
                  taskId={active.id}
                  turn={turn}
                  disabled={busy}
                />
              )}
              {turn.elapsedMs !== undefined && (
                <p className="text-[10px] text-ink-500">{(turn.elapsedMs / 1000).toFixed(1)}s</p>
              )}
            </article>
          ))}
        </div>

        <div className="border-t border-ink-800 bg-ink-950/40 p-3">
          {reviewing && (
            <p className="mb-2 text-xs text-warn">
              Apply or discard the proposed changes before continuing this task.
            </p>
          )}
          <textarea
            value={active?.draft ?? ""}
            onChange={(event) => setDraft(event.target.value)}
            onKeyDown={(event) => {
              if (
                event.key === "Enter" &&
                (event.metaKey || event.ctrlKey) &&
                !event.nativeEvent.isComposing
              ) {
                event.preventDefault();
                if (active) void send(active.id);
              }
            }}
            rows={3}
            placeholder={
              active?.turns.length
                ? "Describe the next step or ask a follow-up…"
                : "Describe a task, ask a question, or request a change…"
            }
            aria-label="Request"
            className="w-full resize-none rounded-md border border-ink-750 bg-ink-900 px-3 py-2 text-sm text-ink-100 placeholder:text-ink-500 focus:border-ink-500 focus:outline-none select-text"
          />
          <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
            <Toggle
              checked={active?.autoApply ?? false}
              onChange={(autoApply) => editTask(active?.id ?? createTask(projectId), { autoApply })}
              label="Apply low-risk changes automatically"
              hint="Other changes wait for your review."
            />
            <Button
              variant="primary"
              size="md"
              onClick={() => active && void send(active.id)}
              disabled={busy || reviewing || !active?.draft.trim()}
            >
              {busy ? "Working…" : "Send"}
              <span className="text-[10px] opacity-70">⌘ / Ctrl ↵</span>
            </Button>
          </div>
        </div>
      </section>
    </div>
  );
}

function TaskVerification({
  taskId,
  turn,
  disabled,
}: {
  taskId: string;
  turn: TaskTurn;
  disabled: boolean;
}) {
  const verify = useTaskStore((state) => state.verify);
  const [commands, setCommands] = useState<string[]>([]);
  const [command, setCommand] = useState("");
  const [loadError, setLoadError] = useState("");
  useEffect(() => {
    let active = true;
    api
      .getCommandAllowlist()
      .then((items) => {
        if (active) {
          setCommands(items);
          setCommand(items[0] ?? "");
        }
      })
      .catch((error) => {
        if (active) setLoadError(toAppError(error).message);
      });
    return () => {
      active = false;
    };
  }, []);
  return (
    <details className="rounded border border-ink-800 p-3 text-xs text-ink-400">
      <summary className="cursor-pointer">
        Checks{" "}
        {turn.testing
          ? "· Running…"
          : turn.testResult
            ? `· ${turn.testResult.passed ? "Passed" : "Failed"}`
            : "· Not run"}
      </summary>
      <p className="mt-2 text-[11px]">Run an allowed command against the current project files.</p>
      {loadError && (
        <p role="alert" className="mt-2 text-danger">
          {loadError}
        </p>
      )}
      {!loadError && !commands.length && <p className="mt-2">No test commands available.</p>}
      {commands.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-2">
          <select
            aria-label="Test command"
            value={command}
            onChange={(event) => setCommand(event.target.value)}
            disabled={disabled}
            className="rounded border border-ink-750 bg-ink-900 px-2 py-1 text-ink-200"
          >
            {commands.map((item) => (
              <option key={item} value={item}>
                {item}
              </option>
            ))}
          </select>
          <Button
            size="xs"
            disabled={disabled || !command}
            onClick={() => void verify(taskId, turn.id, command)}
          >
            {turn.testing ? "Running…" : "Run check"}
          </Button>
        </div>
      )}
      {turn.testResult && (
        <div className="mt-3 space-y-2">
          <p className={turn.testResult.passed ? "text-ok" : "text-danger"}>
            {turn.testResult.command} · {turn.testResult.summary} ·{" "}
            {(turn.testResult.durationMs / 1000).toFixed(1)}s
          </p>
          <pre className="max-h-64 overflow-auto whitespace-pre-wrap rounded bg-ink-950 p-2 text-[11px] select-text">
            {turn.testResult.stdout}
            {turn.testResult.stderr ? `\n${turn.testResult.stderr}` : ""}
          </pre>
        </div>
      )}
    </details>
  );
}

function ResultCard({
  result,
  applyResult,
  applying,
  disabled,
  onResolve,
}: {
  result: PromptRunResponse;
  applyResult: ResolveApprovalResponse | null;
  applying: boolean;
  disabled: boolean;
  onResolve: (approved: boolean) => void;
}) {
  const { metrics, analysis, budget, selection } = result;
  const patches = result.patchProposal?.patches ?? [];
  const applied = result.status === "applied" || applyResult?.patchApplied === true;
  const denied = applyResult?.decision === "denied";
  const filesUpdated = result.filesChanged.length || (applied ? patches.length : 0);
  const reviewing = result.status === "awaiting_approval" && !applyResult;
  const reasons = result.applyDecision?.reasons ?? [];
  const careful = result.applyDecision?.level === "careful_review";
  // A careful review is applied only after the user says they read it.
  const [acknowledged, setAcknowledged] = useState(false);

  return (
    <div className="space-y-3 rounded border border-ink-800 bg-ink-950/60 p-3.5">
      <div className="flex flex-wrap items-center gap-2">
        {applied ? (
          <Chip tone="ok" dot>
            Completed · {filesUpdated} file{filesUpdated === 1 ? "" : "s"} updated
          </Chip>
        ) : denied ? (
          <Chip tone="neutral" dot>
            Discarded · no files changed
          </Chip>
        ) : result.status === "awaiting_approval" ? (
          <Chip tone={careful ? "danger" : "warn"} dot>
            {careful ? "Review carefully · " : "Review "}
            {patches.length} change{patches.length === 1 ? "" : "s"}
          </Chip>
        ) : (
          <Chip tone="ok" dot>
            Completed
          </Chip>
        )}
        {budget.level === "none" ? (
          <Chip tone="neutral">No project context needed</Chip>
        ) : (
          <Chip tone="neutral" title="Project tokens sent to the model versus the whole project">
            Context optimized · {formatNumber(metrics.contextTokensSent)} of ~
            {formatNumber(metrics.repositoryTokensEstimate)} tokens
          </Chip>
        )}
        {result.contextRefreshed || applyResult?.contextRefreshed ? (
          <Chip tone="neutral">Project context refreshed</Chip>
        ) : null}
      </div>

      {result.warnings.map((warning) => (
        <p key={warning} className="flex items-start gap-1.5 text-[11px] text-warn">
          <AlertTriangleIcon size={12} className="mt-0.5 shrink-0" />
          {warning}
        </p>
      ))}

      {result.answer ? (
        <div className="whitespace-pre-wrap text-xs leading-relaxed text-ink-200 select-text">
          {result.answer}
        </div>
      ) : null}

      {result.summary ? <p className="text-xs text-ink-200">{result.summary}</p> : null}

      {reviewing && reasons.length > 0 ? (
        <div
          className={`rounded border p-2.5 text-[11px] ${careful ? "border-danger/40 bg-danger/5" : "border-warn/30 bg-warn/5"}`}
        >
          <p className={`font-semibold ${careful ? "text-danger" : "text-warn"}`}>
            {careful ? "Check these before applying:" : "Held for your review:"}
          </p>
          <ul className="mt-1 list-disc space-y-0.5 pl-4 text-ink-300">
            {reasons.map((reason) => (
              <li key={reason.text}>{reason.text}</li>
            ))}
          </ul>
        </div>
      ) : null}

      {patches.length > 0 ? (
        <div className="space-y-1.5">
          {patches.map((patch) => (
            <details key={patch.path} className="rounded border border-ink-800 bg-ink-900/60">
              <summary className="flex cursor-pointer items-center justify-between px-2.5 py-1.5 text-xs text-ink-200">
                <span className="mono">
                  {patch.path}
                  {patch.isNewFile ? " (new)" : ""}
                </span>
                <span className="mono text-[11px]">
                  <span className="text-ok">+{patch.linesAdded}</span>{" "}
                  <span className="text-danger">−{patch.linesDeleted}</span>
                </span>
              </summary>
              <pre className="mono max-h-72 overflow-auto border-t border-ink-800 p-2.5 text-[11px] leading-snug text-ink-300 select-text">
                {patch.unifiedDiff}
              </pre>
            </details>
          ))}
        </div>
      ) : null}

      {reviewing ? (
        <div className="flex items-center justify-end gap-2">
          {careful ? (
            <label className="mr-auto flex items-center gap-1.5 text-[11px] text-ink-300">
              <input
                type="checkbox"
                checked={acknowledged}
                onChange={(event) => setAcknowledged(event.target.checked)}
                className="size-3.5 rounded accent-ink-100"
              />
              I&apos;ve read these changes
            </label>
          ) : null}
          <Button
            variant="secondary"
            size="sm"
            onClick={() => onResolve(false)}
            disabled={disabled || applying}
          >
            Discard
          </Button>
          <Button
            variant="primary"
            size="sm"
            onClick={() => onResolve(true)}
            disabled={disabled || applying || (careful && !acknowledged)}
          >
            {applying ? "Applying…" : "Apply changes"}
          </Button>
        </div>
      ) : null}

      {applyResult ? (
        <p
          className={`flex items-center gap-1.5 text-xs ${applyResult.patchApplied ? "text-ok" : "text-ink-400"}`}
        >
          <CheckCircleIcon size={12} />
          {applyResult.message}
        </p>
      ) : null}

      <p className="text-[11px] text-ink-500">
        {result.finalModelName} ({result.escalations.length > 0 ? "escalated" : selection.tier}) ·{" "}
        {metrics.llmCalls} call{metrics.llmCalls === 1 ? "" : "s"} ·{" "}
        {formatNumber(metrics.billedInputTokens + metrics.billedOutputTokens)} billed tokens · ~$
        {metrics.estimatedCostUsd.toFixed(4)}
      </p>

      <details className="text-[11px] text-ink-400">
        <summary className="cursor-pointer text-ink-500 hover:text-ink-300">
          How LeanAI handled this
        </summary>
        <div className="mt-2 space-y-2">
          <dl className="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-1">
            <dt className="text-ink-500">Intent</dt>
            <dd className="text-ink-300">
              {INTENT_LABELS[analysis.intent] ?? analysis.intent} (
              {Math.round(analysis.intentConfidence * 100)}%)
            </dd>
            <dt className="text-ink-500">Complexity</dt>
            <dd className="mono text-ink-300">{analysis.complexity.toFixed(1)} / 10</dd>
            <dt className="text-ink-500">Context need</dt>
            <dd className="mono text-ink-300">{analysis.contextRequirement.toFixed(1)} / 10</dd>
            <dt className="text-ink-500">Capabilities</dt>
            <dd className="text-ink-300">{analysis.requiredCapabilities.join(", ") || "basic"}</dd>
            <dt className="text-ink-500">Model</dt>
            <dd className="text-ink-300">
              {selection.displayName} · {selection.tier} (minimum {selection.minimumTier}) ·{" "}
              {selection.explanation}
            </dd>
            <dt className="text-ink-500">Context</dt>
            <dd className="text-ink-300">
              {budget.level === "none"
                ? "none sent"
                : `${budget.sections.join(", ")} · ${formatNumber(metrics.initialContextTokens)} initial + ${formatNumber(metrics.retrievedTokens)} requested tokens (cap ${formatNumber(budget.initialContextTokens + budget.maxRetrievedTokens)})`}
            </dd>
          </dl>
          {result.escalations.length > 0 ? (
            <p className="text-warn">
              Escalated:{" "}
              {result.escalations
                .map((e) => `${e.fromModelId} → ${e.toModelId} (${e.reason})`)
                .join("; ")}
            </p>
          ) : null}
          <ol className="space-y-0.5 border-l border-ink-800 pl-3">
            {result.trace.map((step, index) => (
              <li key={index} className="mono">
                <span className="text-ink-500">{step.kind}</span> {step.detail}
                {step.tokens > 0 ? (
                  <span className="text-ink-600"> · {formatNumber(step.tokens)} tok</span>
                ) : null}
              </li>
            ))}
          </ol>
        </div>
      </details>
    </div>
  );
}
