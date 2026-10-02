import { useCallback, useEffect, useMemo, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { FileTree } from "../components/FileTree";
import { FolderPicker } from "../components/FolderPicker";
import { MiniMarkdown } from "../components/context/MiniMarkdown";
import {
  Button,
  Chip,
  EmptyState,
  Panel,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import { api, toAppError } from "../ipc/client";
import type {
  ContextDocument,
  ContextFileInfo,
  ContextSection,
  DiffScope,
  SelectionRecipe,
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
} from "../components/icons";

export function ContextBundlerPage() {
  const store = useAppStore();
  const buildBundle = useAppStore((state) => state.buildBundle);
  const { inventory, selection, git, options, bundle, building, context } = store;

  // Search & Filter state
  const [search, setSearch] = useState("");
  const [browseMode, setBrowseMode] = useState<"files" | "folders">("files");

  // Presets & Git diff
  const [presets, setPresets] = useState<PresetRecord[]>([]);
  const [presetName, setPresetName] = useState("");
  const [diffScopes, setDiffScopes] = useState<DiffScope[]>(["unstaged", "untracked"]);
  const [baseRef, setBaseRef] = useState("");
  const [showDiffFilter, setShowDiffFilter] = useState(false);

  // Dialog states
  const [pendingOverride, setPendingOverride] = useState<FileEntry | null>(null);
  const [preflight, setPreflight] = useState<ExportPreflight | null>(null);
  const [inspectedSource, setInspectedSource] = useState<SourceOnDemandResponse | null>(null);

  // Debounced bundle build. Depends on the stable action, not the whole
  // store, which changes after every build and would rebuild in a loop.
  useEffect(() => {
    const timer = setTimeout(() => {
      void buildBundle();
    }, 250);
    return () => clearTimeout(timer);
  }, [selection, options, buildBundle]);

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

  // The map on screen is the one in the bundle, so what you see is what is
  // copied; before the first build, fall back to the stored index.
  const mapDocument = bundle?.projectMap ?? context?.document ?? null;
  const mapFile = bundle?.projectMapFile ?? context?.file ?? null;
  const mapIncluded = options.includeProjectMap && (bundle?.projectMap ?? null) !== null;
  const canCopy = !!bundle && (selection.files.size > 0 || mapIncluded);
  // What Copy sends, in plain words: the parts, then how big it is.
  const contents = !bundle
    ? building
      ? "Getting things ready…"
      : "Pick files on the left, or include the project map."
    : [
        mapIncluded ? "Project map" : null,
        bundle.fileCount > 0
          ? `${formatNumber(bundle.fileCount)} file${bundle.fileCount === 1 ? "" : "s"}`
          : null,
      ]
        .filter(Boolean)
        .join(" + ");
  const size = bundle ? sizeGuide(bundle.estimate.value) : null;

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

  const handleStartExport = async () => {
    try {
      const data = await api.exportPreflight(toSpec(selection), options, "clipboard", null);
      setPreflight(data);
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  const handleConfirmExport = async () => {
    if (!preflight) return;
    try {
      const response = await api.exportBundle(toSpec(selection), options, "clipboard", null);
      if (response.text !== null) {
        await writeText(response.text);
      }
      store.setNotice(
        preflight.projectMapIncluded
          ? `Copied the project map and ${preflight.fileCount} file${preflight.fileCount === 1 ? "" : "s"} to the clipboard.`
          : "Context bundle copied to system clipboard.",
      );
      setPreflight(null);
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* File explorer and preview layout */}
      <div className="grid h-full min-h-0 gap-2.5 lg:grid-cols-[300px_minmax(0,1fr)] xl:grid-cols-[320px_minmax(0,1fr)]">
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

        {/* PANE 2: Everything the AI gets, in one place: the project map on
            top, the chosen files below, one Copy for both (ADR 0015). */}
        <div className="flex flex-col overflow-hidden rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs">
          <div className="flex flex-wrap items-center justify-between gap-3 border-b border-ink-800/80 px-3.5 py-2.5 bg-ink-950/40">
            <div className="min-w-0">
              <h2 className="text-xs font-semibold text-ink-100">What the AI gets</h2>
              <p className="text-[11px] text-ink-300">{contents}</p>
              {bundle && size ? (
                <p className="text-[11px] text-ink-500">
                  <span className={`font-semibold ${size.tone}`}>{size.word}</span>{" "}
                  <span
                    title={`About ${formatNumber(bundle.estimate.value)} tokens (${bundle.estimateLabel}). Tokens are how AI tools measure text; each AI counts a little differently.`}
                    className="underline decoration-dotted underline-offset-2"
                  >
                    (~{shortNumber(bundle.estimate.value)} tokens)
                  </span>{" "}
                  — {size.advice}
                </p>
              ) : null}
            </div>
            <Button
              variant="primary"
              size="sm"
              disabled={!canCopy}
              onClick={() => handleStartExport()}
            >
              <CopyIcon size={13} />
              <span>Copy for AI</span>
            </Button>
          </div>

          <div className="flex-1 space-y-4 overflow-y-auto p-4">
            <ContextFileAlert
              file={mapFile}
              onReplace={async () => {
                // Replacing is a generation too: afterwards, go on to Tasks.
                await store.regenerateMap(true);
                await buildBundle();
              }}
            />

            <ProjectMapCard
              included={options.includeProjectMap}
              document={mapDocument}
              onRegenerate={async () => {
                // Once the map is rebuilt, go on to Tasks to use it.
                await store.regenerateMap();
                await buildBundle();
              }}
              onInclude={(include) => store.setOptions({ includeProjectMap: include })}
              onOpenSource={async (path) => {
                try {
                  setInspectedSource(await api.fetchSource(path));
                } catch (err) {
                  store.setError(toAppError(err));
                }
              }}
            />

            <section aria-label="Selected files" className="space-y-2">
              <div className="flex items-center justify-between text-xs text-ink-400">
                <span className="flex items-center gap-1.5 font-semibold text-ink-200">
                  <FileCodeIcon size={12} className="text-ink-400" />
                  Selected files
                  {selection.files.size > 0 ? (
                    <span className="font-normal text-ink-400">
                      · {formatNumber(selection.files.size)} files · {formatBytes(selectedBytes)}
                    </span>
                  ) : null}
                </span>
                {bundle ? (
                  <span className="mono text-[11px] text-ink-500">
                    sha256 {bundle.outputHash.slice(0, 16)}…
                  </span>
                ) : null}
              </div>

              {selection.files.size === 0 ? (
                <p className="rounded-lg border border-dashed border-ink-800 p-4 text-center text-[11px] text-ink-500">
                  {options.includeProjectMap
                    ? "No files picked. Copy sends just the project map; pick files on the left to add their code below it."
                    : "Pick files on the left. This shows exactly what a model would receive."}
                </p>
              ) : building && !bundle ? (
                <div className="flex items-center justify-center py-12 text-xs text-ink-400">
                  <RefreshCwIcon size={16} className="animate-spin text-ink-300 mr-2" />
                  <span>Building preview…</span>
                </div>
              ) : bundle ? (
                <>
                  <pre className="mono max-h-[70vh] overflow-auto rounded-lg border border-ink-800 bg-ink-950 p-4 text-[11px] leading-relaxed whitespace-pre-wrap text-ink-300 select-text">
                    {bundle.preview}
                  </pre>
                  {bundle.previewTruncated ? (
                    <p className="rounded-md border border-warn/30 bg-warn/10 p-2.5 text-[11px] text-warn">
                      Preview shortened for display. The copy and every number here cover all{" "}
                      {formatNumber(bundle.fileCount)} files in full.
                    </p>
                  ) : null}
                </>
              ) : null}
            </section>

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
          preflight={preflight}
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

/**
 * Shown when PROJECT_CONTEXT.md already exists in the project and LeanAI left
 * it alone: the user decides whether LeanAI's map replaces it (ADR 0016).
 */
function ContextFileAlert({
  file,
  onReplace,
}: {
  file: ContextFileInfo | null;
  onReplace: () => Promise<void>;
}) {
  const [dismissed, setDismissed] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [replacing, setReplacing] = useState(false);
  if (!file?.alert || dismissed === file.state) return null;

  const title = file.canReplace
    ? "PROJECT_CONTEXT.md already exists in this project"
    : "LeanAI can't write PROJECT_CONTEXT.md in this project";
  return (
    <div role="alert" className="rounded-lg border border-warn/40 bg-warn/10 p-3 text-[11px]">
      <p className="font-semibold text-warn">{title}</p>
      <p className="mt-0.5 text-ink-300">{file.alert}</p>
      <div className="mt-2 flex flex-wrap items-center gap-2">
        {!file.canReplace ? (
          <Button size="xs" variant="ghost" onClick={() => setDismissed(file.state)}>
            Dismiss
          </Button>
        ) : confirming ? (
          <>
            <span className="text-ink-200">
              Replace your file with LeanAI&apos;s map? Its current content will be overwritten.
            </span>
            <Button
              size="xs"
              variant="primary"
              disabled={replacing}
              onClick={async () => {
                setReplacing(true);
                try {
                  await onReplace();
                } finally {
                  setReplacing(false);
                  setConfirming(false);
                }
              }}
            >
              {replacing ? "Replacing…" : "Yes, replace it"}
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setConfirming(false)}>
              Cancel
            </Button>
          </>
        ) : (
          <>
            <Button size="xs" onClick={() => setConfirming(true)}>
              Replace file
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setDismissed(file.state)}>
              Keep mine
            </Button>
          </>
        )}
      </div>
    </div>
  );
}

/** Freshness in plain words. */
const FRESHNESS_WORDS = { fresh: "Up to date", stale: "Out of date", unknown: "Can't check" };

/**
 * How big the copy is, in words anyone can act on. Tokens are how AI tools
 * measure text; the thresholds follow common chat limits and are guidance,
 * not a promise about any one AI.
 */
function sizeGuide(tokens: number): { word: string; tone: string; advice: string } {
  if (tokens <= 30_000) {
    return { word: "Small", tone: "text-ok", advice: "fits in almost any AI chat." };
  }
  if (tokens <= 120_000) {
    return {
      word: "Medium",
      tone: "text-ink-200",
      advice: "fits in most AI chats; some free plans may cut it off.",
    };
  }
  return {
    word: "Large",
    tone: "text-warn",
    advice: "too big for many AI chats. Pick fewer files.",
  };
}

/** 96217 → "96k", 1234567 → "1.2M". */
function shortNumber(value: number): string {
  if (value < 1_000) return String(value);
  if (value < 1_000_000) return `${Math.round(value / 1_000)}k`;
  return `${(value / 1_000_000).toFixed(1)}M`;
}

/** Overall freshness of the map: stale if any section is, unknown if any is. */
function mapFreshness(document: ContextDocument): "fresh" | "stale" | "unknown" {
  if (document.sections.some((section) => section.freshness === "stale")) return "stale";
  if (document.sections.some((section) => section.freshness === "unknown")) return "unknown";
  return "fresh";
}

/**
 * The project map (PROJECT_CONTEXT.md) as readable sections, with each
 * section's sources and limits one click away. It heads the copied text when
 * included.
 */
function ProjectMapCard({
  included,
  document,
  onRegenerate,
  onInclude,
  onOpenSource,
}: {
  included: boolean;
  document: ContextDocument | null;
  onRegenerate: () => Promise<void>;
  onInclude: (include: boolean) => void;
  onOpenSource: (path: string) => Promise<void>;
}) {
  const [regenerating, setRegenerating] = useState(false);
  const [open, setOpen] = useState(true);
  const regenerate = async () => {
    setRegenerating(true);
    try {
      await onRegenerate();
    } finally {
      setRegenerating(false);
    }
  };

  if (!included) {
    return (
      <div className="flex items-center justify-between gap-3 rounded-lg border border-dashed border-ink-800 px-3.5 py-2.5 text-[11px] text-ink-500">
        <span>
          The project map is off, so the AI gets only your files, without a summary of the project.
        </span>
        <Button size="xs" onClick={() => onInclude(true)}>
          Include it
        </Button>
      </div>
    );
  }

  if (!document) {
    return (
      <div className="flex items-center justify-center rounded-lg border border-ink-800 py-8 text-xs text-ink-400">
        <RefreshCwIcon size={14} className="mr-2 animate-spin text-ink-300" />
        Making the project map…
      </div>
    );
  }

  const freshness = mapFreshness(document);
  return (
    <section aria-label="Project map" className="rounded-lg border border-ink-800 bg-ink-950/70">
      <div className="flex items-center justify-between gap-2 px-3.5 py-2.5">
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="flex min-w-0 items-center gap-2 text-left"
        >
          <span aria-hidden className="text-[10px] text-ink-500">
            {open ? "▾" : "▸"}
          </span>
          <span className="text-xs font-semibold text-ink-100">Project map</span>
          <Chip
            tone={freshness === "fresh" ? "ok" : freshness === "stale" ? "warn" : "neutral"}
            dot
          >
            {FRESHNESS_WORDS[freshness]}
          </Chip>
        </button>
        <div className="flex shrink-0 items-center gap-3">
          <label className="flex cursor-pointer items-center gap-1.5 text-[11px] text-ink-300">
            <input
              type="checkbox"
              checked
              aria-label="Include project map"
              onChange={() => onInclude(false)}
              className="size-3.5 rounded accent-ink-100"
            />
            Include
          </label>
          <Button
            size="xs"
            disabled={regenerating}
            onClick={() => void regenerate()}
            title="Make the map again from the current files"
          >
            <RefreshCwIcon size={11} className={regenerating ? "animate-spin" : ""} />
            <span>{regenerating ? "Refreshing…" : "Refresh"}</span>
          </Button>
        </div>
      </div>

      {open ? (
        <div className="space-y-3 border-t border-ink-800 px-3.5 py-3">
          <p className="text-[11px] text-ink-500">
            A short summary of your project. The AI reads it first, before your files. It is also
            saved in your project as <span className="mono">PROJECT_CONTEXT.md</span>.
          </p>
          {freshness === "stale" ? (
            <p className="rounded-md border border-warn/30 bg-warn/10 p-2 text-[11px] text-warn">
              Some files changed since this map was made. Press Refresh to update it.
            </p>
          ) : null}
          {document.sections.map((section: ContextSection) => (
            <section key={section.key} aria-label={section.title} className="space-y-1.5">
              <h4 className="flex items-center gap-2 text-xs font-semibold text-ink-200">
                {section.title}
                {section.freshness === "stale" ? <Chip tone="warn">Out of date</Chip> : null}
              </h4>
              <MiniMarkdown text={section.body} />
              {section.sourceRefs.length > 0 || section.limitations.length > 0 ? (
                <details className="text-[10px] text-ink-500">
                  <summary className="cursor-pointer hover:text-ink-300">
                    Sources and limits
                  </summary>
                  {section.limitations.length > 0 ? (
                    <ul className="mt-1 list-disc space-y-0.5 pl-4">
                      {section.limitations.map((limitation) => (
                        <li key={limitation}>{limitation}</li>
                      ))}
                    </ul>
                  ) : null}
                  {section.sourceRefs.length > 0 ? (
                    <div className="mt-1.5 flex flex-wrap gap-1">
                      {section.sourceRefs.map((ref) => (
                        <button
                          key={ref.path}
                          type="button"
                          onClick={() => void onOpenSource(ref.path)}
                          className="mono rounded border border-ink-800 bg-ink-900 px-1.5 py-0.5 text-[10px] text-ink-300 hover:border-ink-600 hover:text-ink-100 transition-colors"
                        >
                          {ref.path}
                        </button>
                      ))}
                    </div>
                  ) : null}
                </details>
              ) : null}
            </section>
          ))}
        </div>
      ) : null}
    </section>
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
        {preflight.projectMapIncluded ? (
          <p className="mt-1 text-xs text-ink-400">
            The project map (PROJECT_CONTEXT.md) goes first, above the files. It is scanned too.
          </p>
        ) : null}

        <div className="mt-4 grid grid-cols-3 gap-3">
          <div className="rounded-lg border border-ink-800 bg-ink-950 p-2.5">
            <span className="text-[10px] text-ink-500">Files</span>
            <p className="mt-0.5 text-sm font-semibold text-ink-100">
              {formatNumber(preflight.fileCount)}
              {preflight.projectMapIncluded ? " + map" : ""}
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
