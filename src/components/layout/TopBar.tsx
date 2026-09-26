import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { useAppStore } from "../../store/useAppStore";
import { Button, Chip, formatNumber } from "../primitives";
import {
  DownloadCloudIcon,
  GitBranchIcon,
  LogoIcon,
  MoonIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  SunIcon,
  UploadCloudIcon,
} from "../icons";
import { GitSyncModal } from "../git/GitSyncModal";
import { CloneRepoModal } from "../git/CloneRepoModal";

export function TopBar({ onOpenCommandPalette }: { onOpenCommandPalette: () => void }) {
  const { project, git, remoteStatus, scanning, scan, openProject, bundle, setRoute, theme, toggleTheme } =
    useAppStore();
  const [showSyncModal, setShowSyncModal] = useState(false);
  const [showCloneModal, setShowCloneModal] = useState(false);

  const handleOpenFolder = async () => {
    const selected = await open({ directory: true, multiple: false, title: "Choose a project" });
    if (typeof selected === "string") {
      await openProject(selected);
    }
  };

  const tokenEstimate = bundle?.estimate.value ?? 0;
  const tokenMax = 128000;
  const tokenPct = Math.min((tokenEstimate / tokenMax) * 100, 100);

  return (
    <header className="flex h-10 shrink-0 items-center justify-between border-b border-ink-800/60 bg-ink-950 px-3 select-none">
      {/* Brand */}
      <div className="flex items-center gap-2.5">
        <div
          className="flex cursor-pointer items-center gap-2 transition-opacity hover:opacity-75"
          onClick={() => setRoute("overview")}
          title="Home"
        >
          <LogoIcon size={22} className="rounded-md" />
          <span className="text-xs font-semibold text-ink-100 tracking-tight">LeanAI</span>
        </div>

        <div className="h-3.5 w-px bg-ink-800" />

        {/* Project Breadcrumb */}
        {project ? (
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              onClick={handleOpenFolder}
              title={project.canonicalPath}
              className="max-w-[160px] truncate rounded px-1.5 py-0.5 text-xs text-ink-300 hover:bg-ink-800 hover:text-white transition-colors"
            >
              {project.displayName}
            </button>

            {git?.isRepository ? (
              <div className="flex items-center gap-1">
                <Chip
                  tone={git.isDirty ? "warn" : "ok"}
                  dot
                  title={git.isDirty ? "Uncommitted changes" : "Clean"}
                >
                  <GitBranchIcon size={10} className="shrink-0" />
                  <span className="max-w-[90px] truncate">{git.headRef ?? "detached"}</span>
                </Chip>

                {remoteStatus && remoteStatus.remotes.length > 0 && (
                  <button
                    type="button"
                    onClick={() => setShowSyncModal(true)}
                    className="flex items-center gap-1 rounded border border-ink-800 bg-ink-900/60 px-1.5 py-0.5 text-[10px] text-ink-400 hover:border-ink-700 hover:text-ink-200 transition-colors cursor-pointer"
                    title={`Remote: ${remoteStatus.remotes[0]?.name ?? "origin"}`}
                  >
                    <UploadCloudIcon
                      size={10}
                      className={remoteStatus.ahead > 0 ? "text-brand" : "text-ink-500"}
                    />
                    {remoteStatus.ahead > 0 ? (
                      <span className="text-brand font-medium">↑{remoteStatus.ahead}</span>
                    ) : remoteStatus.behind > 0 ? (
                      <span className="text-warn font-medium">↓{remoteStatus.behind}</span>
                    ) : (
                      <span className="text-ink-500 mono text-[9px]">synced</span>
                    )}
                  </button>
                )}
              </div>
            ) : null}
          </div>
        ) : (
          <div className="flex items-center gap-1">
            <Button variant="ghost" size="xs" onClick={handleOpenFolder}>
              Open…
            </Button>
            <Button
              variant="ghost"
              size="xs"
              onClick={() => setShowCloneModal(true)}
              title="Clone from GitHub"
              className="flex items-center gap-1 text-ink-500"
            >
              <DownloadCloudIcon size={11} />
              <span>Clone</span>
            </Button>
          </div>
        )}
      </div>

      {/* Center: token meter */}
      <div className="hidden md:flex items-center gap-2">
        {tokenEstimate > 0 && (
          <div
            className="flex cursor-pointer items-center gap-2 rounded px-2 py-0.5 text-[11px] hover:bg-ink-800/60 transition-colors"
            onClick={() => setRoute("context")}
            title="Context token usage"
          >
            <span className="text-ink-500">Context</span>
            <span className="mono text-ink-200 font-medium">~{formatNumber(tokenEstimate)}</span>
            <div className="h-1 w-16 overflow-hidden rounded-full bg-ink-800">
              <div
                className={`h-full rounded-full transition-all duration-300 ${
                  tokenPct < 50
                    ? "bg-brand"
                    : tokenPct < 80
                      ? "bg-warn"
                      : "bg-danger"
                }`}
                style={{ width: `${Math.max(tokenPct, tokenEstimate > 0 ? 3 : 0)}%` }}
              />
            </div>
            <span className="text-ink-600 mono text-[9px]">128k</span>
          </div>
        )}
      </div>

      {/* Right: Actions */}
      <div className="flex items-center gap-1">
        {project && (
          <Button
            variant="ghost"
            size="xs"
            disabled={scanning}
            onClick={scan}
            title="Rescan (⌘R)"
          >
            <RefreshCwIcon size={12} className={scanning ? "animate-spin text-brand" : ""} />
          </Button>
        )}

        <button
          type="button"
          onClick={onOpenCommandPalette}
          className="flex items-center gap-1 rounded border border-ink-800 bg-ink-900/40 px-2 py-1 text-xs text-ink-400 hover:border-ink-700 hover:text-ink-200 transition-colors"
          title="Command palette (⌘K)"
        >
          <SearchIcon size={12} />
          <kbd className="mono rounded border border-ink-750 bg-ink-850 px-1 text-[9px] text-ink-500">
            ⌘K
          </kbd>
        </button>

        <button
          type="button"
          onClick={toggleTheme}
          className="flex size-7 items-center justify-center rounded text-ink-500 hover:bg-ink-800 hover:text-ink-200 transition-colors"
          title={`Switch to ${theme === "dark" ? "light" : "dark"} theme`}
          aria-label={`Switch to ${theme === "dark" ? "light" : "dark"} theme`}
        >
          {/* The icon shows the theme you would switch *to*. */}
          {theme === "dark" ? <SunIcon size={13} /> : <MoonIcon size={13} />}
        </button>

        <button
          type="button"
          onClick={() => setRoute("settings")}
          className="flex size-7 items-center justify-center rounded text-ink-500 hover:bg-ink-800 hover:text-ink-200 transition-colors"
          title="Settings"
        >
          <SettingsIcon size={13} />
        </button>
      </div>

      <GitSyncModal open={showSyncModal} onClose={() => setShowSyncModal(false)} />
      <CloneRepoModal open={showCloneModal} onClose={() => setShowCloneModal(false)} />
    </header>
  );
}
