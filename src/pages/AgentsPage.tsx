import { useState } from "react";
import { useAppStore } from "../store/useAppStore";
import { AgentCard, type AgentDefinition } from "../components/agents/AgentCard";
import { AgentGraph } from "../components/agents/AgentGraph";
import { Button, Chip } from "../components/primitives";
import { AgentsIcon, ZapIcon } from "../components/icons";

export function AgentsPage() {
  const { setRoute } = useAppStore();
  const [selectedAgent, setSelectedAgent] = useState<AgentDefinition | null>(null);

  const agents: AgentDefinition[] = [
    {
      id: "orch",
      name: "Orchestrator",
      role: "Supervisor & Task Coordinator",
      model: "Claude 3.5 Sonnet",
      description:
        "Deconstructs requests, determines execution order, assigns subtasks to specialist agents, and coordinates dependencies.",
      status: "active",
      capabilities: ["Task Routing", "Dependency Graph", "Step Validation", "Subagent Spawning"],
      temperature: 0.2,
    },
    {
      id: "researcher",
      name: "Researcher",
      role: "Context & Index Specialist",
      model: "Local Qwen 2.5 7B",
      description:
        "Queries the LeanAi context index, discovers relevant files, inspects symbol graphs, and eliminates redundant tokens.",
      status: "ready",
      capabilities: ["Context Bundling", "Symbol Search", "Secret Filter", "Git Tree Navigation"],
      temperature: 0.1,
    },
    {
      id: "planner",
      name: "Planner",
      role: "Architecture Strategist",
      model: "Claude 3.5 Sonnet",
      description:
        "Formulates step-by-step implementation plans, checks ADR constraints, and designs contracts before code is touched.",
      status: "ready",
      capabilities: [
        "ADR Compliance",
        "API Contract Design",
        "Risk Assessment",
        "Migration Strategy",
      ],
      temperature: 0.2,
    },
    {
      id: "coder",
      name: "Coder",
      role: "Implementation Specialist",
      model: "Claude 3.5 Sonnet",
      description:
        "Generates clean, idiomatic code modifications, minimal diffs, and preserves all existing docstrings and structure.",
      status: "ready",
      capabilities: ["TypeScript / React", "Rust / Tauri", "Diff Synthesis", "Refactoring"],
      temperature: 0.2,
    },
    {
      id: "validator",
      name: "Validator",
      role: "Safety & Test Inspector",
      model: "Local llama-server",
      description:
        "Executes test suites, typecheckers, security scanners, and prevents broken builds from entering the tree.",
      status: "ready",
      capabilities: ["Cargo Test Runner", "Vitest Runner", "Clippy Linter", "Safety Verification"],
      temperature: 0.0,
    },
    {
      id: "documenter",
      name: "Documenter",
      role: "Context Spec Maintainer",
      model: "Local Qwen 2.5 7B",
      description:
        "Synchronizes PROJECT_CONTEXT.md, updates API documentation, and drafts release summaries.",
      status: "idle",
      capabilities: [
        "Context Index Sync",
        "Release Notes",
        "Markdown Generation",
        "Changelog Audit",
      ],
      temperature: 0.3,
    },
  ];

  return (
    <div className="mx-auto max-w-6xl space-y-6 pb-6">
      {/* Top Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between border-b border-ink-800/80 pb-3">
        <div>
          <div className="flex items-center gap-2">
            <AgentsIcon size={18} className="text-brand" />
            <div>
              <h1 className="text-base font-bold text-white tracking-tight">
                Agent Fleet
              </h1>
            </div>
          </div>
        </div>

        <div className="flex items-center gap-2 mt-3 sm:mt-0">
          <Chip tone="ok" dot>
            6 Online
          </Chip>
          <Button variant="primary" onClick={() => setRoute("tasks")}>
            <ZapIcon size={12} />
            <span>Launch Task →</span>
          </Button>
        </div>
      </div>

      {/* Interactive Workflow Graph */}
      <AgentGraph />

      {/* Agent Fleet Roster Grid */}
      <div className="space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-semibold text-ink-100">
            Agents
          </h2>
        </div>

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {agents.map((agent) => (
            <AgentCard key={agent.id} agent={agent} onSelect={setSelectedAgent} />
          ))}
        </div>
      </div>

      {/* Agent Detail Modal */}
      {selectedAgent ? (
        <div
          role="dialog"
          aria-modal="true"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
          onClick={() => setSelectedAgent(null)}
        >
          <div
            className="w-full max-w-lg rounded-xl border border-ink-800/60 bg-ink-950 p-6 shadow-2xl space-y-4"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-start justify-between">
              <div className="flex items-center gap-3">
                <div className="flex size-10 items-center justify-center rounded-lg border border-brand/20 bg-brand/10 text-brand">
                  <AgentsIcon size={18} />
                </div>
                <div>
                  <h3 className="text-base font-semibold text-white">{selectedAgent.name}</h3>
                  <p className="text-xs text-ink-400">{selectedAgent.role}</p>
                </div>
              </div>
            </div>

            <p className="text-sm leading-relaxed text-ink-300">{selectedAgent.description}</p>

            <div className="rounded-lg border border-ink-800/60 bg-ink-900/40 p-3 space-y-2 text-xs">
              <div className="flex items-center justify-between text-ink-400">
                <span>Model:</span>
                <span className="font-medium text-ink-100">{selectedAgent.model}</span>
              </div>
              <div className="flex items-center justify-between text-ink-400">
                <span>Temperature:</span>
                <span className="mono">{selectedAgent.temperature}</span>
              </div>
              <div className="flex items-center justify-between text-ink-400">
                <span>Execution:</span>
                <span className="mono text-ok">Zero Egress Verified</span>
              </div>
            </div>

            <div>
              <span className="text-xs font-medium text-ink-400">Capabilities</span>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {selectedAgent.capabilities.map((cap) => (
                  <Chip key={cap} tone="neutral">
                    {cap}
                  </Chip>
                ))}
              </div>
            </div>

            <div className="flex justify-end gap-2 pt-4">
              <Button variant="ghost" onClick={() => setSelectedAgent(null)}>
                Dismiss
              </Button>
              <Button
                variant="primary"
                onClick={() => {
                  setSelectedAgent(null);
                  setRoute("tasks");
                }}
              >
                Dispatch {selectedAgent.name}
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
