import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { PromptRunResponse } from "../../ipc/types";

const runPrompt = vi.fn();
const resolveApproval = vi.fn();
const getCommandAllowlist = vi.fn();
const runTesterStep = vi.fn();
vi.mock("../../ipc/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/client")>();
  return { ...actual, api: { runPrompt, resolveApproval, getCommandAllowlist, runTesterStep } };
});

const { PromptRunner } = await import("./PromptRunner");
const { useTaskStore } = await import("../../store/useTaskStore");
const { useAppStore } = await import("../../store/useAppStore");

function response(overrides: Partial<PromptRunResponse> = {}): PromptRunResponse {
  return {
    runId: "run-1",
    status: "completed",
    answer: "Recursion is a function calling itself.",
    summary: null,
    analysis: {
      intent: "EXPLANATION",
      intentConfidence: 0.8,
      changeShare: 0,
      complexity: 1.2,
      contextRequirement: 0,
      signals: {
        reasoningDifficulty: 2,
        scope: 1.5,
        repositoryDependency: 0.5,
        domainDifficulty: 1,
        risk: 0.5,
        crossFileDependency: 0,
        architectureDependency: 0,
        projectKnowledge: 0.3,
        operationCount: 1,
        layers: [],
        codeArtifacts: [],
        projectTerms: [],
      },
      required: {
        coding: 0,
        reasoning: 1.6,
        creativity: 2,
        longContext: 0,
        instructionFollowing: 4,
      },
      requiredCapabilities: ["instruction following"],
      explanation: [],
    },
    budget: {
      level: "none",
      sections: [],
      initialContextTokens: 0,
      maxRetrievedTokens: 0,
      maxFileTokens: 0,
      maxRounds: 1,
      maxOutputTokens: 2000,
      allowRetrieval: false,
    },
    selection: {
      modelId: "anthropic/claude-haiku-4-5",
      displayName: "Claude Haiku 4.5",
      provider: "anthropic",
      apiModel: "claude-haiku-4-5",
      tier: "fast",
      minimumTier: "fast",
      estimatedCostUsd: 0.001,
      escalationChain: [],
      capabilityShortfall: [],
      explanation: "Cheapest available model.",
    },
    finalModelId: "anthropic/claude-haiku-4-5",
    finalModelName: "Claude Haiku 4.5",
    escalations: [],
    retrievals: [],
    searches: [],
    patchProposal: null,
    validatorVerdict: null,
    applyDecision: null,
    pendingApproval: null,
    filesChanged: [],
    contextRefreshed: false,
    metrics: {
      repositoryTokensEstimate: 50000,
      initialContextTokens: 0,
      retrievedTokens: 0,
      contextTokensSent: 0,
      tokensAvoided: 50000,
      billedInputTokens: 40,
      billedOutputTokens: 120,
      llmCalls: 1,
      estimatedCostUsd: 0.00064,
    },
    trace: [],
    warnings: [],
    ...overrides,
  };
}

