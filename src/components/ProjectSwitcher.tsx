import { useCallback, useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api, toAppError } from "../ipc/client";
import type { ProjectRecord } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";
import {
  CheckIcon,
  ChevronDownIcon,
  FolderIcon,
  GithubIcon,
  XIcon,
} from "./icons";

/**
 * Switches the open repository.
 *
 * Previously the only way to change repository was the unlabelled project name
 * in the top bar, which opened a folder dialog — so "switch to a repo I opened
 * last week" meant navigating the filesystem again. This lists recent projects,
 * and keeps the folder picker and clone flow alongside them.
 */
export function ProjectSwitcher({ onCloneRequest }: { onCloneRequest: () => void }) {
  const { project, openProject, closeProject, setRoute, setError } = useAppStore();

  const [expanded, setExpanded] = useState(false);
  const [recent, setRecent] = useState<ProjectRecord[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!expanded) return;
    api
      .listProjects()
      .then(setRecent)
      .catch((error: unknown) => setError(toAppError(error)));
  }, [expanded, setError]);

  const close = useCallback(() => {
    setExpanded(false);
    triggerRef.current?.focus();
  }, []);

  // Close on outside click or Escape, the two things every menu is expected to do.
  useEffect(() => {
    if (!expanded) return;

    const onPointerDown = (event: PointerEvent) => {
      if (!containerRef.current?.contains(event.target as Node)) setExpanded(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        close();
      }
    };

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [expanded, close]);

  /**
   * Switching repository from Overview keeps you on Overview. `scan()` moves
   * the route to the Context Studio, which is right after *opening* a project
   * but disorienting when you were reading the dashboard.
   */
  const switchTo = async (path: string) => {
    setExpanded(false);
    await openProject(path);
    setRoute("overview");
  };

  const chooseFolder = async () => {
    setExpanded(false);
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Choose a project directory",
    });
    if (typeof selected === "string") await switchTo(selected);
  };

  if (!project) return null;

  const others = recent.filter((candidate) => candidate.id !== project.id);

  return (
    <div ref={containerRef} className="relative shrink-0">
      <button
        ref={triggerRef}
        type="button"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
        aria-haspopup="menu"
        title={project.canonicalPath}
        className="group flex items-center gap-1.5 rounded-md px-1.5 py-1 -ml-1.5 text-left transition-colors hover:bg-ink-800/60"
      >
        {/* `max-w-*` bounds a long repository name; the trigger itself must not
            carry `max-w-full`, which resolves against this box circularly and
            clips even a short name by a few pixels. */}
        <h1 className="max-w-[15rem] truncate text-sm font-bold tracking-tight text-ink-100">
          {project.displayName}
        </h1>
        <ChevronDownIcon
          size={13}
          className={`shrink-0 text-ink-500 transition-transform group-hover:text-ink-300 ${
            expanded ? "rotate-180" : ""
          }`}
        />
      </button>

      {expanded && (
        <div
          role="menu"
          aria-label="Switch repository"
          className="absolute left-0 top-full z-40 mt-1 w-80 overflow-hidden rounded-xl border border-ink-750 bg-ink-900 shadow-2xl"
        >
          <div className="max-h-64 overflow-y-auto p-1">
            <p className="px-2 pb-1 pt-1.5 text-[10px] font-semibold uppercase tracking-wider text-ink-600">
              Repositories
            </p>

            <MenuRow
              icon={<CheckIcon size={13} className="text-brand" />}
              title={project.displayName}
              subtitle={project.canonicalPath}
              current
              onClick={close}
            />

            {others.map((candidate) => (
              <MenuRow
                key={candidate.id}
                icon={<FolderIcon size={13} className="text-ink-500" />}
                title={candidate.displayName}
                subtitle={candidate.canonicalPath}
                onClick={() => void switchTo(candidate.canonicalPath)}
              />
            ))}

            {others.length === 0 && (
              <p className="px-2 py-2 text-[11px] text-ink-500">
                No other repositories opened yet.
              </p>
            )}
          </div>

          <div className="border-t border-ink-800 p-1">
            <MenuRow
              icon={<FolderIcon size={13} className="text-ink-500" />}
              title="Open a folder…"
              onClick={() => void chooseFolder()}
            />
            <MenuRow
              icon={<GithubIcon size={13} className="text-ink-500" />}
              title="Clone from GitHub…"
              onClick={() => {
                setExpanded(false);
                onCloneRequest();
              }}
            />
            <MenuRow
              icon={<XIcon size={13} className="text-ink-500" />}
              title="Close repository"
              onClick={() => {
                setExpanded(false);
                void closeProject();
              }}
            />
          </div>
        </div>
      )}
    </div>
  );
}

function MenuRow({
  icon,
  title,
  subtitle,
  current = false,
  onClick,
}: {
  icon: React.ReactNode;
  title: string;
  subtitle?: string;
  current?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      role="menuitem"
      onClick={onClick}
      aria-current={current || undefined}
      className="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left transition-colors hover:bg-ink-800/70"
    >
      <span className="flex w-4 shrink-0 justify-center">{icon}</span>
      <span className="min-w-0 flex-1">
        <span
          className={`block truncate text-xs ${
            current ? "font-semibold text-ink-100" : "text-ink-200"
          }`}
        >
          {title}
        </span>
        {subtitle ? (
          <span className="mono block truncate text-[10px] text-ink-600">{subtitle}</span>
        ) : null}
      </span>
      {current ? (
        <span className="shrink-0 text-[10px] font-medium text-ink-500">current</span>
      ) : null}
    </button>
  );
}
