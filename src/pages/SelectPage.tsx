import { useCallback, useEffect, useMemo, useState } from "react";

import { FileTree } from "../components/FileTree";
import {
  Button,
  Chip,
  EmptyState,
  Panel,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import { api, toAppError } from "../ipc/client";
import type { ChangedFile, DiffScope, FileEntry, PresetRecord } from "../ipc/types";
import { toSpec, useAppStore } from "../store/useAppStore";

export function SelectPage() {
  const store = useAppStore();
  const { inventory, selection, git } = store;
  const [search, setSearch] = useState("");
  const [pendingOverride, setPendingOverride] = useState<FileEntry | null>(null);
  const [presets, setPresets] = useState<PresetRecord[]>([]);
  const [presetName, setPresetName] = useState("");
  const [diffScopes, setDiffScopes] = useState<DiffScope[]>(["unstaged", "untracked"]);
  const [baseRef, setBaseRef] = useState("");
  const [changed, setChanged] = useState<ChangedFile[] | null>(null);

  const refreshPresets = useCallback(() => {
    api
      .listPresets()
      .then(setPresets)
      .catch((error) => store.setError(toAppError(error)));
  }, [store]);

  useEffect(() => {
    if (inventory) refreshPresets();
    // Presets are per project, so reload whenever the scan revision changes.
  }, [inventory?.sourceRevision, refreshPresets, inventory]);

  const selectableCount = useMemo(
    () => inventory?.files.filter((file) => file.selectable).length ?? 0,
    [inventory],
  );
  const selectedBytes = useMemo(() => {
    if (!inventory) return 0;
    let total = 0;
    for (const file of inventory.files) if (selection.files.has(file.path)) total += file.sizeBytes;
    return total;
  }, [inventory, selection.files]);

  if (!inventory) {
    return (
      <EmptyState
        title="Nothing scanned yet"
        body="Open a project and run a scan to choose files."
        action={<Button onClick={() => store.setRoute("workspace")}>Go to the project</Button>}
      />
    );
  }

  const applyDiff = async () => {
    try {
      const files = await api.changedFiles(diffScopes, baseRef.trim() || null);
      setChanged(files);
      const selectable = new Set(
        inventory.files.filter((file) => file.selectable).map((file) => file.path),
      );
      const usable = files.map((file) => file.path).filter((path) => selectable.has(path));
      store.selectPaths(usable, true);
      store.setNotice(
        `Selected ${usable.length} of ${files.length} changed files. ${
          files.length - usable.length
        } are excluded by policy.`,
      );
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(320px,1fr)]">
      <Panel
        title="Files"
        description={`${formatNumber(selectableCount)} of ${formatNumber(
          inventory.files.length,
        )} files can be selected. The rest are excluded by policy with a stated reason.`}
        actions={
          <>
            <Button
              onClick={() =>
                store.selectPaths(
                  inventory.files.filter((file) => file.selectable).map((file) => file.path),
                  true,
                )
              }
            >
              Select all selectable
            </Button>
            <Button variant="ghost" onClick={store.clearSelection}>
              Clear
            </Button>
          </>
        }
      >
        <label className="mb-3 block">
          <span className="sr-only">Search files by path</span>
          <input
            type="search"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search by path…"
            className="w-full rounded border border-ink-800 bg-ink-950 px-3 py-1.5 text-xs text-ink-100 placeholder:text-ink-500"
          />
        </label>

        <FileTree
          inventory={inventory}
          selected={selection.files}
          overrides={selection.overrides}
          search={search}
          onToggleFile={store.toggleFile}
          onToggleDirectory={store.toggleDirectory}
          onRequestOverride={setPendingOverride}
        />

        <p className="mt-3 text-xs text-ink-500" role="status" aria-live="polite">
          {formatNumber(selection.files.size)} selected · {formatBytes(selectedBytes)}
        </p>
      </Panel>

      <div className="space-y-4">
        <Panel title="Selection summary">
          {selection.files.size === 0 ? (
            <p className="text-xs text-ink-500">Nothing selected yet.</p>
          ) : (
            <>
              <ul className="max-h-56 space-y-0.5 overflow-auto text-[11px]">
                {[...selection.files].sort().map((path) => (
                  <li key={path} className="flex items-center justify-between gap-2">
                    <span className="mono truncate text-ink-300">{path}</span>
                    <button
                      type="button"
                      onClick={() => store.toggleFile(path, false)}
                      className="shrink-0 text-ink-500 hover:text-danger"
                      aria-label={`Remove ${path} from the selection`}
                    >
                      remove
                    </button>
                  </li>
                ))}
              </ul>
              <Button variant="primary" onClick={() => store.setRoute("preview")}>
                Preview bundle →
              </Button>
            </>
          )}
        </Panel>

        <Panel
          title="Select from git changes"
          description={
            git?.isRepository
              ? "Choose exactly which change scopes to include."
              : "This project is not a git repository, so diff selection is unavailable."
          }
        >
          {git?.isRepository ? (
            <div className="space-y-2 text-xs">
              {(["staged", "unstaged", "untracked", "against_ref"] as DiffScope[]).map((scope) => (
                <label key={scope} className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={diffScopes.includes(scope)}
                    onChange={(event) =>
                      setDiffScopes((current) =>
                        event.target.checked
                          ? [...current, scope]
                          : current.filter((value) => value !== scope),
                      )
                    }
                    className="size-3.5 rounded accent-ink-100"
                  />
                  {scope.replace("_", " ")}
                </label>
              ))}
              {diffScopes.includes("against_ref") ? (
                <input
                  value={baseRef}
                  onChange={(event) => setBaseRef(event.target.value)}
                  placeholder="base ref, e.g. main or HEAD~3"
                  className="w-full rounded border border-ink-800 bg-ink-950 px-2 py-1 text-xs"
                />
              ) : null}
              <Button onClick={applyDiff}>Select changed files</Button>
              {changed ? (
                <p className="text-[11px] text-ink-500">
                  {changed.length} changed path{changed.length === 1 ? "" : "s"} found.
                </p>
              ) : null}
            </div>
          ) : (
            <p className="text-xs text-ink-500">
              Select files manually, or open a git checkout to use diff mode.
            </p>
          )}
        </Panel>

        <Panel
          title="Presets"
          description="A preset stores the rule, not a frozen file list, and is re-checked against the current scan."
        >
          <div className="flex gap-2">
            <input
              value={presetName}
              onChange={(event) => setPresetName(event.target.value)}
              placeholder="Preset name"
              aria-label="Preset name"
              className="min-w-0 flex-1 rounded border border-ink-800 bg-ink-950 px-2 py-1 text-xs"
            />
            <Button
              disabled={!presetName.trim() || selection.files.size === 0}
              onClick={async () => {
                try {
                  await api.savePreset(presetName.trim(), toSpec(selection), store.options);
                  setPresetName("");
                  refreshPresets();
                  store.setNotice("Preset saved.");
                } catch (error) {
                  store.setError(toAppError(error));
                }
              }}
            >
              Save
            </Button>
          </div>
          <ul className="mt-3 space-y-2 text-xs">
            {presets.map((preset) => {
              const missing = preset.validation?.missing.length ?? 0;
              const rejected = preset.validation?.rejected.length ?? 0;
              return (
                <li key={preset.id} className="rounded border border-ink-800 px-2 py-1.5">
                  <div className="flex items-center justify-between gap-2">
                    <span className="truncate text-ink-100">{preset.name}</span>
                    <div className="flex shrink-0 gap-1">
                      <Button
                        onClick={() => {
                          store.selectPaths(preset.validation?.files ?? [], true);
                          store.setOptions(preset.options);
                          store.setNotice(`Applied preset "${preset.name}".`);
                        }}
                      >
                        Apply
                      </Button>
                      <Button
                        variant="danger"
                        onClick={async () => {
                          await api.deletePreset(preset.id).catch(() => undefined);
                          refreshPresets();
                        }}
                      >
                        Delete
                      </Button>
                    </div>
                  </div>
                  <div className="mt-1 flex flex-wrap gap-1">
                    <Chip>{preset.validation?.files.length ?? 0} files resolve</Chip>
                    {missing > 0 ? <Chip tone="warn">{missing} no longer exist</Chip> : null}
                    {rejected > 0 ? <Chip tone="danger">{rejected} now blocked</Chip> : null}
                  </div>
                </li>
              );
            })}
            {presets.length === 0 ? <li className="text-ink-500">No presets yet.</li> : null}
          </ul>
        </Panel>
      </div>

      {pendingOverride ? (
        <OverrideDialog
          entry={pendingOverride}
          onCancel={() => setPendingOverride(null)}
          onConfirm={() => {
            store.addOverride(pendingOverride.path);
            setPendingOverride(null);
          }}
        />
      ) : null}
    </div>
  );
}

/**
 * The per-file unlock for a path the policy blocks. Deliberately high friction:
 * the reason is restated and the user must type the word to proceed (FR-08,
 * NFR 7.1 "informed override only with an explicit high-friction confirmation").
 */
function OverrideDialog({
  entry,
  onCancel,
  onConfirm,
}: {
  entry: FileEntry;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const [typed, setTyped] = useState("");
  const sensitive = entry.class === "credential_sensitive";
  const phrase = sensitive ? "include secret" : "include";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="override-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
    >
      <div className="w-full max-w-md rounded-lg border border-ink-700 bg-ink-900 p-5">
        <h2 id="override-title" className="text-sm font-semibold text-ink-100">
          Include an excluded file?
        </h2>
        <p className="mono mt-2 text-xs text-ink-300">{entry.path}</p>
        <p className="mt-3 rounded border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
          {entry.exclusion?.reason ?? `This file is classified as ${entry.class}.`}
        </p>
        {sensitive ? (
          <p className="mt-2 text-xs text-ink-300">
            Anything in this file becomes part of the bundle you copy or save. LeanAI cannot
            un-share it afterwards.
          </p>
        ) : null}
        <label className="mt-3 block text-xs text-ink-300">
          Type <span className="mono text-ink-100">{phrase}</span> to confirm
          <input
            value={typed}
            onChange={(event) => setTyped(event.target.value)}
            className="mt-1 w-full rounded border border-ink-700 bg-ink-950 px-2 py-1 text-xs"
            autoFocus
          />
        </label>
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="danger" disabled={typed.trim() !== phrase} onClick={onConfirm}>
            Include this file
          </Button>
        </div>
      </div>
    </div>
  );
}