describe("PromptRunner", () => {
  beforeEach(() => {
    useTaskStore.setState({ tasks: [], activeByProject: {} });
    useAppStore.setState({ project: null });
    runPrompt.mockReset();
    resolveApproval.mockReset();
    getCommandAllowlist.mockReset().mockResolvedValue(["npm test"]);
    runTesterStep.mockReset();
  });

  it("sends only the prompt and shows a simple answer", async () => {
    runPrompt.mockResolvedValue(response());
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Explain recursion.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));

    expect(runPrompt).toHaveBeenCalledWith(
      { prompt: "Explain recursion.", autoApply: false },
      expect.any(Function),
    );
    expect(await screen.findByText("Recursion is a function calling itself.")).toBeInTheDocument();
    expect(screen.getByText("No project context needed")).toBeInTheDocument();
  });

  it("reviews then applies a change through the scoped approval", async () => {
    const proposal = {
      summary: "Rename the login button",
      rationale: "",
      affectedFiles: ["src/Login.tsx"],
      patches: [
        {
          path: "src/Login.tsx",
          unifiedDiff: "@@ -1,1 +1,1 @@\n-Login\n+Sign In\n",
          isNewFile: false,
          isDeleted: false,
          linesAdded: 1,
          linesDeleted: 1,
        },
      ],
      sha256Hash: "a".repeat(64),
    };
    runPrompt.mockResolvedValue(
      response({
        status: "awaiting_approval",
        answer: null,
        summary: proposal.summary,
        patchProposal: proposal,
        pendingApproval: {
          id: "appr-1",
          runId: "run-1",
          capability: "write_file",
          projectRoot: "/p",
          affectedPaths: ["src/Login.tsx"],
          patchHash: proposal.sha256Hash,
          token: "t",
          createdAtMs: 0,
          expiresAtMs: 1,
          state: "pending",
        },
        budget: { ...response().budget, level: "minimal", sections: ["quick_reference"] },
        metrics: {
          ...response().metrics,
          contextTokensSent: 900,
          initialContextTokens: 600,
          retrievedTokens: 300,
        },
      }),
    );
    resolveApproval.mockResolvedValue({
      approvalId: "appr-1",
      decision: "approved",
      patchApplied: true,
      rollbackPerformed: false,
      message: "Transactional patch successfully applied to 1 file(s).",
      contextRefreshed: true,
    });

    render(<PromptRunner />);
    await userEvent.type(
      screen.getByLabelText("Request"),
      "Change the Login button text to Sign In",
    );
    await userEvent.click(screen.getByRole("button", { name: /send/i }));

    expect(await screen.findByText("Review 1 change")).toBeInTheDocument();
    expect(screen.getByText(/Context optimized/)).toHaveTextContent("900");
    expect(screen.getByRole("button", { name: /send/i })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Apply changes" }));

    expect(resolveApproval).toHaveBeenCalledWith({
      approvalId: "appr-1",
      approved: true,
      approver: "user",
      proposal,
    });
    expect(await screen.findByText("Completed · 1 file updated")).toBeInTheDocument();
    expect(screen.getByText("Project context refreshed")).toBeInTheDocument();
  });

  it("shows why a change was held back and gates a careful review", async () => {
    const proposal = {
      summary: "Rename the package",
      rationale: "",
      affectedFiles: ["package.json"],
      patches: [
        {
          path: "package.json",
          unifiedDiff: '@@ -1,1 +1,1 @@\n-"demo"\n+"demo-app"\n',
          isNewFile: false,
          isDeleted: false,
          linesAdded: 1,
          linesDeleted: 1,
        },
      ],
      sha256Hash: "b".repeat(64),
    };
    runPrompt.mockResolvedValue(
      response({
        status: "awaiting_approval",
        answer: null,
        summary: proposal.summary,
        patchProposal: proposal,
        applyDecision: {
          level: "careful_review",
          reasons: [
            {
              level: "careful_review",
              text: "Changes package.json (build or dependency configuration).",
            },
          ],
        },
        pendingApproval: {
          id: "appr-2",
          runId: "run-1",
          capability: "write_file",
          projectRoot: "/p",
          affectedPaths: ["package.json"],
          patchHash: proposal.sha256Hash,
          token: "t",
          createdAtMs: 0,
          expiresAtMs: 1,
          state: "pending",
        },
      }),
    );
    resolveApproval.mockResolvedValue({
      approvalId: "appr-2",
      decision: "approved",
      patchApplied: true,
      rollbackPerformed: false,
      message: "Transactional patch successfully applied to 1 file(s).",
      contextRefreshed: false,
    });

    render(<PromptRunner />);
    await userEvent.click(screen.getByLabelText(/Apply low-risk changes automatically/));
    await userEvent.type(screen.getByLabelText("Request"), "Rename the package to demo-app");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));

    expect(runPrompt).toHaveBeenCalledWith(
      {
        prompt: "Rename the package to demo-app",
        autoApply: true,
      },
      expect.any(Function),
    );
    expect(await screen.findByText("Review carefully · 1 change")).toBeInTheDocument();
    expect(
      screen.getByText("Changes package.json (build or dependency configuration)."),
    ).toBeInTheDocument();

    const apply = screen.getByRole("button", { name: "Apply changes" });
    expect(apply).toBeDisabled();
    await userEvent.click(screen.getByLabelText("I've read these changes"));
    expect(apply).toBeEnabled();
    await userEvent.click(apply);
    expect(resolveApproval).toHaveBeenCalledWith(
      expect.objectContaining({ approvalId: "appr-2", approved: true }),
    );
  });

  it("shows backend errors with their recovery step", async () => {
    runPrompt.mockRejectedValue({
      code: "no_model_available",
      message: "No model is available.",
      recovery: "Open Models to add an API key or start a local model.",
      retryable: false,
    });
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Hi");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Open Models to add an API key");
  });
  it("keeps previous answers and sends recent context with a follow-up", async () => {
    runPrompt
      .mockResolvedValueOnce(response())
      .mockResolvedValueOnce(response({ runId: "run-2", answer: "Here is an example." }));
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Explain recursion.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await screen.findByText("Recursion is a function calling itself.");
    await userEvent.type(screen.getByLabelText("Request"), "Give me an example.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(await screen.findByText("Here is an example.")).toBeInTheDocument();
    expect(screen.getByText("Recursion is a function calling itself.")).toBeInTheDocument();
    expect(runPrompt.mock.calls[1]?.[0].history).toEqual([
      { role: "user", content: "Explain recursion." },
      {
        role: "assistant",
        content: expect.stringContaining("Recursion is a function calling itself."),
      },
    ]);
  });

  it("retains running tasks across navigation and shows actual backend progress", async () => {
    let finish!: (result: PromptRunResponse) => void;
    runPrompt.mockImplementation((_request, progress) => {
      progress({ kind: "read_files", detail: "Read src/Login.tsx", tokens: 40, modelId: null });
      return new Promise<PromptRunResponse>((resolve) => {
        finish = resolve;
      });
    });
    const view = render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Inspect the login code");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(screen.getByRole("status")).toHaveTextContent("Read src/Login.tsx");
    view.unmount();
    finish(response());
    render(<PromptRunner />);
    expect(await screen.findByText("Recursion is a function calling itself.")).toBeInTheDocument();
    expect(runPrompt).toHaveBeenCalledTimes(1);
  });

  it("starts independent tasks without leaking another task's conversation", async () => {
    runPrompt.mockResolvedValue(response());
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Explain recursion.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await screen.findByText("Recursion is a function calling itself.");
    await userEvent.click(screen.getByRole("button", { name: /new task/i }));
    await userEvent.type(screen.getByLabelText("Request"), "A separate question");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(runPrompt.mock.calls[1]?.[0].history).toBeUndefined();
    expect(useTaskStore.getState().tasks).toHaveLength(2);
  });

  it("does not submit an old project's task in a newly opened project", async () => {
    const store = useTaskStore.getState();
    const id = store.createTask("another-project");
    store.editTask(id, { draft: "Change this project's code" });
    await store.send(id);
    expect(runPrompt).not.toHaveBeenCalled();
  });
  it("runs a real check and keeps its failed output on the task", async () => {
    useAppStore.setState({ project: { id: "p1", displayName: "demo" } as never });
    runPrompt.mockResolvedValue(response({ status: "applied", filesChanged: ["src/Login.tsx"] }));
    runTesterStep.mockResolvedValue({
      command: "npm test",
      passed: false,
      stdout: "1 assertion failed",
      stderr: "",
      durationMs: 200,
      summary: "Tests failed",
      timestampMs: 0,
    });
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Fix the login button");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await screen.findByText("Completed · 1 file updated");
    await userEvent.click(screen.getByText("Checks · Not run"));
    await userEvent.click(await screen.findByRole("button", { name: "Run check" }));
    expect(await screen.findByText("1 assertion failed")).toBeInTheDocument();
    expect(screen.getByText("Checks · Failed")).toBeInTheDocument();
    expect(runTesterStep).toHaveBeenCalledWith({ runId: "run-1", command: "npm test" });
    await userEvent.type(screen.getByLabelText("Request"), "Fix the failing test.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(runPrompt.mock.calls[1]?.[0].history[1].content).toContain("npm test failed");
    expect(runPrompt.mock.calls[1]?.[0].history[1].content).toContain("1 assertion failed");
  });

  it("restores a failed request for retry without losing earlier answers", async () => {
    runPrompt
      .mockResolvedValueOnce(response())
      .mockRejectedValueOnce(new Error("Model disconnected"));
    render(<PromptRunner />);
    await userEvent.type(screen.getByLabelText("Request"), "Explain recursion.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    await screen.findByText("Recursion is a function calling itself.");
    await userEvent.type(screen.getByLabelText("Request"), "Show an example.");
    await userEvent.click(screen.getByRole("button", { name: /send/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Model disconnected");
    expect(screen.getByLabelText("Request")).toHaveValue("Show an example.");
    expect(screen.getByText("Recursion is a function calling itself.")).toBeInTheDocument();
  });
});
