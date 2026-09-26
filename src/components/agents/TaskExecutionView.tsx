import { useState, useEffect } from "react";
import { useAppStore } from "../../store/useAppStore";
import { api } from "../../ipc/client";
import type {
  TaskRunResponse,
  StepRecord,
  RetrievalResult,
  ProposedFile,
  ResolveApprovalResponse,
  TesterArtifact,
} from "../../ipc/types";
import { Button, Chip, formatNumber } from "../primitives";
import {
  CheckCircleIcon,
  AlertTriangleIcon,
  FileCodeIcon,
  SparklesIcon,
  ZapIcon,
  ShieldCheckIcon,
  SearchIcon,
  TerminalIcon,
  UploadCloudIcon,
  XIcon,
} from "../icons";
import { GitSyncModal } from "../git/GitSyncModal";

export function TaskExecutionView() {
  const { bundle, selection, git } = useAppStore();
  const [promptInput, setPromptInput] = useState("");
  const [contextFiles, setContextFiles] = useState<string[]>([]);
  const [activeTab, setActiveTab] = useState<
    "trace" | "plan" | "citations" | "validator" | "tester" | "retrieval"
  >("trace");

  const [isLoading, setIsLoading] = useState(false);
  const [currentRun, setCurrentRun] = useState<TaskRunResponse | null>(null);
  const [approvalResult, setApprovalResult] = useState<ResolveApprovalResponse | null>(null);
  const [showGitSync, setShowGitSync] = useState(false);
  const [approvalSubmitting, setApprovalSubmitting] = useState(false);

  // Tester state
  const [testerCommand, setTesterCommand] = useState("cargo test");
  const [testerLoading, setTesterLoading] = useState(false);
  const [testerArtifact, setTesterArtifact] = useState<TesterArtifact | null>(null);
  const [allowlist, setAllowlist] = useState<string[]>([
    "cargo test",
    "cargo check",
    "npm test",
    "pytest",
  ]);

  // Semantic Retrieval state
  const [retrievalQuery, setRetrievalQuery] = useState("");
  const [retrievalLoading, setRetrievalLoading] = useState(false);
  const [retrievalResult, setRetrievalResult] = useState<RetrievalResult | null>(null);

  // Load allowlist on mount
  useEffect(() => {
    api
      .getCommandAllowlist()
      .then((cmds) => {
        if (cmds && cmds.length > 0 && cmds[0]) {
          setAllowlist(cmds);
          setTesterCommand(cmds[0]);
        }
      })
      .catch(() => {});
  }, []);

  // Default initial context files from current bundle or selection
  useEffect(() => {
    if (contextFiles.length === 0) {
      if (
        bundle?.manifest?.selection?.includedFiles &&
        bundle.manifest.selection.includedFiles.length > 0
      ) {
        setContextFiles(bundle.manifest.selection.includedFiles.slice(0, 5));
      } else if (selection.files.size > 0) {
        setContextFiles([...selection.files].slice(0, 5));
      } else {
        setContextFiles(["src/main.rs", "src/lib.rs"]);
      }
    }
  }, [bundle, selection, contextFiles.length]);

  const handleRunTask = async () => {
    if (!promptInput.trim() || isLoading) return;
    setIsLoading(true);
    setApprovalResult(null);

    try {
      const response = await api.startTaskRun({
        objective: promptInput.trim(),
        contextFiles: contextFiles.length > 0 ? contextFiles : ["src/main.rs"],
        testCommand: testerCommand.trim() ? testerCommand.trim() : undefined,
        maxTokens: 128000,
        maxCostUsd: 1.5,
      });
      setCurrentRun(response);
      if (response.testerVerdict) {
        setTesterArtifact(response.testerVerdict);
      }
    } catch (err) {
      console.error("Failed to run agent task:", err);
    } finally {
      setIsLoading(false);
    }
  };

  const handleRunTester = async (cmdToRun?: string) => {
    const cmd = cmdToRun || testerCommand;
    if (!cmd.trim() || testerLoading) return;
    setTesterLoading(true);

    try {
      const artifact = await api.runTesterStep({
        runId: currentRun?.runId,
        command: cmd.trim(),
      });
      setTesterArtifact(artifact);
      setActiveTab("tester");
    } catch (err) {
      console.error("Failed to run tester command:", err);
    } finally {
      setTesterLoading(false);
    }
  };

  const handleResolveApproval = async (approved: boolean) => {
    if (!currentRun?.pendingApproval || approvalSubmitting) return;
    setApprovalSubmitting(true);

    try {
      const res = await api.resolveApproval({
        approvalId: currentRun.pendingApproval.approvalId,
        approved,
        approver: "user",
        proposal: currentRun.patchProposal,
      });
      setApprovalResult(res);
      // Update local state to reflect decision
      setCurrentRun((prev) => {
        if (!prev) return null;
        return {
          ...prev,
          status: approved ? "completed" : "denied",
          pendingApproval: undefined,
        };
      });
    } catch (err) {
      console.error("Failed to resolve approval:", err);
    } finally {
      setApprovalSubmitting(false);
    }
  };

  const handleSemanticSearch = async () => {
    if (!retrievalQuery.trim() || retrievalLoading) return;
    setRetrievalLoading(true);

    try {
      const res = await api.queryTaskContext(retrievalQuery.trim(), contextFiles, true);
      setRetrievalResult(res);
    } catch (err) {
      console.error("Failed semantic retrieval:", err);
    } finally {
      setRetrievalLoading(false);
    }
  };

  const addFileToContext = (filePath: string) => {
    if (!contextFiles.includes(filePath)) {
      setContextFiles([...contextFiles, filePath]);
    }
  };

  const removeFileFromContext = (filePath: string) => {
    setContextFiles(contextFiles.filter((p) => p !== filePath));
  };

  const totalTokensSpent =
    currentRun?.steps.reduce((acc: number, s: StepRecord) => acc + s.tokensUsed, 0) ?? 7600;

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-800/60 px-4 py-2.5 bg-ink-950/40">
        <div className="flex items-center gap-2.5">
          <div className="flex size-7 items-center justify-center rounded border border-ink-750 bg-ink-850 text-ink-300">
            <ZapIcon size={14} />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h2 className="text-xs font-semibold text-ink-100">
                {currentRun ? `Task Run #${currentRun.runId.slice(0, 8)}` : "Agent Orchestration"}
              </h2>
              {currentRun?.status === "awaiting_approval" ? (
                <Chip tone="warn" dot>
                  Awaiting Approval
                </Chip>
              ) : currentRun?.status === "completed" ? (
                <Chip tone="ok" dot>
                  Approved & Applied
                </Chip>
              ) : currentRun?.status === "denied" ? (
                <Chip tone="danger" dot>
                  Denied
                </Chip>
              ) : (
                <Chip tone="neutral" dot>
                  Ready
                </Chip>
              )}
            </div>
            <p className="text-[11px] text-ink-400">
              {currentRun
                ? `Objective: ${currentRun.plan.objective}`
                : "Guided multi-agent execution with zero-egress policy and scoped approvals"}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-4 text-xs">
          <div className="flex items-center gap-1.5 text-ink-400">
            <SparklesIcon size={11} className="text-ink-500" />
            <span className="text-ink-500">Tokens:</span>
            <span className="mono font-semibold text-ink-200">
              {formatNumber(totalTokensSpent)}
            </span>
          </div>

          <div className="h-3 w-px bg-ink-800" />

          {/* Navigation Tabs */}
          <div className="flex gap-1">
            <button
              type="button"
              onClick={() => setActiveTab("trace")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "trace" ? "bg-ink-800 text-white" : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Trace ({currentRun?.steps.length ?? 5})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("plan")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "plan" ? "bg-ink-800 text-white" : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Plan ({currentRun?.plan.subtasks.length ?? 0})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("citations")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "citations"
                  ? "bg-ink-800 text-white"
                  : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Citations ({currentRun?.contextManifest.citations.length ?? 0})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("validator")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "validator"
                  ? "bg-ink-800 text-white"
                  : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Validator ({currentRun?.validatorVerdict.checks.length ?? 0})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("tester")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "tester" ? "bg-ink-800 text-white" : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Tester ({testerArtifact ? (testerArtifact.passed ? "PASSED" : "FAILED") : "Ready"})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("retrieval")}
              className={`rounded px-2 py-0.5 text-xs font-medium transition-colors ${
                activeTab === "retrieval"
                  ? "bg-ink-800 text-white"
                  : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Context Retrieval
            </button>
          </div>
        </div>
      </div>

      {/* Approval Banner if pending */}
      {currentRun?.pendingApproval && (
        <div className="border-b border-warn/30 bg-warn/10 p-3.5">
          <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <AlertTriangleIcon size={14} className="text-warn" />
                <span className="text-xs font-semibold text-warn">
                  Approval Required for File Modification (ADR 0010)
                </span>
                <span className="font-mono text-[10px] text-ink-400">
                  Diff SHA: {currentRun.pendingApproval.diffHash.slice(0, 12)}
                </span>
              </div>
              <p className="text-xs text-ink-300">
                Agent proposed patch for:{" "}
                <span className="font-mono font-medium text-ink-100">
                  {currentRun.pendingApproval.targetPaths.join(", ")}
                </span>
                . Changes are staged in memory; no files have been written.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button
                variant="secondary"
                size="sm"
                onClick={() => handleResolveApproval(false)}
                disabled={approvalSubmitting}
              >
                Deny & Rollback
              </Button>
              <Button
                variant="primary"
                size="sm"
                onClick={() => handleResolveApproval(true)}
                disabled={approvalSubmitting}
              >
                {approvalSubmitting ? "Applying..." : "Approve & Apply"}
              </Button>
            </div>
          </div>

          {/* Unified Diff Preview */}
          {currentRun.patchProposal?.patches?.[0] && (
            <div className="mt-3 rounded border border-ink-800 bg-ink-950 p-2.5 font-mono text-[11px] leading-snug">
              <div className="mb-1 text-ink-400">
                Diff preview ({currentRun.patchProposal.patches[0].path}):
              </div>
              <pre className="overflow-x-auto text-ink-200">
                {currentRun.patchProposal.patches[0].unifiedDiff}
              </pre>
            </div>
          )}
        </div>
      )}

      {/* Approval Result Banner */}
      {approvalResult && (
        <div
          className={`border-b p-3 text-xs ${
            approvalResult.patchApplied
              ? "border-ok/30 bg-ok/10 text-ok"
              : "border-critical/30 bg-critical/10 text-critical"
          }`}
        >
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <CheckCircleIcon size={14} />
              <span className="font-semibold">{approvalResult.message}</span>
              {approvalResult.rollbackPerformed && (
                <span className="text-[11px] text-ink-400 font-mono">
                  (Automatic rollback clean)
                </span>
              )}
            </div>
            {approvalResult.patchApplied && git?.isRepository && (
              <Button
                variant="secondary"
                size="xs"
                onClick={() => setShowGitSync(true)}
                className="flex items-center gap-1 shrink-0"
              >
                <UploadCloudIcon size={12} />
                <span>Push to Remote</span>
              </Button>
            )}
          </div>
        </div>
      )}

      {/* Tab Panels */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {activeTab === "trace" && (
          <div className="relative pl-5 space-y-3.5 before:absolute before:bottom-2 before:left-[9px] before:top-2 before:w-px before:bg-ink-800">
            {(
              currentRun?.steps ?? [
                {
                  id: "step-1",
                  role: "orchestrator",
                  action: "Preflight verified: bounds, policy, and budget constraints initialized.",
                  tokensUsed: 250,
                  timestampMs: Date.now() - 30000,
                  details: "Project root locked; budget cap: 128,000 tokens.",
                  status: "completed",
                },
                {
                  id: "step-2",
                  role: "planner",
                  action: "Constructed structured plan with 3 subtasks.",
                  tokensUsed: 1200,
                  timestampMs: Date.now() - 25000,
                  details: "Definition of success: verified zero egress and clean patch execution.",
                  status: "completed",
                },
                {
                  id: "step-3",
                  role: "context_builder",
                  action: "Assembled citations for 2 source files.",
                  tokensUsed: 2100,
                  timestampMs: Date.now() - 18000,
                  details:
                    "Generated SHA-256 hashes and line ranges for citations; 0 files omitted.",
                  status: "completed",
                },
                {
                  id: "step-4",
                  role: "coder",
                  action: "Generated read-only patch proposal.",
                  tokensUsed: 3400,
                  timestampMs: Date.now() - 10000,
                  details:
                    "Unified diff created with relative path citations; project files untouched.",
                  status: "completed",
                },
                {
                  id: "step-5",
                  role: "validator",
                  action: "Deterministic validation passed: 0 secret leaks, 0 boundary violations.",
                  tokensUsed: 650,
                  timestampMs: Date.now() - 2000,
                  details: "Checked 4 deterministic safety and isolation rules.",
                  status: "completed",
                },
              ]
            ).map((step) => (
              <div key={step.id} className="relative group">
                <div className="absolute -left-5 top-1.5 flex size-3.5 items-center justify-center rounded-full border border-ok/40 bg-ok/15 text-ok">
                  <CheckCircleIcon size={9} />
                </div>
                <div className="rounded border border-ink-800/80 bg-ink-950/70 p-3 shadow-2xs hover:border-ink-750 transition-colors">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-1.5">
                      <span className="text-xs font-semibold text-ink-100 capitalize">
                        {step.role.replace("_", " ")}
                      </span>
                      <span className="text-[10px] text-ink-500 font-mono">({step.status})</span>
                    </div>
                    <div className="flex items-center gap-1.5 text-[10px] text-ink-500">
                      <span className="mono">{formatNumber(step.tokensUsed)} tok</span>
                      <span>·</span>
                      <span className="mono">
                        {new Date(step.timestampMs).toLocaleTimeString()}
                      </span>
                    </div>
                  </div>
                  <p className="mt-1 text-xs text-ink-300">{step.action}</p>
                  {step.details && (
                    <p className="mt-1.5 text-[11px] leading-relaxed text-ink-400 font-mono bg-ink-900/60 p-2 rounded border border-ink-800/70">
                      {step.details}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}

        {activeTab === "plan" && (
          <div className="space-y-3">
            {currentRun ? (
              <>
                <div className="rounded border border-ink-800 bg-ink-950 p-3">
                  <h3 className="text-xs font-semibold text-ink-200">Definition of Success</h3>
                  <p className="mt-1 text-xs text-ink-400">{currentRun.plan.definitionOfSuccess}</p>
                </div>
                <div className="space-y-2">
                  <h3 className="text-xs font-semibold text-ink-300">
                    Subtasks ({currentRun.plan.subtasks.length})
                  </h3>
                  {currentRun.plan.subtasks.map((st) => (
                    <div
                      key={st.id}
                      className="rounded border border-ink-800/80 bg-ink-950/60 p-3 space-y-1.5"
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-xs font-medium text-ink-100">
                          {st.id}: {st.title}
                        </span>
                        <span className="font-mono text-[10px] text-ink-500">
                          Target: {st.targetFiles.join(", ") || "General"}
                        </span>
                      </div>
                      <p className="text-xs text-ink-400">{st.description}</p>
                      <div className="rounded bg-ink-900/50 p-1.5 font-mono text-[11px] text-ink-300 border border-ink-800/50">
                        Expected outcome: {st.expectedOutcome}
                      </div>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div className="rounded border border-ink-800/60 bg-ink-950/40 p-6 text-center text-xs text-ink-400">
                Dispatch a task below to generate an architectural plan artifact.
              </div>
            )}
          </div>
        )}

        {activeTab === "citations" && (
          <div className="space-y-3">
            {currentRun ? (
              <>
                <div className="flex items-center justify-between text-xs text-ink-400">
                  <span>
                    Total Cited Tokens:{" "}
                    <span className="font-semibold text-ink-200">
                      {formatNumber(currentRun.contextManifest.totalEstimatedTokens)}
                    </span>
                  </span>
                  <span>{currentRun.contextManifest.citations.length} file citation(s)</span>
                </div>
                <div className="space-y-2">
                  {currentRun.contextManifest.citations.map((c) => (
                    <div
                      key={c.relativePath}
                      className="rounded border border-ink-800 bg-ink-950/70 p-2.5 flex items-center justify-between"
                    >
                      <div className="flex items-center gap-2">
                        <FileCodeIcon size={14} className="text-ink-400" />
                        <span className="font-mono text-xs text-ink-200">{c.relativePath}</span>
                      </div>
                      <div className="flex items-center gap-3 text-[11px] font-mono text-ink-400">
                        <span>SHA: {c.sha256.slice(0, 10)}</span>
                        <span>{formatNumber(c.estimatedTokens)} tok</span>
                      </div>
                    </div>
                  ))}
                </div>
              </>
            ) : (
              <div className="rounded border border-ink-800/60 bg-ink-950/40 p-6 text-center text-xs text-ink-400">
                Context citations with line numbers and SHA-256 hashes appear after task execution.
              </div>
            )}
          </div>
        )}

        {activeTab === "validator" && (
          <div className="space-y-3">
            {currentRun ? (
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <ShieldCheckIcon
                    size={16}
                    className={currentRun.validatorVerdict.isValid ? "text-ok" : "text-critical"}
                  />
                  <span className="text-xs font-semibold text-ink-100">
                    Deterministic Checks:{" "}
                    {currentRun.validatorVerdict.isValid
                      ? "All Rules Passed"
                      : "Violations Flagged"}
                  </span>
                </div>
                {currentRun.validatorVerdict.checks.map((check) => (
                  <div
                    key={check.checkName}
                    className="rounded border border-ink-800/80 bg-ink-950/70 p-2.5 flex items-start justify-between gap-3"
                  >
                    <div>
                      <div className="text-xs font-medium text-ink-200">
                        {check.checkName.replace(/_/g, " ")}
                      </div>
                      <div className="text-[11px] text-ink-400 mt-0.5">{check.message}</div>
                    </div>
                    <Chip tone={check.passed ? "ok" : "danger"}>
                      {check.passed ? "PASSED" : "FAILED"}
                    </Chip>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded border border-ink-800/60 bg-ink-950/40 p-6 text-center text-xs text-ink-400">
                Deterministic validator checks (secret scanning, boundary rules) execute
                automatically.
              </div>
            )}
          </div>
        )}

        {activeTab === "tester" && (
          <div className="space-y-4">
            {/* Header / Config */}
            <div className="space-y-3 rounded border border-ink-800/80 bg-ink-950/60 p-3">
              <div className="flex flex-wrap items-center justify-between gap-2">
                <div className="flex items-center gap-2">
                  <TerminalIcon size={14} className="text-primary-400" />
                  <span className="text-xs font-semibold text-ink-100">
                    Tester Execution Engine (Role: Tester)
                  </span>
                </div>
                <div className="flex items-center gap-1.5 text-[10px] text-ink-400">
                  <ShieldCheckIcon size={12} className="text-ok-400" />
                  <span>Canonical Root Isolation • Stripped API Secrets • Allowlist Protected</span>
                </div>
              </div>

              {/* Allowlist Chips */}
              <div className="space-y-1.5">
                <div className="text-[11px] text-ink-400">
                  Select allowlisted command or enter custom:
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {allowlist.map((cmd) => (
                    <button
                      key={cmd}
                      type="button"
                      onClick={() => {
                        setTesterCommand(cmd);
                        handleRunTester(cmd);
                      }}
                      className={`rounded border px-2 py-1 font-mono text-xs transition-colors ${
                        testerCommand === cmd
                          ? "border-primary-500 bg-primary-500/10 text-primary-300"
                          : "border-ink-750 bg-ink-850 text-ink-300 hover:border-ink-600 hover:text-ink-100"
                      }`}
                    >
                      {cmd}
                    </button>
                  ))}
                </div>
              </div>

              {/* Command Input & Run */}
              <div className="flex items-center gap-2">
                <div className="relative flex-1">
                  <span className="absolute left-3 top-2 font-mono text-xs text-ink-500">$</span>
                  <input
                    type="text"
                    value={testerCommand}
                    onChange={(e) => setTesterCommand(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        handleRunTester();
                      }
                    }}
                    placeholder="Enter command (e.g. cargo test, npm test)..."
                    className="w-full rounded-md border border-ink-800 bg-ink-900 py-1.5 pl-7 pr-3 font-mono text-xs text-ink-100 placeholder:text-ink-500 focus:border-ink-600 focus:outline-hidden"
                  />
                </div>
                <Button
                  variant="primary"
                  size="sm"
                  onClick={() => handleRunTester()}
                  disabled={testerLoading || !testerCommand.trim()}
                >
                  {testerLoading ? "Running..." : "Run Test"}
                </Button>
              </div>
            </div>

            {/* Verdict Display */}
            {testerArtifact ? (
              <div className="space-y-3">
                <div
                  className={`flex items-center justify-between rounded border p-3 ${
                    testerArtifact.passed
                      ? "border-ok/30 bg-ok/10 text-ok"
                      : "border-danger/30 bg-danger/10 text-danger"
                  }`}
                >
                  <div className="flex items-center gap-2.5">
                    {testerArtifact.passed ? (
                      <CheckCircleIcon size={16} className="text-ok" />
                    ) : (
                      <AlertTriangleIcon size={16} className="text-danger" />
                    )}
                    <div>
                      <div className="text-xs font-semibold">
                        {testerArtifact.passed ? "All Tests Passed" : "Test Execution Failed"} (Exit
                        Code: {testerArtifact.exitCode})
                      </div>
                      <div className="text-[11px] opacity-80">{testerArtifact.summary}</div>
                    </div>
                  </div>
                  <div className="flex items-center gap-3 font-mono text-[11px]">
                    <span>{testerArtifact.durationMs}ms</span>
                    <Chip tone={testerArtifact.passed ? "ok" : "danger"}>
                      {testerArtifact.passed ? "PASSED" : "FAILED"}
                    </Chip>
                  </div>
                </div>

                {/* Output Console */}
                <div className="overflow-hidden rounded border border-ink-800 bg-ink-950 font-mono text-xs">
                  <div className="flex items-center justify-between border-b border-ink-800 px-3 py-1.5 bg-ink-900/50">
                    <span className="text-[11px] text-ink-400">$ {testerArtifact.command}</span>
                    <span className="text-[10px] text-ink-500">Output capped at 64 KB</span>
                  </div>
                  <div className="max-h-96 overflow-y-auto p-3 space-y-2">
                    {testerArtifact.stdout && (
                      <pre className="whitespace-pre-wrap text-ink-200">
                        {testerArtifact.stdout}
                      </pre>
                    )}
                    {testerArtifact.stderr && (
                      <pre className="whitespace-pre-wrap text-danger-300">
                        {testerArtifact.stderr}
                      </pre>
                    )}
                    {!testerArtifact.stdout && !testerArtifact.stderr && (
                      <span className="italic text-ink-500">No output produced.</span>
                    )}
                  </div>
                </div>
              </div>
            ) : (
              <div className="flex flex-col items-center justify-center rounded border border-dashed border-ink-800 p-8 text-center">
                <TerminalIcon size={24} className="mb-2 text-ink-600" />
                <div className="text-xs font-medium text-ink-300">No Tests Executed Yet</div>
                <p className="mt-1 max-w-sm text-[11px] text-ink-500">
                  Select an allowlisted command above or dispatch a task to automatically invoke the
                  Tester role and view test execution outputs.
                </p>
              </div>
            )}
          </div>
        )}

        {activeTab === "retrieval" && (
          <div className="space-y-4">
            <div className="flex gap-2">
              <div className="relative flex-1">
                <SearchIcon
                  size={13}
                  className="absolute left-3 top-1/2 -translate-y-1/2 text-ink-500"
                />
                <input
                  type="text"
                  value={retrievalQuery}
                  onChange={(e) => setRetrievalQuery(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleSemanticSearch();
                  }}
                  placeholder="Search project context or query task needs (e.g. 'token estimation, sidecar lifecycle')..."
                  className="w-full rounded-md border border-ink-800 bg-ink-900 py-1.5 pl-8 pr-3 text-xs text-ink-100 placeholder:text-ink-500 focus:border-ink-600 focus:outline-hidden"
                />
              </div>
              <Button
                variant="secondary"
                size="sm"
                onClick={handleSemanticSearch}
                disabled={!retrievalQuery.trim() || retrievalLoading}
              >
                {retrievalLoading ? "Searching..." : "Retrieve"}
              </Button>
            </div>

            {retrievalResult && (
              <div className="space-y-3">
                <div className="text-xs font-semibold text-ink-300">
                  Hybrid Ranked Results ({retrievalResult.proposedFiles.length} file(s) found)
                </div>
                <div className="space-y-2">
                  {retrievalResult.proposedFiles.map((file: ProposedFile) => (
                    <div
                      key={file.path}
                      className="flex items-center justify-between rounded border border-ink-800/80 bg-ink-950/60 p-2.5"
                    >
                      <div className="space-y-1">
                        <div className="flex items-center gap-2">
                          <span className="font-mono text-xs font-medium text-ink-100">
                            {file.path}
                          </span>
                          <span className="font-mono text-[10px] text-ink-400">
                            Score: {file.score.toFixed(2)}
                          </span>
                        </div>
                        <div className="flex flex-wrap gap-1">
                          {file.reasons.map((r: string) => (
                            <span
                              key={r}
                              className="rounded bg-ink-850 px-1 py-0.5 font-mono text-[9px] text-ink-400 border border-ink-750"
                            >
                              {r.replace(/_/g, " ")}
                            </span>
                          ))}
                        </div>
                      </div>
                      <Button
                        variant="secondary"
                        size="xs"
                        onClick={() => addFileToContext(file.path)}
                        disabled={contextFiles.includes(file.path)}
                      >
                        {contextFiles.includes(file.path) ? "Attached" : "Attach"}
                      </Button>
                    </div>
                  ))}
                </div>

                {retrievalResult.episodicMemories.length > 0 && (
                  <div className="mt-4 space-y-2">
                    <div className="text-xs font-semibold text-ink-300">
                      Episodic Memory ({retrievalResult.episodicMemories.length})
                    </div>
                    {retrievalResult.episodicMemories.map((mem) => (
                      <div
                        key={mem.id}
                        className="rounded border border-ink-800 bg-ink-950 p-2.5 space-y-1"
                      >
                        <div className="text-xs font-medium text-ink-200">{mem.taskSummary}</div>
                        <p className="text-[11px] text-ink-400 font-mono">{mem.keyFindings}</p>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )}
          </div>
        )}
      </div>

      {/* Input & Dispatch Control */}
      <div className="border-t border-ink-800/80 bg-ink-950/80 p-3">
        {/* Attached Context Files Chips */}
        <div className="mb-2 flex flex-wrap items-center gap-1.5 text-[11px]">
          <span className="text-ink-500">Context Files:</span>
          {contextFiles.map((file) => (
            <span
              key={file}
              className="inline-flex items-center gap-1 rounded border border-ink-750 bg-ink-850 px-2 py-0.5 font-mono text-ink-200"
            >
              {file}
              <button
                type="button"
                onClick={() => removeFileFromContext(file)}
                className="text-ink-500 hover:text-ink-200"
              >
                <XIcon size={10} />
              </button>
            </span>
          ))}
          <button
            type="button"
            onClick={() => setActiveTab("retrieval")}
            className="rounded border border-dashed border-ink-700 px-1.5 py-0.5 text-ink-400 hover:border-ink-500 hover:text-ink-200"
          >
            + Add Files via Retrieval
          </button>
        </div>

        {/* Input Area */}
        <div className="flex items-end gap-2">
          <textarea
            value={promptInput}
            onChange={(e) => setPromptInput(e.target.value)}
            onKeyDown={(e) => {
              if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                e.preventDefault();
                handleRunTask();
              }
            }}
            placeholder="Instruct the agent fleet (e.g. 'Refactor FileTree.tsx to reduce re-renders' or ⌘+Enter)..."
            rows={2}
            className="w-full resize-none rounded-md border border-ink-800 bg-ink-900 px-3 py-2 text-xs text-ink-100 placeholder:text-ink-500 focus:border-ink-600 focus:outline-hidden"
          />
          <Button
            variant="primary"
            size="md"
            onClick={handleRunTask}
            disabled={!promptInput.trim() || isLoading}
            title="Dispatch Task (⌘+Enter)"
            className="shrink-0"
          >
            {isLoading ? (
              <span>Executing...</span>
            ) : (
              <span className="flex items-center gap-1.5">
                <TerminalIcon size={13} />
                <span>Dispatch</span>
              </span>
            )}
          </Button>
        </div>
      </div>

      <GitSyncModal open={showGitSync} onClose={() => setShowGitSync(false)} />
    </div>
  );
}
