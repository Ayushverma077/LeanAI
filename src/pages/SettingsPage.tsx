import { useEffect, useState } from "react";

import { Button, Chip, Field, Panel, Toggle, formatBytes } from "../components/primitives";
import { GitAuthSettings } from "../components/git/GitAuthSettings";
import { api, toAppError } from "../ipc/client";
import type { AuditEntry, BundleRecord, Diagnostics, Settings } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";

export function SettingsPage() {
  const store = useAppStore();
  const { settings, policy, project } = store;
  const [draft, setDraft] = useState<Settings | null>(settings);
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [history, setHistory] = useState<BundleRecord[]>([]);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [aiIgnore, setAiIgnore] = useState<{ contents: string; preview: string | null }>({
    contents: "",
    preview: null,
  });

  useEffect(() => setDraft(settings), [settings]);

  useEffect(() => {
    api
      .diagnostics()
      .then(setDiagnostics)
      .catch(() => undefined);
    if (project) {
      api
        .bundleHistory()
        .then(setHistory)
        .catch(() => undefined);
      api
        .auditLog()
        .then(setAudit)
        .catch(() => undefined);
      api
        .readAiIgnore()
        .then((file) => setAiIgnore({ contents: file.contents || file.template, preview: null }))
        .catch(() => undefined);
    }
  }, [project]);

  if (!draft) return <p className="text-xs text-ink-500">Loading settings…</p>;

  const update = (partial: Partial<Settings>) => setDraft({ ...draft, ...partial });

  return (
    <div className="grid gap-4 lg:grid-cols-2">
      <Panel
        title="Privacy"
        actions={
          <Button variant="primary" onClick={() => store.saveSettings(draft)}>
            Save
          </Button>
        }
      >
        <Field
          label="Bundle history retention"
          hint="Metadata refers to hashes, counts, and paths — never file contents."
        >
          <select
            value={draft.bundleRetention}
            onChange={(event) =>
              update({ bundleRetention: event.target.value as Settings["bundleRetention"] })
            }
            className="rounded border border-ink-800 bg-ink-950 px-2 py-1 text-xs text-ink-200"
          >
            <option value="metadata_only">Metadata only (default)</option>
            <option value="full_text">Metadata and bundle text</option>
            <option value="none">Keep nothing</option>
          </select>
        </Field>
        <div className="mt-4 space-y-3">
          <Toggle
            checked={draft.requireExportConfirmation}
            onChange={(requireExportConfirmation) => update({ requireExportConfirmation })}
            label="Require confirmation for secret scan warnings"
          />
          <Toggle
            checked={draft.telemetryOptIn}
            onChange={(telemetryOptIn) => update({ telemetryOptIn })}
            label="Send anonymous usage counts"
            hint="Never includes text, paths or credentials."
          />
        </div>
      </Panel>

      <GitAuthSettings />

      <Panel title="Scan policy">
        <div className="space-y-3">
          <Toggle
            checked={draft.policy.respectGitIgnore}
            onChange={(respectGitIgnore) => update({ policy: { ...draft.policy, respectGitIgnore } })}
            label="Respect git excludes (.gitignore, global excludes)"
          />
          <Toggle
            checked={draft.policy.respectAiIgnore}
            onChange={(respectAiIgnore) => update({ policy: { ...draft.policy, respectAiIgnore } })}
            label="Respect .aiignore"
          />
          <Toggle
            checked={draft.policy.excludeLockfiles}
            onChange={(excludeLockfiles) => update({ policy: { ...draft.policy, excludeLockfiles } })}
            label="Exclude dependency lockfiles"
          />
          <Toggle
            checked={draft.policy.excludeGenerated}
            onChange={(excludeGenerated) => update({ policy: { ...draft.policy, excludeGenerated } })}
            label="Exclude generated and minified output"
          />
          <Toggle
            checked={draft.policy.includeHidden}
            onChange={(includeHidden) => update({ policy: { ...draft.policy, includeHidden } })}
            label="Include hidden files"
          />
          <Toggle
            checked={draft.policy.followSymlinks}
            onChange={(followSymlinks) => update({ policy: { ...draft.policy, followSymlinks } })}
            label="Follow symbolic links"
          />
          <Field label="Maximum size per file">
            <input
              type="number"
              min={1024}
              step={1024}
              value={draft.policy.limits.maxFileBytes}
              onChange={(event) =>
                update({
                  policy: {
                    ...draft.policy,
                    limits: { ...draft.policy.limits, maxFileBytes: Number(event.target.value) },
                  },
                })
              }
              className="rounded border border-ink-800 bg-ink-950 px-2 py-1 text-xs text-ink-200"
            />
          </Field>
          <p className="mt-1 text-[11px] text-ink-500">
            Currently {formatBytes(draft.policy.limits.maxFileBytes)}.
          </p>
          <div className="mt-4 flex gap-2">
            <Button variant="primary" onClick={() => store.saveSettings(draft)}>
              Save policy
            </Button>
            <Button
              onClick={async () => {
                const reset = await api.resetSettings();
                setDraft(reset);
                store.setNotice("Settings restored to defaults.");
              }}
            >
              Restore defaults
            </Button>
          </div>
        </div>
      </Panel>

      {policy ? (
        <Panel title="Ignore precedence">
          <ol className="space-y-1 text-xs">
            {policy.ignore_precedence.map((entry) => (
              <li key={entry.source} className="flex items-center gap-2">
                <span className="w-5 text-ink-500">{entry.rank}</span>
                <span className="text-ink-100">{entry.label}</span>
              </li>
            ))}
          </ol>
          <h3 className="mt-4 text-xs font-semibold text-ink-300">Why limits exist</h3>
          <dl className="mt-1 space-y-2 text-[11px]">
            {policy.limit_reasons.map((entry) => (
              <div key={entry.key}>
                <dt className="mono text-ink-300">{entry.key}</dt>
                <dd className="text-ink-500">{entry.reason}</dd>
              </div>
            ))}
          </dl>
        </Panel>
      ) : null}

      {project ? (
        <Panel title=".aiignore">
          <textarea
            value={aiIgnore.contents}
            onChange={(event) => setAiIgnore({ contents: event.target.value, preview: null })}
            rows={8}
            aria-label=".aiignore contents"
            className="mono w-full rounded-md border border-ink-800 bg-ink-950 p-2.5 text-[11px] text-ink-300 focus:border-brand focus:outline-hidden"
          />
          <div className="mt-2 flex gap-2">
            <Button
              onClick={async () => {
                try {
                  const preview = await api.previewAiIgnore(aiIgnore.contents);
                  setAiIgnore((current) => ({
                    ...current,
                    preview: [
                      `${preview.newlyExcluded.length} file(s) would be newly excluded.`,
                      `${preview.selectableBefore} → ${preview.selectableAfter} selectable files.`,
                      preview.unmatchedRules.length
                        ? `Rules matching nothing: ${preview.unmatchedRules.join(", ")}`
                        : "",
                      preview.invalidRules.length
                        ? `Invalid rules: ${preview.invalidRules.join("; ")}`
                        : "",
                    ]
                      .filter(Boolean)
                      .join("\n"),
                  }));
                } catch (error) {
                  store.setError(toAppError(error));
                }
              }}
            >
              Preview
            </Button>
            <Button
              variant="primary"
              disabled={aiIgnore.preview === null}
              title={aiIgnore.preview === null ? "Preview the effect first" : undefined}
              onClick={async () => {
                try {
                  await api.writeAiIgnore(aiIgnore.contents);
                  store.setNotice(".aiignore written. Rescan to apply it.");
                } catch (error) {
                  store.setError(toAppError(error));
                }
              }}
            >
              Write .aiignore
            </Button>
          </div>
          {aiIgnore.preview ? (
            <pre className="mt-2 rounded-md border border-ink-800 bg-ink-950 p-2.5 text-[11px] whitespace-pre-wrap text-ink-300">
              {aiIgnore.preview}
            </pre>
          ) : null}
        </Panel>
      ) : null}

      {project ? (
        <Panel
          title="Local data"
          actions={
            <Button
              variant="danger"
              size="sm"
              onClick={async () => {
                const removed = await api.clearBundleHistory();
                setHistory([]);
                store.setNotice(`Cleared ${removed} history entr${removed === 1 ? "y" : "ies"}.`);
              }}
            >
              Clear history
            </Button>
          }
        >
          <ul className="max-h-48 space-y-1.5 overflow-auto text-[11px]">
            {history.map((record) => (
              <li
                key={record.id}
                className="flex justify-between gap-2 border-b border-ink-800/60 pb-1.5"
              >
                <span className="mono truncate text-ink-300">
                  {record.outputHash.slice(0, 16)}…
                </span>
                <span className="shrink-0 text-ink-500">
                  {record.fileCount} files · {formatBytes(record.byteLen)} ·{" "}
                  {record.hasText ? "text kept" : "metadata only"}
                </span>
              </li>
            ))}
            {history.length === 0 ? <li className="text-ink-500">No bundles recorded.</li> : null}
          </ul>
        </Panel>
      ) : null}

      {project ? (
        <Panel title="Audit log">
          <ul className="max-h-48 space-y-1.5 overflow-auto text-[11px]">
            {audit.map((entry) => (
              <li key={entry.id} className="border-b border-ink-800/60 pb-1.5">
                <span className="text-ink-100">{entry.task}</span> <Chip>{entry.mode}</Chip>{" "}
                <span className="text-ink-500">
                  {new Date(entry.startedAtMs).toLocaleString()} · {entry.status}
                </span>
              </li>
            ))}
            {audit.length === 0 ? <li className="text-ink-500">Nothing recorded yet.</li> : null}
          </ul>
        </Panel>
      ) : null}

      {diagnostics ? (
        <Panel title="Diagnostics">
          <dl className="grid grid-cols-2 gap-y-3 text-[11px]">
            <Row label="App version" value={diagnostics.appVersion} />
            <Row label="Platform" value={`${diagnostics.platform} / ${diagnostics.arch}`} />
            <Row
              label="Database schema"
              value={`v${diagnostics.schemaVersion} (latest v${diagnostics.latestSchemaVersion})`}
            />
            <Row label="Policy version" value={`v${diagnostics.policyVersion}`} />
          </dl>
          <h3 className="mt-4 text-xs font-semibold text-ink-300">Granted capabilities</h3>
          <ul className="mt-2 space-y-1 text-[11px] text-ink-500">
            {diagnostics.capabilities.map((capability) => (
              <li key={capability}>{capability}</li>
            ))}
          </ul>
          <p className="mt-3 rounded bg-ink-950 p-2 text-[11px] text-ink-500">{diagnostics.notice}</p>
        </Panel>
      ) : null}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <>
      <dt className="text-ink-500">{label}</dt>
      <dd className="text-ink-100">{value}</dd>
    </>
  );
}
