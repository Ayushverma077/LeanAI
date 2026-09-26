import { useState } from "react";
import { Chip } from "../primitives";

interface GraphNode {
  id: string;
  label: string;
  role: string;
  status: "idle" | "ready" | "active" | "completed";
  x: number;
  y: number;
  description: string;
}

export function AgentGraph() {
  const [selectedNode, setSelectedNode] = useState<string | null>("orchestrator");

  const nodes: GraphNode[] = [
    {
      id: "prompt",
      label: "Task Input",
      role: "Developer Goal / PR",
      status: "completed",
      x: 60,
      y: 140,
      description: "User goal, problem statement, or target feature specification.",
    },
    {
      id: "orchestrator",
      label: "Orchestrator",
      role: "Supervisor & Task Router",
      status: "active",
      x: 230,
      y: 140,
      description:
        "Deconstructs requests, assigns subtasks to specialist agents, and coordinates dependencies.",
    },
    {
      id: "researcher",
      label: "Researcher",
      role: "Context & Index Specialist",
      status: "ready",
      x: 420,
      y: 60,
      description:
        "Queries the LeanAi context index, discovers relevant files, and eliminates redundant tokens.",
    },
    {
      id: "planner",
      label: "Planner",
      role: "Architecture Strategist",
      status: "ready",
      x: 420,
      y: 220,
      description:
        "Formulates step-by-step implementation plans, checks ADR constraints, and designs contracts.",
    },
    {
      id: "coder",
      label: "Coder",
      role: "Implementation Specialist",
      status: "idle",
      x: 610,
      y: 140,
      description:
        "Produces clean, idiomatic code modifications, minimal diffs, and precise syntax implementations.",
    },
    {
      id: "validator",
      label: "Validator",
      role: "Test & Linter Runner",
      status: "idle",
      x: 780,
      y: 140,
      description:
        "Executes test suites, typecheckers, security scanners, and prevents broken builds.",
    },
    {
      id: "documenter",
      label: "Documenter",
      role: "Context Spec Maintainer",
      status: "idle",
      x: 940,
      y: 140,
      description:
        "Synchronizes PROJECT_CONTEXT.md, updates API documentation, and drafts release summaries.",
    },
  ];

  const activeNode = nodes.find((n) => n.id === selectedNode) ?? nodes[1];

  return (
    <div className="rounded-lg border border-ink-800/80 bg-ink-900/60 p-3.5 shadow-2xs">
      <div className="flex items-center justify-between border-b border-ink-800/80 pb-2.5">
        <div>
          <h3 className="text-xs font-semibold text-ink-100">Multi-Agent Workflow Graph</h3>
          <p className="text-[11px] text-ink-400">
            Pipeline routing for task orchestration, context pruning, and automated verification.
          </p>
        </div>
        <Chip tone="ok" dot>
          6 Agents Linked
        </Chip>
      </div>

      {/* SVG Canvas */}
      <div className="relative mt-3 overflow-x-auto rounded border border-ink-800 bg-ink-950/80 p-2">
        <svg
          viewBox="0 0 1060 280"
          className="h-56 w-full min-w-[760px] select-none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Connection Edges */}
          {/* Prompt -> Orchestrator */}
          <path d="M 120 140 L 180 140" stroke="#52525b" strokeWidth="1.5" strokeDasharray="3 3" />

          {/* Orchestrator -> Researcher */}
          <path
            d="M 280 140 C 330 140, 340 60, 370 60"
            stroke="#52525b"
            strokeWidth="1.5"
            fill="none"
          />

          {/* Orchestrator -> Planner */}
          <path
            d="M 280 140 C 330 140, 340 220, 370 220"
            stroke="#52525b"
            strokeWidth="1.5"
            fill="none"
          />

          {/* Researcher -> Coder */}
          <path
            d="M 470 60 C 520 60, 530 140, 560 140"
            stroke="#3f3f46"
            strokeWidth="1.5"
            fill="none"
          />

          {/* Planner -> Coder */}
          <path
            d="M 470 220 C 520 220, 530 140, 560 140"
            stroke="#3f3f46"
            strokeWidth="1.5"
            fill="none"
          />

          {/* Coder -> Validator */}
          <path d="M 660 140 L 730 140" stroke="#3f3f46" strokeWidth="1.5" />

          {/* Validator -> Documenter */}
          <path d="M 830 140 L 890 140" stroke="#3f3f46" strokeWidth="1.5" />

          {/* Nodes */}
          {nodes.map((node) => {
            const isSelected = selectedNode === node.id;

            return (
              <g
                key={node.id}
                transform={`translate(${node.x}, ${node.y})`}
                onClick={() => setSelectedNode(node.id)}
                className="cursor-pointer"
              >
                {/* Node Box */}
                <rect
                  x="-50"
                  y="-26"
                  width="100"
                  height="52"
                  rx="6"
                  className={`${
                    isSelected
                      ? "fill-ink-800 stroke-white stroke-1.5"
                      : "fill-ink-900 stroke-ink-750 stroke-1 hover:stroke-ink-600 hover:fill-ink-850 transition-colors"
                  }`}
                />

                {/* Node Label */}
                <text
                  textAnchor="middle"
                  y="-5"
                  className={`text-[11px] font-medium ${
                    isSelected ? "fill-white font-semibold" : "fill-ink-200"
                  }`}
                >
                  {node.label}
                </text>

                {/* Role / Subtext */}
                <text textAnchor="middle" y="11" className="fill-ink-400 text-[8.5px] font-mono">
                  {node.role.slice(0, 16)}
                </text>

                {/* Status Dot */}
                <circle
                  cx="38"
                  cy="-15"
                  r="2.5"
                  className={
                    node.status === "active"
                      ? "fill-ok"
                      : node.status === "completed"
                        ? "fill-ink-400"
                        : node.status === "ready"
                          ? "fill-ink-300"
                          : "fill-ink-600"
                  }
                />
              </g>
            );
          })}
        </svg>
      </div>

      {/* Selected Node Details Footer */}
      {activeNode ? (
        <div className="mt-2.5 flex items-center justify-between rounded border border-ink-800 bg-ink-950/70 px-3 py-2">
          <div>
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold text-ink-100">{activeNode.label}</span>
              <span className="text-[11px] text-ink-400">({activeNode.role})</span>
              <Chip
                tone={
                  activeNode.status === "active"
                    ? "ok"
                    : activeNode.status === "ready"
                      ? "info"
                      : "neutral"
                }
                dot
              >
                {activeNode.status}
              </Chip>
            </div>
            <p className="mt-0.5 text-[11px] text-ink-400">{activeNode.description}</p>
          </div>
        </div>
      ) : null}
    </div>
  );
}
