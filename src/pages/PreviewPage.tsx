import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import {
  Button,
  Chip,
  EmptyState,
  Panel,
  Toggle,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import { api, toAppError } from "../ipc/client";
import type { ExportDestination, ExportPreflight } from "../ipc/types";
import { toSpec, useAppStore } from "../store/useAppStore";

export function PreviewPage() {
  const store = useAppStore();
  const { bundle, building, selection, options } = store;
  const [preflight, setPreflight] = useState<{
    data: ExportPreflight;
    destination: ExportDestination;
    targetPath: string | null;
  } | null>(null);

  // Rebuild after a debounce so option changes feel immediate without
  // rebuilding on every keystroke (FR-13).
  useEffect(() => {
    const timer = setTimeout(() => {
      void store.buildBundle();
    }, 200);
    return () => clearTimeout(timer);
  }, [selection, options, store]);

  if (selection.files.size === 0) {
    return (
      <EmptyState
        title="Nothing selected"
        body="Choose files first; the preview shows exactly what a model would receive."
        action={<Button onClick={() => store.setRoute("select")}>Choose files</Button>}
      />
    );
  }

  const startExport = async (destination: ExportDestination) => {
    try {
      let targetPath: string | null = null;
      if (destination === "file") {
        targetPath = await save({
          title: "Save bundle",
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

  const confirmExport = async () => {
    if (!preflight) return;
    try {
      const response = await api.exportBundle(
        toSpec(selection),
        options,
        preflight.destination,
        preflight.targetPath,
      );
      if (response.text !== null) await writeText(response.text);
      store.setNotice(
        response.writtenPath
          ? `Saved to ${response.writtenPath} (manifest: ${response.manifestPath}).`
          : "Bundle copied to the clipboard.",
      );
      setPreflight(null);
    } catch (error) {
      store.setError(toAppError(error));
    }
  };

  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,2fr)_minmax(320px,1fr)]">
      <Panel
        title="Preview"
        actions={
          <>
            <Button onClick={() => startExport("clipboard")} disabled={!bundle}>
              Copy…
            </Button>
            <Button variant="primary" onClick={() => startExport("file")} disabled={!bundle}>
              Save as…
            </Button>
          </>
        }
      >
        {building && !bundle ? (
          <p className="py-8 text-center text-xs text-ink-500" role="status">
            Building…
          </p>
        ) : bundle ? (
          <>
            <pre className="mono max-h-[55vh] overflow-auto rounded-lg border border-ink-800 bg-ink-950 p-4 text-[11px] leading-relaxed whitespace-pre-wrap text-ink-300">
              {bundle.preview}
            </pre>
            {bundle.previewTruncated ? (
              <p className="mt-2.5 rounded border border-warn/30 bg-warn/10 p-2.5 text-[11px] text-warn">
                Preview shortened for display. The full bundle contains all content.
              </p>
            ) : null}
          </>
        ) : (
          <p className="py-8 text-center text-xs text-ink-500">No bundle yet.</p>
        )}
      </Panel>

      <div className="space-y-4">
        <Panel title="Estimate">
          {bundle ? (
            <>
              <div className="grid grid-cols-2 gap-3">
                <Stat label="Files" value={formatNumber(bundle.fileCount)} />
                <Stat label="Bytes" value={formatBytes(bundle.byteLen)} />
              </div>
              <div className="mt-3 rounded-lg border border-ink-800 bg-ink-950/40 px-3 py-3">
                <p className="text-xl font-semibold text-brand">
                  ~{formatNumber(bundle.estimate.value)} <span className="text-sm font-normal text-ink-400">tokens</span>
                </p>
                <p className="mt-1 text-[11px] text-ink-500">{bundle.estimateLabel}</p>
              </div>
              <p className="mono mt-3 text-[11px] break-all text-ink-500">
                sha256 {bundle.outputHash.slice(0, 32)}…
              </p>
            </>
          ) : (
            <p className="text-xs text-ink-500">Build a bundle to see its size.</p>
          )}
        </Panel>

        <Panel title="Options">
          <Toggle
            checked={options.headers}
            onChange={(headers) => store.setOptions({ headers })}
            label="File path headers"
          />
          <Toggle
            checked={options.includeTree}
            onChange={(includeTree) => store.setOptions({ includeTree })}
            label="Directory tree preamble"
          />
          <Toggle
            checked={options.codeFences}
            onChange={(codeFences) => store.setOptions({ codeFences })}
            label="Code fences"
          />
          <Toggle
            checked={options.lineNumbers}
            onChange={(lineNumbers) => store.setOptions({ lineNumbers })}
            label="Line numbers"
          />
          <Toggle
            checked={options.fileSizeAnnotations}
            onChange={(fileSizeAnnotations) => store.setOptions({ fileSizeAnnotations })}
            label="Size and token annotations"
          />
          <Toggle
            checked={options.includeFrontMatter}
            onChange={(includeFrontMatter) => store.setOptions({ includeFrontMatter })}
            label="Embed provenance manifest"
          />
          <Toggle
            checked={options.normalizeLineEndings}
            onChange={(normalizeLineEndings) => store.setOptions({ normalizeLineEndings })}
            label="Normalise line endings"
          />
        </Panel>

        {bundle && bundle.contributions.length > 0 ? (
          <Panel title="Token usage">
            <ul className="max-h-64 space-y-2 overflow-auto text-[11px]">
              {[...bundle.contributions]
                .sort((a, b) => b.tokens - a.tokens)
                .map((contribution) => (
                  <li key={contribution.path}>
                    <div className="flex justify-between gap-2">
                      <span className="mono truncate text-ink-300">{contribution.path}</span>
                      <span className="shrink-0 text-ink-500">
                        {formatNumber(contribution.tokens)} ·{" "}
                        {(contribution.share * 100).toFixed(1)}%
                      </span>
                    </div>
                    <div className="mt-1 h-1.5 rounded-full bg-ink-800">
                      <div
                        className="h-1.5 rounded-full bg-brand"
                        style={{ width: `${Math.max(contribution.share * 100, 1)}%` }}
                      />
                    </div>
                  </li>
                ))}
            </ul>
          </Panel>
        ) : null}

        {bundle && (bundle.skipped.length > 0 || bundle.truncations.length > 0) ? (
          <Panel title="Warnings">
            <ul className="space-y-1 text-[11px] text-warn">
              {bundle.truncations.map((truncation) => (
                <li key={truncation.path}>
                  {truncation.path} truncated to {formatBytes(truncation.includedBytes)} of{" "}
                  {formatBytes(truncation.originalBytes)}
                </li>
              ))}
              {bundle.skipped.map((path) => (
                <li key={path}>{path} could not be read at build time and was left out.</li>
              ))}
            </ul>
          </Panel>
        ) : null}
      </div>

      {preflight ? (
        <ExportPreflightDialog
          preflight={preflight.data}
          onCancel={() => setPreflight(null)}
          onConfirm={confirmExport}
        />
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
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
    >
      <div className="max-h-[85vh] w-full max-w-lg overflow-auto rounded-xl border border-ink-800/60 bg-ink-950 p-6 shadow-2xl">
        <h2 id="preflight-title" className="text-base font-semibold text-white">
          Review Export
        </h2>
        
        <div className="mt-4 grid grid-cols-3 gap-3">
          <Stat label="Files" value={formatNumber(preflight.fileCount)} />
          <Stat label="Size" value={formatBytes(preflight.byteLen)} />
          <Stat label="Tokens" value={`~${formatNumber(preflight.estimate.value)}`} />
        </div>

        {preflight.secretReport.findings.length > 0 ? (
          <>
            <div className="mt-5 flex items-center justify-between">
              <h3 className="text-sm font-semibold text-ink-100">Secret scan findings</h3>
              <div className="flex gap-2">
                <Chip tone={preflight.secretReport.high > 0 ? "danger" : "neutral"}>
                  {preflight.secretReport.high} high
                </Chip>
                <Chip tone={preflight.secretReport.medium > 0 ? "warn" : "neutral"}>
                  {preflight.secretReport.medium} medium
                </Chip>
              </div>
            </div>
            
            <ul className="mt-3 max-h-40 space-y-2 overflow-auto text-xs">
              {preflight.secretReport.findings.map((finding) => (
                <li
                  key={`${finding.path}:${finding.line}:${finding.rule}`}
                  className="rounded-lg border border-ink-800/60 bg-ink-900/40 p-2.5"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="mono text-ink-300 truncate">
                      {finding.path}:{finding.line}
                    </span>
                    <Chip tone={finding.confidence === "high" ? "danger" : "warn"}>
                      {finding.rule}
                    </Chip>
                  </div>
                  <p className="mono mt-2 break-all text-ink-500 bg-ink-950 p-2 rounded">{finding.redactedExcerpt}</p>
                </li>
              ))}
            </ul>
          </>
        ) : null}

        {preflight.requiresConfirmation ? (
          <label className="mt-5 flex items-start gap-3 rounded-lg border border-warn/30 bg-warn/10 p-3 text-xs text-ink-100 cursor-pointer">
            <input
              type="checkbox"
              checked={acknowledged}
              onChange={(event) => setAcknowledged(event.target.checked)}
              className="mt-0.5 size-4 rounded accent-brand"
            />
            <span>I have reviewed the findings above and want to export these files anyway.</span>
          </label>
        ) : null}

        <div className="mt-6 flex justify-end gap-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button variant="primary" disabled={blocked} onClick={onConfirm}>
            Export
          </Button>
        </div>
      </div>
    </div>
  );
}
