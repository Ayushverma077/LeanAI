import { create } from "zustand";

import { api, toAppError } from "../ipc/client";
import type {
  AppError,
  PromptRunResponse,
  PromptTraceStep,
  ResolveApprovalResponse,
  RunPromptRequest,
  TesterArtifact,
} from "../ipc/types";
import { useAppStore } from "./useAppStore";

export interface TaskTurn {
  id: string;
  prompt: string;
  state: "running" | "completed" | "failed";
  startedAt: number;
  elapsedMs?: number;
  progress: PromptTraceStep[];
  result?: PromptRunResponse;
  error?: AppError;
  applyResult?: ResolveApprovalResponse;
  applying?: boolean;
  testing?: boolean;
  testResult?: TesterArtifact;
}

export interface TaskConversation {
  id: string;
  projectId: string | null;
  title: string;
  draft: string;
  autoApply: boolean;
  turns: TaskTurn[];
}

/** Keep recent conversation pairs bounded; diffs and source files are retrieved afresh. */
export function conversationHistory(turns: TaskTurn[]): NonNullable<RunPromptRequest["history"]> {
  return turns
    .filter((turn) => turn.result)
    .slice(-6)
    .flatMap((turn) => {
      const result = turn.result!;
      const status =
        turn.applyResult?.patchApplied || result.status === "applied"
          ? "Changes applied."
          : turn.applyResult?.decision === "denied"
            ? "Changes discarded."
            : result.status === "awaiting_approval"
              ? "Changes proposed; not applied."
              : "Completed.";
      return [
        { role: "user" as const, content: turn.prompt.slice(0, 1200) },
        {
          role: "assistant" as const,
          content:
            `${status}\nFiles: ${result.patchProposal?.affectedFiles.join(", ") ?? result.filesChanged.join(", ")}\n${turn.testResult ? `Check: ${turn.testResult.command} ${turn.testResult.passed ? "passed" : "failed"}\n${turn.testResult.stdout.slice(-300)}\n${turn.testResult.stderr.slice(-300)}` : "Checks not run."}\n${result.answer ?? result.summary ?? ""}`.slice(
              0,
              1200,
            ),
        },
      ];
    });
}

export function needsReview(turn: TaskTurn) {
  return Boolean(turn.result?.pendingApproval && !turn.applyResult);
}

interface TaskStore {
  tasks: TaskConversation[];
  activeByProject: Record<string, string>;
  createTask: (projectId: string | null) => string;
  selectTask: (task: TaskConversation) => void;
  editTask: (id: string, update: Partial<Pick<TaskConversation, "draft" | "autoApply">>) => void;
  send: (id: string) => Promise<void>;
  resolve: (taskId: string, turnId: string, approved: boolean) => Promise<void>;
  verify: (taskId: string, turnId: string, command: string) => Promise<void>;
}

export const projectTaskKey = (id: string | null) => id ?? "__general__";

