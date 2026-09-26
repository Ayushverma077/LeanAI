import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api, toAppError } from "../ipc/client";
import type { BundleRecord, ProjectRecord } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";
import {
  Button,
  Chip,
  EmptyState,
  StatCard,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import {
  ContextIcon,
  FolderIcon,
  GitBranchIcon,
  GithubIcon,
  LogoIcon,
  RefreshCwIcon,
  ShieldCheckIcon,
  SparklesIcon,
  ZapIcon,
} from "../components/icons";
import { CloneRepoModal } from "../components/git/CloneRepoModal";
import { ProjectSwitcher } from "../components/ProjectSwitcher";

export function OverviewPage() {
  const {
    project,
    git,
    inventory,
    classCounts,
    scanning,
    scanProgress,
    openProject,
    scan,
    bundle,
    setRoute,
    setError,
  } = useAppStore();

  const [recent, setRecent] = useState<ProjectRecord[]>([]);
  const [history, setHistory] = useState<BundleRecord[]>([]);
  const [showCloneModal, setShowCloneModal] = useState(false);

  useEffect(() => {
    api
      .listProjects()
      .then(setRecent)
      .catch((error: unknown) => setError(toAppError(error)));

    if (project) {
      api
        .bundleHistory()
        .then(setHistory)
        .catch(() => undefined);
    }
  }, [project, setError]);

  const chooseProject = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose a project directory",
    });
    if (typeof selected === "string") {
      await openProject(selected);
    }
  };

  /* ── No project: welcome screen ── */
  if (!project) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-8 py-8">
        {/* Hero */}
        <div className="flex flex-col items-center gap-4 text-center max-w-sm">
          <LogoIcon size={52} className="logo-tile" />
          <div>
            <h1 className="text-lg font-bold text-ink-100 tracking-tight">LeanAI Desktop</h1>
            <p className="mt-1 text-xs text-ink-400 leading-relaxed">
              Open a repository to start bundling context for your AI agents.
            </p>
          </div>
          <div className="flex items-center gap-2 mt-1">
            <Button variant="primary" size="md" onClick={chooseProject}>
              <FolderIcon size={13} />
              Open Project
            </Button>
            <Button variant="default" size="md" onClick={() => setShowCloneModal(true)}>
              <GithubIcon size={13} />
              Clone Repo
            </Button>
          </div>
        </div>

        {/* Recent projects */}
        {recent.length > 0 && (
          <div className="w-full max-w-md">
            <p className="mb-2 text-[11px] font-semibold uppercase tracking-wider text-ink-600 px-1">
              Recent
            </p>
            <div className="flex flex-col gap-1">
              {recent.slice(0, 5).map((rec) => (
                <button
                  key={rec.id}
                  type="button"
                  onClick={() => openProject(rec.canonicalPath)}
                  className="flex items-center justify-between rounded-lg border border-ink-800/60 bg-ink-900/40 px-3 py-2 text-left hover:border-brand/30 hover:bg-ink-800/40 transition-all group"
                >
                  <div className="min-w-0">
                    <p className="text-xs font-medium text-ink-200 group-hover:text-ink-100 transition-colors truncate">
                      {rec.displayName}
                    </p>
                    <p className="mono text-[10px] text-ink-600 truncate mt-0.5">
                      {rec.canonicalPath}
                    </p>
                  </div>
                  <span className="ml-3 text-[10px] text-ink-600 group-hover:text-brand transition-colors shrink-0">
                    →
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}

        <CloneRepoModal open={showCloneModal} onClose={() => setShowCloneModal(false)} />
      </div>
    );
  }

  /* ── With project ── */
  const filesSeen = inventory?.stats.filesSeen ?? 0;
  const bytesSeen = inventory?.stats.bytesSeen ?? 0;
  const elapsedMs = inventory?.stats.elapsedMs ?? 0;
  const bundledTokens = bundle?.estimate.value ?? 0;

  return (
    <div className="mx-auto max-w-4xl space-y-4 pb-6">
      {/* Project header */}
      <div className="rounded-xl border border-ink-800/60 bg-ink-900/40 p-4">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <ProjectSwitcher onCloneRequest={() => setShowCloneModal(true)} />
              {git?.isRepository && (
                <Chip tone={git.isDirty ? "warn" : "ok"} dot>
                  <GitBranchIcon size={10} className="shrink-0" />
                  <span>{git.headRef ?? "detached"}</span>
                </Chip>
              )}
            </div>
            <p className="mono mt-0.5 text-[10px] text-ink-600 truncate">{project.canonicalPath}</p>
          </div>
          <div className="flex items-center gap-1.5 shrink-0">
            <Button variant="default" size="xs" disabled={scanning} onClick={scan}>
              <RefreshCwIcon size={11} className={scanning ? "animate-spin" : ""} />
              {scanning ? "Scanning…" : "Rescan"}
            </Button>
            <Button variant="primary" size="xs" onClick={() => setRoute("context")}>
              <ContextIcon size={11} />
              Context Studio
            </Button>
          </div>
        </div>

        {/* Scan progress */}
        {scanning && (
          <div className="mt-3 flex items-center gap-2 rounded-md border border-ink-750 bg-ink-950/60 px-3 py-2 text-xs text-ink-400">
            <RefreshCwIcon size={11} className="animate-spin text-brand shrink-0" />
            <span>
              {formatNumber(scanProgress?.filesSeen ?? 0)} files,{" "}
              {formatNumber(scanProgress?.directoriesSeen ?? 0)} dirs scanned…
            </span>
          </div>
        )}

        {/* Stats */}
        <div className="mt-3 grid grid-cols-2 gap-2 sm:grid-cols-4">
          <StatCard
            label="Files"
            value={formatNumber(filesSeen)}
            icon={<FolderIcon size={14} />}
          />
          <StatCard
            label="Size"
            value={formatBytes(bytesSeen)}
            subtext={`${elapsedMs}ms`}
            icon={<SparklesIcon size={14} />}
          />
          <StatCard
            label="Bundle Tokens"
            value={bundledTokens > 0 ? `~${formatNumber(bundledTokens)}` : "—"}
            subtext={bundledTokens > 0 ? "Ready" : "No selection"}
            icon={<ZapIcon size={14} />}
          />
          <StatCard
            label="Privacy"
            value="Offline"
            subtext="Zero egress"
            icon={<ShieldCheckIcon size={14} />}
          />
        </div>
      </div>

      {/* Two-col: classification + recent bundles */}
      <div className="grid gap-4 lg:grid-cols-2">
        {/* File classification */}
        {Object.keys(classCounts).length > 0 && (
          <div className="rounded-xl border border-ink-800/60 bg-ink-900/40 p-4">
            <h2 className="mb-3 text-xs font-semibold text-ink-300">File Classification</h2>
            <table className="w-full text-xs">
              <tbody className="divide-y divide-ink-800/40">
                {Object.entries(classCounts).map(([label, count]) => (
                  <tr key={label}>
                    <td className="py-1.5 mono text-[11px] text-ink-400">{label}</td>
                    <td className="py-1.5 text-right font-semibold text-ink-200">
                      {formatNumber(Number(count))}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            <Button
              variant="ghost"
              size="xs"
              className="mt-3 w-full justify-center text-ink-500"
              onClick={() => setRoute("context")}
            >
              Configure exclusions →
            </Button>
          </div>
        )}

        {/* Recent bundles */}
        {history.length > 0 && (
          <div className="rounded-xl border border-ink-800/60 bg-ink-900/40 p-4">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xs font-semibold text-ink-300">Recent Bundles</h2>
              <button
                type="button"
                onClick={() => setRoute("history")}
                className="text-[11px] text-ink-500 hover:text-brand transition-colors"
              >
                All →
              </button>
            </div>
            <div className="flex flex-col divide-y divide-ink-800/40">
              {history.slice(0, 4).map((rec) => (
                <div key={rec.id} className="flex items-center justify-between py-2 text-xs">
                  <div>
                    <span className="mono text-ink-300">{rec.outputHash.slice(0, 12)}…</span>
                    <p className="mt-0.5 text-[10px] text-ink-600">
                      {rec.fileCount} files · {formatBytes(rec.byteLen)}
                    </p>
                  </div>
                  <Button size="xs" variant="ghost" onClick={() => setRoute("context")}>
                    View
                  </Button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Quick actions if no classification yet */}
        {Object.keys(classCounts).length === 0 && (
          <EmptyState
            icon={<ContextIcon size={22} className="text-brand" />}
            title="No files indexed yet"
            body="Run a scan to index your repository."
            action={
              <Button variant="primary" size="sm" onClick={scan} disabled={scanning}>
                <RefreshCwIcon size={12} className={scanning ? "animate-spin" : ""} />
                {scanning ? "Scanning…" : "Scan Now"}
              </Button>
            }
          />
        )}
      </div>

      {/* Mounted here as well as on the welcome screen, so the switcher's
          "Clone from GitHub…" works while a repository is already open. */}
      <CloneRepoModal open={showCloneModal} onClose={() => setShowCloneModal(false)} />
    </div>
  );
}
