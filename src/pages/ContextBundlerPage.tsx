import { useCallback, useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { FileTree } from "../components/FileTree";
import { FolderPicker } from "../components/FolderPicker";
import {
  Button,
  Chip,
  EmptyState,
  Panel,
  Toggle,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import { TokenGauge } from "../components/context/TokenGauge";
import { api, toAppError } from "../ipc/client";
import type {
  ContextSection,
  DiffScope,
  FileClass,
  SelectionRecipe,
  ExportDestination,
  ExportPreflight,
  FileEntry,
  PresetRecord,
  SourceOnDemandResponse,
} from "../ipc/types";
import { toSpec, useAppStore } from "../store/useAppStore";
import {
  CopyIcon,
  FileCodeIcon,
  FolderIcon,
  GitBranchIcon,
  RefreshCwIcon,
  SearchIcon,
  ShieldCheckIcon,
  SparklesIcon,
} from "../components/icons";

/** Plain-language names for the classes shown in the exclusion summary. */
const EXCLUSION_LABELS: Record<FileClass, string> = {
  source_text: "Source text",
  binary: "Binary files",
  generated: "Generated and vendored output",
  lockfile: "Dependency lockfiles",
  credential_sensitive: "Credential-sensitive paths",
  hidden_metadata: "Hidden and editor metadata",
  too_large: "Too large for their type",
  symlink: "Symbolic links",
  unsupported_encoding: "Unsupported encoding",
  unreadable: "Unreadable",
};

export function ContextBundlerPage() {
  const store = useAppStore();
  const { inventory, selection, git, options, bundle, building, context } = store;

  // Search & Filter state
  const [search, setSearch] = useState("");
  const [centerTab, setCenterTab] = useState<"bundle" | "context">("bundle");
  const [browseMode, setBrowseMode] = useState<"files" | "folders">("files");

  // Presets & Git diff
  const [presets, setPresets] = useState<PresetRecord[]>([]);
  const [presetName, setPresetName] = useState("");
  const [diffScopes, setDiffScopes] = useState<DiffScope[]>(["unstaged", "untracked"]);
  const [baseRef, setBaseRef] = useState("");
  const [showDiffFilter, setShowDiffFilter] = useState(false);

  // Dialog states
  const [pendingOverride, setPendingOverride] = useState<FileEntry | null>(null);
  const [preflight, setPreflight] = useState<{
    data: ExportPreflight;
    destination: ExportDestination;
    targetPath: string | null;
  } | null>(null);
  const [inspectedSource, setInspectedSource] = useState<SourceOnDemandResponse | null>(null);

  // Debounced bundle build
  useEffect(() => {
    const timer = setTimeout(() => {
      void store.buildBundle();
    }, 250);
    return () => clearTimeout(timer);
  }, [selection, options, store]);

  const refreshPresets = useCallback(() => {
    api
      .listPresets()
      .then(setPresets)
      .catch((error) => store.setError(toAppError(error)));
  }, [store]);

  useEffect(() => {
    if (inventory) refreshPresets();
  }, [inventory?.sourceRevision, refreshPresets, inventory]);

  const selectableCount = useMemo(
    () => inventory?.files.filter((file) => file.selectable).length ?? 0,
    [inventory],
  );

  const selectedBytes = useMemo(() => {
    if (!inventory) return 0;
    let total = 0;
    for (const file of inventory.files) {
      if (selection.files.has(file.path)) total += file.sizeBytes;
    }
    return total;
  }, [inventory, selection.files]);

  /**
   * What the policy kept out, grouped by reason. On a large repository this is
   * the difference between a bundle that fits a context window and one that
   * does not, so it belongs on screen rather than buried in the file tree.
   */
  const excluded = useMemo(() => {
    const counts = new Map<FileClass, { files: number; bytes: number }>();
    for (const file of inventory?.files ?? []) {
      if (file.selectable) continue;
      const slot = counts.get(file.class) ?? { files: 0, bytes: 0 };
      slot.files += 1;
      slot.bytes += file.sizeBytes;
      counts.set(file.class, slot);
    }
    const rows = [...counts.entries()].sort((a, b) => b[1].bytes - a[1].bytes);
    return {
      files: rows.reduce((total, [, slot]) => total + slot.files, 0),
      bytes: rows.reduce((total, [, slot]) => total + slot.bytes, 0),
      rows,
    };
  }, [inventory]);

  if (!inventory) {
    return (
      <EmptyState
        icon={<FolderIcon size={22} />}
        title="No files indexed yet"
        body="Open a repository to start choosing context."
        action={
          <Button variant="primary" onClick={() => store.setRoute("overview")}>
            Open a repository
          </Button>
        }
      />
    );
  }

  const handleApplyDiff = async () => {
    try {
      const files = await api.changedFiles(diffScopes, baseRef.trim() || null);
      const selectable = new Set(
        inventory.files.filter((file) => file.selectable).map((file) => file.path),
      );
      const usable = files.map((file) => file.path).filter((path) => selectable.has(path));
      store.selectPaths(usable, true);
      store.setNotice(
        `Selected ${usable.length} of ${files.length} changed files. ${
          files.length - usable.length
        } excluded by policy.`,
      );
      setShowDiffFilter(false);
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  const handleStartExport = async (destination: ExportDestination) => {
    try {
      let targetPath: string | null = null;
      if (destination === "file") {
        targetPath = await save({
          title: "Save context bundle",
          defaultPath: "leanai-bundle.md",
          filters: [{ name: "Markdown", extensions: ["md", "txt"] }],
        });
        if (!targetPath) return;
      }
      const data = await api.exportPreflight(toSpec(selection), options, destination, targetPath);
      setPreflight({ data, destination, targetPath });
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  const handleConfirmExport = async () => {
    if (!preflight) return;
    try {
      const response = await api.exportBundle(
        toSpec(selection),
        options,
        preflight.destination,
        preflight.targetPath,
      );
      if (response.text !== null) {
        await writeText(response.text);
      }
      store.setNotice(
        response.writtenPath
          ? `Saved bundle to ${response.writtenPath} (manifest: ${response.manifestPath}).`
          : "Context bundle copied to system clipboard.",
      );
      setPreflight(null);
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* 3-Pane Studio Layout */}
      <div className="grid h-full min-h-0 gap-2.5 lg:grid-cols-[300px_minmax(0,1fr)_320px] xl:grid-cols-[320px_minmax(0,1fr)_340px]">
        {/* PANE 1: File Explorer & Presets */}
        <div className="flex flex-col overflow-hidden rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs">
          {/* Pane Header */}
          <div className="border-b border-ink-800/80 p-2.5 space-y-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5">
                <FolderIcon size={13} className="text-ink-400" />
                <h2 className="text-xs font-semibold text-ink-100">Files</h2>
              </div>
              <span className="mono text-[10px] text-ink-500">
                {formatNumber(selectableCount)} / {formatNumber(inventory.files.length)}
              </span>
            </div>

            {/* Search Input */}
            <div className="relative">
              <SearchIcon
                size={12}
                className="absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-500 pointer-events-none"
              />
              <input
                type="search"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Filter by path…"
                className="w-full rounded border border-ink-750 bg-ink-950 py-1 pl-7 pr-2.5 text-xs text-ink-100 placeholder:text-ink-500 focus:border-ink-600 focus:outline-hidden"
              />
            </div>

            {/* Quick Action Toolbar */}
            {/* One-click starting points, so a new repository does not open on
                an empty selection and a hundred checkboxes. */}
            <div className="flex flex-wrap items-center gap-1 pt-0.5">
              {(
                [
                  ["source_only", "Source"],
                  ["tests_only", "Tests"],
                  ["everything", "All"],
                ] as [SelectionRecipe, string][]
              ).map(([recipe, label]) => (
                <button
                  key={recipe}
                  type="button"
                  onClick={() => void store.applyRecipe(recipe)}
                  className="rounded border border-ink-800 px-2 py-1 text-[11px] text-ink-300 transition-colors hover:border-ink-700 hover:bg-ink-800 hover:text-ink-100"
                >
                  {label}
                </button>
              ))}
              <button
                type="button"
                onClick={store.clearSelection}
                className="rounded px-2 py-1 text-[11px] text-ink-400 hover:bg-ink-800 hover:text-danger"
              >
                Clear
              </button>
            </div>

            <div className="flex items-center justify-between pt-0.5 text-xs">
              <button
                type="button"
                onClick={() => setBrowseMode(browseMode === "files" ? "folders" : "files")}
                className={`flex items-center gap-1 rounded px-2 py-1 text-[11px] font-medium transition-colors ${
                  browseMode === "folders"
                    ? "bg-brand/15 text-brand"
                    : "text-ink-400 hover:bg-ink-800 hover:text-ink-200"
                }`}
              >
                <FolderIcon size={11} />
                <span>{browseMode === "folders" ? "Browsing folders" : "Add whole folders"}</span>
              </button>

              {git?.isRepository ? (
                <button
                  type="button"
                  onClick={() => setShowDiffFilter(!showDiffFilter)}
                  className={`flex items-center gap-1 rounded px-2 py-1 text-[11px] font-medium transition-colors ${
                    showDiffFilter
                      ? "bg-brand/15 text-brand"
                      : "text-ink-400 hover:bg-ink-800 hover:text-ink-200"
                  }`}
                >
                  <GitBranchIcon size={11} />
                  <span>Changed only</span>
                </button>
              ) : null}
            </div>

            {/* Expandable Git Diff Selection Panel */}
            {showDiffFilter && git?.isRepository ? (
              <div className="rounded-lg border border-ink-750 bg-ink-950/90 p-2.5 space-y-2 text-xs">
                <span className="font-medium text-ink-300">Include which changes?</span>
                <div className="grid grid-cols-2 gap-1 text-[11px]">
                  {(["staged", "unstaged", "untracked", "against_ref"] as DiffScope[]).map(
                    (scope) => (
                      <label key={scope} className="flex items-center gap-1.5 text-ink-300">
                        <input
                          type="checkbox"
                          checked={diffScopes.includes(scope)}
                          onChange={(e) =>
                            setDiffScopes((prev) =>
                              e.target.checked ? [...prev, scope] : prev.filter((s) => s !== scope),
                            )
                          }
                          className="rounded size-3 accent-brand"
                        />
                        <span className="capitalize">{scope.replace("_", " ")}</span>
                      </label>
                    ),
                  )}
                </div>
                {diffScopes.includes("against_ref") && (
                  <input
                    type="text"
                    placeholder="base ref (e.g. main)"
                    value={baseRef}
                    onChange={(e) => setBaseRef(e.target.value)}
                    className="w-full rounded border border-ink-700 bg-ink-900 px-2 py-1 text-[11px] text-ink-100"
                  />
                )}
                <Button variant="secondary" size="xs" className="w-full" onClick={handleApplyDiff}>
                  Select changed files
                </Button>
              </div>
            ) : null}
          </div>

          <div className="flex-1 overflow-hidden p-2">
            {browseMode === "files" ? (
              <FileTree
                inventory={inventory}
                selected={selection.files}
                overrides={selection.overrides}
                search={search}
                onToggleFile={store.toggleFile}
                onToggleDirectory={store.toggleDirectory}
                onRequestOverride={setPendingOverride}
              />
            ) : (
              <FolderPicker
                inventory={inventory}
                selected={selection.files}
                search={search}
                onToggleDirectory={store.toggleDirectory}
              />
            )}
          </div>

          {/* Pane Footer: Selection Count & Presets */}
          <div className="border-t border-ink-800/80 bg-ink-950/60 p-2.5">
            <div className="flex items-center justify-between text-[11px] text-ink-400">
              <span className="font-semibold text-ink-100">
                {formatNumber(selection.files.size)} selected
              </span>
              <span className="mono">{formatBytes(selectedBytes)}</span>
            </div>

            {/* Presets are set-once and were dominating the footer; they now
                live behind a disclosure. */}
            <details className="mt-2 group">
              <summary className="cursor-pointer list-none text-[11px] text-ink-500 hover:text-ink-300">
                Presets{presets.length > 0 ? ` (${presets.length})` : ""}
              </summary>
            <div className="mt-2 flex items-center gap-1.5">
              <input
                type="text"
                value={presetName}
                onChange={(e) => setPresetName(e.target.value)}
                placeholder="Name this selection…"
                className="w-full rounded border border-ink-800 bg-ink-900 px-2 py-1 text-[11px] text-ink-100 placeholder:text-ink-600"
              />
              <Button
                size="xs"
                disabled={!presetName.trim() || selection.files.size === 0}
                onClick={async () => {
                  try {
                    await api.savePreset(presetName.trim(), toSpec(selection), store.options);
                    setPresetName("");
                    refreshPresets();
                    store.setNotice(`Saved preset "${presetName.trim()}".`);
                  } catch (err) {
                    store.setError(toAppError(err));
                  }
                }}
              >
                Save
              </Button>
            </div>

            {presets.length > 0 ? (
              <div className="mt-2 flex flex-wrap gap-1">
                {presets.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => {
                      store.selectPaths(p.validation?.files ?? [], true);
                      store.setOptions(p.options);
                      store.setNotice(`Applied preset "${p.name}".`);
                    }}
                    className="rounded border border-ink-800 bg-ink-900 px-1.5 py-0.5 text-[10px] text-ink-300 hover:border-ink-700 hover:text-ink-100"
                  >
                    {p.name} ({p.validation?.files.length ?? 0})
                  </button>
                ))}
              </div>
            ) : null}
            </details>
          </div>
        </div>

        {/* PANE 2: Live Context Document & Markdown Viewer */}
        <div className="flex flex-col overflow-hidden rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs">
          {/* Header with Tabs and Actions */}
          <div className="flex flex-wrap items-center justify-between gap-2 border-b border-ink-800/80 px-3.5 py-2 bg-ink-950/40">
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => setCenterTab("bundle")}
                className={`flex items-center gap-1.5 rounded px-2.5 py-1 text-xs font-medium transition-colors ${
                  centerTab === "bundle"
                    ? "bg-ink-800 text-ink-100 shadow-2xs"
                    : "text-ink-400 hover:text-ink-200"
                }`}
              >
                <FileCodeIcon size={12} className="text-ink-300" />
                <span>Bundle</span>
              </button>

              <button
                type="button"
                onClick={() => setCenterTab("context")}
                className={`flex items-center gap-1.5 rounded px-2.5 py-1 text-xs font-medium transition-colors ${
                  centerTab === "context"
                    ? "bg-ink-800 text-ink-100 shadow-2xs"
                    : "text-ink-400 hover:text-ink-200"
                }`}
              >
                <SparklesIcon size={12} className="text-ink-300" />
                <span>PROJECT_CONTEXT.md</span>
                {context ? (
                  <span className="size-1.5 rounded-full bg-ok" />
                ) : (
                  <span className="size-1.5 rounded-full bg-warn" />
                )}
              </button>
            </div>

            {/* Export Toolbar */}
            <div className="ml-auto flex shrink-0 items-center gap-1.5">
              <Button
                variant="default"
                size="xs"
                disabled={!bundle || selection.files.size === 0}
                onClick={() => handleStartExport("clipboard")}
              >
                <CopyIcon size={12} />
                <span>Copy</span>
              </Button>
              <Button
                variant="primary"
                size="xs"
                className="shrink-0 whitespace-nowrap"
                disabled={!bundle || selection.files.size === 0}
                onClick={() => handleStartExport("file")}
              >
                <span>Save as…</span>
              </Button>
            </div>
          </div>

          {/* Center Pane Content */}
          <div className="flex-1 overflow-y-auto p-4">
            {centerTab === "bundle" ? (
              selection.files.size === 0 ? (
                <EmptyState
                  icon={<FileCodeIcon size={20} />}
                  title="Nothing selected"
                  body="Pick files on the left. The bundle preview shows exactly what a model would receive."
                />
              ) : building && !bundle ? (
                <div className="flex h-full items-center justify-center py-20 text-xs text-ink-400">
                  <RefreshCwIcon size={16} className="animate-spin text-ink-300 mr-2" />
                  <span>Building preview…</span>
                </div>
              ) : bundle ? (
                <div className="space-y-3">
                  <div className="flex items-center justify-between text-xs text-ink-400">
                    <span className="font-semibold text-ink-200">
                      {formatNumber(bundle.fileCount)} files · {formatBytes(bundle.byteLen)}
                    </span>
                    <span className="mono text-[11px] text-ink-500">
                      sha256 {bundle.outputHash.slice(0, 16)}…
                    </span>
                  </div>

                  {/* Formatted Code Box */}
                  <pre className="mono max-h-[70vh] overflow-auto rounded-lg border border-ink-800 bg-ink-950 p-4 text-[11px] leading-relaxed whitespace-pre-wrap text-ink-300 select-text">
                    {bundle.preview}
                  </pre>

                  {bundle.previewTruncated ? (
                    <p className="rounded-md border border-warn/30 bg-warn/10 p-2.5 text-[11px] text-warn">
                      Preview shortened for display. The saved bundle and every number here cover
                      all {formatNumber(bundle.fileCount)} files in full.
                    </p>
                  ) : null}
                </div>
              ) : null
            ) : (
              /* PROJECT_CONTEXT.md View */
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div>
                    <h3 className="text-xs font-semibold text-ink-100">Project context index</h3>
                    <p className="text-[11px] text-ink-400">
                      Structure, modules, routes and dependencies — written by code, not a model.
                      Every section links to the files it came from.
                    </p>
                  </div>
                  <Button variant="primary" size="xs" onClick={store.generateContext}>
                    {context ? "Regenerate" : "Generate"}
                  </Button>
                </div>

                {context ? (
                  <div className="space-y-2.5">
                    {context.document.sections.map((section: ContextSection) => (
                      <div
                        key={section.key}
                        className="rounded-lg border border-ink-800 bg-ink-950/70 p-3.5"
                      >
                        <div className="flex items-center justify-between">
                          <h4 className="text-xs font-semibold text-ink-200">{section.title}</h4>
                          <Chip tone={section.freshness === "fresh" ? "ok" : "warn"} dot>
                            {section.freshness}
                          </Chip>
                        </div>
                        <pre className="mono mt-2 max-h-48 overflow-auto rounded border border-ink-800/80 bg-ink-950 p-2.5 text-[11px] text-ink-400 whitespace-pre-wrap">
                          {section.body}
                        </pre>
                        {section.sourceRefs.length > 0 ? (
                          <div className="mt-2 flex flex-wrap gap-1">
                            {section.sourceRefs.map((ref) => (
                              <button
                                key={ref.path}
                                type="button"
                                onClick={async () => {
                                  try {
                                    setInspectedSource(await api.fetchSource(ref.path));
                                  } catch (err) {
                                    store.setError(toAppError(err));
                                  }
                                }}
                                className="mono rounded border border-ink-800 bg-ink-900 px-1.5 py-0.5 text-[10px] text-ink-300 hover:border-ink-600 hover:text-ink-100 transition-colors"
                              >
                                {ref.path}
                              </button>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    ))}
                  </div>
                ) : (
                  <EmptyState
                    title="Not generated yet"
                    body="Build a map of this repository that a model can read instead of the whole source."
                    action={
                      <Button variant="primary" onClick={store.generateContext}>
                        Generate
                      </Button>
                    }
                  />
                )}
              </div>
            )}
          </div>
        </div>

        {/* PANE 3: token budget, then the controls that change it */}
        <div className="flex flex-col overflow-y-auto space-y-2.5 rounded-lg border border-ink-800/80 bg-ink-900/60 p-2.5 shadow-2xs">
          <TokenGauge
            tokens={bundle?.estimate.value ?? 0}
            label={bundle?.estimateLabel ?? "Indicative cl100k estimate"}
            targetModel="Claude 3.5 Sonnet"
          />

          {/* Biggest contributors first: on a large repository this is the
              panel that tells you what to drop. */}
          {bundle && bundle.contributions.length > 0 ? (
            <Panel title="Biggest files" description="What is using the token budget.">
              <ul className="max-h-56 space-y-2 overflow-y-auto text-[11px]">
                {[...bundle.contributions]
                  .sort((a, b) => b.tokens - a.tokens)
                  .slice(0, 10)
                  .map((c) => (
                    <li key={c.path}>
                      <div className="flex justify-between gap-2">
                        <span className="mono truncate text-ink-300">{c.path}</span>
                        <span className="mono shrink-0 text-ink-400">
                          {formatNumber(c.tokens)} ({((c.share ?? 0) * 100).toFixed(0)}%)
                        </span>
                      </div>
                      <div className="mt-1 h-1 w-full rounded-full bg-ink-800">
                        <div
                          className="h-full rounded-full bg-brand"
                          style={{ width: `${Math.max((c.share ?? 0) * 100, 2)}%` }}
                        />
                      </div>
                    </li>
                  ))}
              </ul>
            </Panel>
          ) : null}

          {/* What the policy already kept out, and why. */}
          {excluded.files > 0 ? (
            <Panel
              title="Never bundled"
              description={`${formatNumber(excluded.files)} files · ${formatBytes(excluded.bytes)} the policy keeps out.`}
            >
              <ul className="space-y-1 text-[11px]">
                {excluded.rows.map(([fileClass, slot]) => (
                  <li key={fileClass} className="flex items-baseline justify-between gap-2">
                    <span className="truncate text-ink-300">{EXCLUSION_LABELS[fileClass]}</span>
                    <span className="mono shrink-0 text-ink-500">
                      {formatNumber(slot.files)} · {formatBytes(slot.bytes)}
                    </span>
                  </li>
                ))}
              </ul>
              <p className="mt-2 text-[10px] leading-relaxed text-ink-500">
                Each excluded file is still listed in the tree with its reason. Any of them can be
                added back one at a time.
              </p>
            </Panel>
          ) : null}

          {/* Format is set once and then ignored, so it collapses by default. */}
          <details className="rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs">
            <summary className="cursor-pointer list-none px-4 py-2.5 text-xs font-semibold text-ink-100">
              Output format
              <span className="ml-1.5 font-normal text-ink-500">— what each file block looks like</span>
            </summary>
            <div className="space-y-1 border-t border-ink-800/60 p-4 pt-3">
              <Toggle
                checked={options.headers}
                onChange={(headers) => store.setOptions({ headers })}
                label="File path headers"
                hint="Tells the model which file each block came from."
              />
              <Toggle
                checked={options.includeTree}
                onChange={(includeTree) => store.setOptions({ includeTree })}
                label="Directory tree preamble"
              />
              <Toggle
                checked={options.codeFences}
                onChange={(codeFences) => store.setOptions({ codeFences })}
                label="Code fences with language hints"
              />
              <Toggle
                checked={options.lineNumbers}
                onChange={(lineNumbers) => store.setOptions({ lineNumbers })}
                label="Line numbers"
                hint="Helps a model cite exact lines; costs roughly 10% more tokens."
              />
              <Toggle
                checked={options.fileSizeAnnotations}
                onChange={(fileSizeAnnotations) => store.setOptions({ fileSizeAnnotations })}
                label="Size and token annotations"
              />
              <Toggle
                checked={options.includeFrontMatter}
                onChange={(includeFrontMatter) => store.setOptions({ includeFrontMatter })}
                label="Embed provenance front matter"
              />
              <Toggle
                checked={options.normalizeLineEndings}
                onChange={(normalizeLineEndings) => store.setOptions({ normalizeLineEndings })}
                label="Normalise line endings"
                hint="Keeps output identical across Windows and macOS."
              />
            </div>
          </details>

          {bundle && (bundle.skipped.length > 0 || bundle.truncations.length > 0) ? (
            <Panel title="Warnings">
              <ul className="space-y-1 text-[11px] text-warn">
                {bundle.truncations.map((t) => (
                  <li key={t.path}>
                    {t.path} capped to {formatBytes(t.includedBytes)}
                  </li>
                ))}
                {bundle.skipped.map((f) => (
                  <li key={f}>{f} could not be read and was left out</li>
                ))}
              </ul>
            </Panel>
          ) : null}
        </div>
      </div>

      {/* Override Confirmation Dialog (FR-08 High Friction) */}
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

      {/* Preflight Export Safety Modal (ADR 0005) */}
      {preflight ? (
        <ExportPreflightDialog
          preflight={preflight.data}
          onCancel={() => setPreflight(null)}
          onConfirm={handleConfirmExport}
        />
      ) : null}

      {/* Source-on-demand inspection modal */}
      {inspectedSource ? (
        <SourceModal source={inspectedSource} onClose={() => setInspectedSource(null)} />
      ) : null}
    </div>
  );
}

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
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
    >
      <div className="w-full max-w-md rounded-xl border border-danger/40 bg-ink-900 p-5 shadow-2xl">
        <div className="flex items-center gap-2 text-danger">
          <ShieldCheckIcon size={18} />
          <h2 id="override-title" className="text-sm font-semibold">
            High Friction Safety Override
          </h2>
        </div>
        <p className="mono mt-2 text-xs text-ink-300">{entry.path}</p>
        <p className="mt-3 rounded-lg border border-warn/40 bg-warn/10 px-3 py-2 text-xs text-warn">
          {entry.exclusion?.reason ?? `This file is classified as ${entry.class}.`}
        </p>
        {sensitive ? (
          <p className="mt-2 text-xs text-ink-300">
            Exporting secrets violates standard security policy. Included credentials will be
            readable by AI models.
          </p>
        ) : null}
        <label className="mt-3.5 block text-xs text-ink-300">
          Type <span className="mono font-bold text-ink-100">{phrase}</span> to unlock:
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            className="mt-1 w-full rounded-md border border-ink-700 bg-ink-950 px-2.5 py-1.5 text-xs text-ink-100 focus:border-danger focus:outline-hidden"
            autoFocus
          />
        </label>
        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="danger" disabled={typed.trim() !== phrase} onClick={onConfirm}>
            Include File
          </Button>
        </div>
      </div>
    </div>
  );
}

function ExportPreflightDialog({
  preflight,
  onCancel,
  onConfirm,
}: {
  preflight: ExportPreflight;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const [acknowledged, setAcknowledged] = useState(false);
  const blocked = preflight.requiresConfirmation && !acknowledged;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="preflight-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
    >
      <div className="max-h-[85vh] w-full max-w-2xl overflow-y-auto rounded-xl border border-ink-700 bg-ink-900 p-5 shadow-2xl">
        <h2 id="preflight-title" className="text-sm font-semibold text-ink-100">
          Export Safety Preflight
        </h2>
        <p className="mt-1 text-xs text-ink-400">{preflight.destinationNote}</p>

        <div className="mt-4 grid grid-cols-3 gap-3">
          <div className="rounded-lg border border-ink-800 bg-ink-950 p-2.5">
            <span className="text-[10px] text-ink-500">Files</span>
            <p className="mt-0.5 text-sm font-semibold text-ink-100">
              {formatNumber(preflight.fileCount)}
            </p>
          </div>
          <div className="rounded-lg border border-ink-800 bg-ink-950 p-2.5">
            <span className="text-[10px] text-ink-500">Size</span>
            <p className="mt-0.5 text-sm font-semibold text-ink-100">
              {formatBytes(preflight.byteLen)}
            </p>
          </div>
          <div className="rounded-lg border border-ink-800 bg-ink-950 p-2.5">
            <span className="text-[10px] text-ink-500">Estimated Tokens</span>
            <p className="mt-0.5 text-sm font-semibold text-ok">
              ~{formatNumber(preflight.estimate.value)}
            </p>
          </div>
        </div>

        {/* Secret scan findings */}
        <h3 className="mt-4 text-xs font-semibold text-ink-300">Secret & Credential Scan</h3>
        <div className="mt-1 flex gap-2">
          <Chip tone={preflight.secretReport.high > 0 ? "danger" : "ok"} dot>
            {preflight.secretReport.high} high risk
          </Chip>
          <Chip tone={preflight.secretReport.medium > 0 ? "warn" : "neutral"}>
            {preflight.secretReport.medium} medium
          </Chip>
          <Chip tone="neutral">{preflight.secretReport.low} low</Chip>
        </div>

        {preflight.requiresConfirmation ? (
          <label className="mt-4 flex items-start gap-2 text-xs text-ink-200">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(e) => setAcknowledged(e.target.checked)}
              className="mt-0.5 size-4 rounded accent-ink-100"
            />
            <span>
              I understand that the selected files contain potential sensitive credentials. Export
              anyway.
            </span>
          </label>
        ) : null}

        <div className="mt-5 flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" disabled={blocked} onClick={onConfirm}>
            Export Context
          </Button>
        </div>
      </div>
    </div>
  );
}

function SourceModal({ source, onClose }: { source: SourceOnDemandResponse; onClose: () => void }) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-xs select-none"
    >
      <div className="max-h-[85vh] w-full max-w-3xl overflow-y-auto rounded-xl border border-ink-700 bg-ink-900 p-5 shadow-2xl">
        <div className="flex items-start justify-between">
          <div>
            <h3 className="mono text-xs font-semibold text-ink-100">{source.path}</h3>
            <p className="mt-0.5 text-[11px] text-ink-500">
              lines {source.fromLine}–{source.toLine} of {source.totalLines}
            </p>
          </div>
          <Button variant="ghost" size="xs" onClick={onClose}>
            Close
          </Button>
        </div>

        <pre className="mono mt-3 max-h-[60vh] overflow-auto rounded-lg border border-ink-800 bg-ink-950 p-3 text-[11px] text-ink-300 whitespace-pre-wrap select-text">
          {source.content}
        </pre>
      </div>
    </div>
  );
}
