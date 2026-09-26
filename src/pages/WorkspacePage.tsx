import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { api, toAppError } from "../ipc/client";
import type { ProjectRecord } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";
import {
  Button,
  Chip,
  EmptyState,
  Panel,
  formatBytes,
  formatNumber,
} from "../components/primitives";

export function WorkspacePage() {
  const {
    project,
    git,
    inventory,
    classCounts,
    scanning,
    scanProgress,
    openProject,
    scan,
    cancelScan,
    setError,
  } = useAppStore();
  const [recent, setRecent] = useState<ProjectRecord[]>([]);

  useEffect(() => {
    api
      .listProjects()
      .then(setRecent)
      .catch((error) => setError(toAppError(error)));
  }, [project, setError]);

  const choose = async () => {
    const selected = await open({ directory: true, multiple: false, title: "Choose a project" });
    if (typeof selected === "string") await openProject(selected);
  };

  if (!project) {
    return (
      <div className="space-y-4">
        <EmptyState
          title="Open a project to begin"
          body={
            <>
              LeanAI reads only the directory you choose. Nothing is sent anywhere: bundling, token
              estimation and the context index all run on this device, with no API key and no
              network access.
            </>
          }
          action={
            <Button variant="primary" onClick={choose}>
              Choose a project directory…
            </Button>
          }
        />
        {recent.length > 0 ? (
          <Panel title="Recent projects" description="Stored locally. Nothing was uploaded.">
            <ul className="divide-y divide-ink-800">
              {recent.map((record) => (
                <li key={record.id} className="flex items-center justify-between gap-3 py-2">
                  <div className="min-w-0">
                    <p className="truncate text-sm text-ink-100">{record.displayName}</p>
                    <p className="mono truncate text-[11px] text-ink-500">{record.canonicalPath}</p>
                  </div>
                  <Button onClick={() => openProject(record.canonicalPath)}>Open</Button>
                </li>
              ))}
            </ul>
          </Panel>
        ) : null}
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <Panel
        title={project.displayName}
        description={<span className="mono">{project.canonicalPath}</span>}
        actions={
          <>
            {scanning ? (
              <Button variant="danger" onClick={cancelScan}>
                Cancel scan
              </Button>
            ) : (
              <Button onClick={scan}>Rescan</Button>
            )}
            <Button onClick={choose}>Open another…</Button>
          </>
        }
      >
        <div className="flex flex-wrap gap-2">
          <Chip tone="info" title="Stable identifier that does not contain your file path">
            {project.fingerprint}
          </Chip>
          {git?.isRepository ? (
            <>
              <Chip>git · {git.headRef ?? "detached"}</Chip>
              <Chip tone={git.isDirty ? "warn" : "ok"}>
                {git.isDirty ? "uncommitted changes" : "clean tree"}
              </Chip>
            </>
          ) : (
            <Chip title="Revision is derived from file contents instead of a commit">
              not a git repository
            </Chip>
          )}
          {inventory ? <Chip>revision {inventory.sourceRevision.slice(0, 22)}</Chip> : null}
        </div>

        {scanning ? (
          <p className="mt-4 text-xs text-ink-300" role="status" aria-live="polite">
            Scanning… {formatNumber(scanProgress?.filesSeen ?? 0)} files,{" "}
            {formatNumber(scanProgress?.directoriesSeen ?? 0)} directories seen.
          </p>
        ) : null}
      </Panel>

      {inventory ? (
        <>
          <Panel
            title="What the scan found"
            description="Files are excluded by policy before you can select them. Every exclusion has a reason."
          >
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <Stat label="Files scanned" value={formatNumber(inventory.stats.filesSeen)} />
              <Stat label="Directories" value={formatNumber(inventory.stats.directoriesSeen)} />
              <Stat label="Bytes seen" value={formatBytes(inventory.stats.bytesSeen)} />
              <Stat label="Scan time" value={`${inventory.stats.elapsedMs} ms`} />
            </div>
            <table className="mt-4 w-full text-xs">
              <caption className="sr-only">File classification counts</caption>
              <thead>
                <tr className="text-left text-ink-500">
                  <th scope="col" className="py-1 font-medium">
                    Classification
                  </th>
                  <th scope="col" className="py-1 text-right font-medium">
                    Files
                  </th>
                </tr>
              </thead>
              <tbody>
                {Object.entries(classCounts).map(([label, count]) => (
                  <tr key={label} className="border-t border-ink-800">
                    <td className="py-1.5">{label}</td>
                    <td className="py-1.5 text-right">{formatNumber(count)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {inventory.stats.truncated ? (
              <p className="mt-3 rounded border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
                The scan stopped at the file limit, so this project is only partly represented.
                Raise the limit in Settings or narrow the project root.
              </p>
            ) : null}
          </Panel>

          {inventory.issues.length > 0 ? (
            <Panel
              title={`Paths that could not be read (${inventory.issues.length})`}
              description="Scanning continued past each one. Nothing was skipped silently."
            >
              <ul className="max-h-56 space-y-1 overflow-auto text-xs">
                {inventory.issues.map((issue) => (
                  <li key={issue.path} className="border-b border-ink-800 pb-1">
                    <span className="mono text-ink-300">{issue.path}</span>
                    <span className="block text-ink-500">{issue.message}</span>
                  </li>
                ))}
              </ul>
            </Panel>
          ) : null}
        </>
      ) : null}
    </div>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded border border-ink-800 px-3 py-2">
      <p className="text-[11px] text-ink-500">{label}</p>
      <p className="mt-0.5 text-sm text-ink-100">{value}</p>
    </div>
  );
}