export const useTaskStore = create<TaskStore>((set, get) => {
  const updateTurn = (taskId: string, turnId: string, update: Partial<TaskTurn>) =>
    set((state) => ({
      tasks: state.tasks.map((task) =>
        task.id !== taskId
          ? task
          : {
              ...task,
              turns: task.turns.map((turn) => (turn.id === turnId ? { ...turn, ...update } : turn)),
            },
      ),
    }));
  return {
    tasks: [],
    activeByProject: {},
    createTask: (projectId) => {
      const id = crypto.randomUUID();
      set((state) => ({
        tasks: [
          { id, projectId, title: "New task", draft: "", autoApply: false, turns: [] },
          ...state.tasks,
        ],
        activeByProject: { ...state.activeByProject, [projectTaskKey(projectId)]: id },
      }));
      return id;
    },
    selectTask: (task) =>
      set((state) => ({
        activeByProject: { ...state.activeByProject, [projectTaskKey(task.projectId)]: task.id },
      })),
    editTask: (id, update) =>
      set((state) => ({
        tasks: state.tasks.map((task) => (task.id === id ? { ...task, ...update } : task)),
      })),
    send: async (id) => {
      const task = get().tasks.find((item) => item.id === id);
      if (
        !task?.draft.trim() ||
        task.projectId !== (useAppStore.getState().project?.id ?? null) ||
        get().tasks.some((item) =>
          item.turns.some((turn) => turn.state === "running" || turn.applying || turn.testing),
        ) ||
        task.turns.some(needsReview)
      )
        return;
      const prompt = task.draft.trim();
      const turn: TaskTurn = {
        id: crypto.randomUUID(),
        prompt,
        state: "running",
        startedAt: Date.now(),
        progress: [],
      };
      const history = conversationHistory(task.turns);
      set((state) => ({
        tasks: state.tasks.map((item) =>
          item.id === id
            ? {
                ...item,
                title: item.turns.length ? item.title : prompt.slice(0, 70),
                draft: "",
                turns: [...item.turns, turn],
              }
            : item,
        ),
      }));
      try {
        const result = await api.runPrompt(
          { prompt, autoApply: task.autoApply, ...(history.length ? { history } : {}) },
          (step) => {
            const current = get()
              .tasks.find((item) => item.id === id)
              ?.turns.find((item) => item.id === turn.id);
            if (current?.state === "running")
              updateTurn(id, turn.id, { progress: [...current.progress, step] });
          },
        );
        updateTurn(id, turn.id, {
          result,
          state: "completed",
          elapsedMs: Date.now() - turn.startedAt,
        });
      } catch (error) {
        updateTurn(id, turn.id, {
          error: toAppError(error),
          state: "failed",
          elapsedMs: Date.now() - turn.startedAt,
        });
        // Restore only when no newer draft has been written.
        if (!get().tasks.find((item) => item.id === id)?.draft)
          get().editTask(id, { draft: prompt });
      }
    },
    resolve: async (taskId, turnId, approved) => {
      const task = get().tasks.find((item) => item.id === taskId);
      const turn = task?.turns.find((item) => item.id === turnId);
      if (
        !turn?.result?.pendingApproval ||
        !turn.result.patchProposal ||
        turn.applying ||
        turn.applyResult ||
        task?.projectId !== (useAppStore.getState().project?.id ?? null) ||
        get().tasks.some((item) =>
          item.turns.some((entry) => entry.state === "running" || entry.applying || entry.testing),
        )
      )
        return;
      updateTurn(taskId, turnId, { applying: true, error: undefined });
      try {
        const applyResult = await api.resolveApproval({
          approvalId: turn.result.pendingApproval.id,
          approved,
          approver: "user",
          proposal: turn.result.patchProposal,
        });
        updateTurn(taskId, turnId, { applyResult });
      } catch (error) {
        updateTurn(taskId, turnId, { error: toAppError(error) });
      } finally {
        updateTurn(taskId, turnId, { applying: false });
      }
    },
    verify: async (taskId, turnId, command) => {
      const task = get().tasks.find((item) => item.id === taskId);
      const turn = task?.turns.find((item) => item.id === turnId);
      if (
        !task?.projectId ||
        task.projectId !== useAppStore.getState().project?.id ||
        !turn?.result ||
        needsReview(turn) ||
        !command.trim() ||
        get().tasks.some((item) =>
          item.turns.some((entry) => entry.state === "running" || entry.applying || entry.testing),
        )
      )
        return;
      updateTurn(taskId, turnId, { testing: true, testResult: undefined, error: undefined });
      try {
        const testResult = await api.runTesterStep({ runId: turn.result.runId, command });
        updateTurn(taskId, turnId, { testResult });
      } catch (error) {
        updateTurn(taskId, turnId, { error: toAppError(error) });
      } finally {
        updateTurn(taskId, turnId, { testing: false });
      }
    },
  };
});
