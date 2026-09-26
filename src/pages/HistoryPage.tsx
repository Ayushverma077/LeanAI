import { useEffect, useState } from "react";
import { api, toAppError } from "../ipc/client";
import type { AuditEntry, BundleRecord } from "../ipc/types";
import { useAppStore } from "../store/useAppStore";
import {
  Button,
  Chip,
  EmptyState,
  Panel,
  formatBytes,
  formatNumber,
} from "../components/primitives";
import { HistoryIcon, ShieldCheckIcon } from "../components/icons";

export function HistoryPage() {
  const { project, setNotice, setError } = useAppStore();
  const [history, setHistory] = useState<BundleRecord[]>([]);
  const [audit, setAudit] = useState<AuditEntry[]>([]);
  const [activeTab, setActiveTab] = useState<"bundles" | "audit">("bundles");

  useEffect(() => {
    if (project) {
      api
        .bundleHistory()
        .then(setHistory)
        .catch((err) => setError(toAppError(err)));

      api
        .auditLog()
        .then(setAudit)
        .catch((err) => setError(toAppError(err)));
    }
  }, [project, setError]);

  if (!project) {
    return (
      <EmptyState
        icon={<HistoryIcon size={22} />}
        title="Open a repository to view history"
        body="Bundle histories and audit records are saved per-project directly in local SQLite."
      />
    );
  }

  const handleClearHistory = async () => {
    try {
      const count = await api.clearBundleHistory();
      setHistory([]);
      setNotice(`Cleared ${count} snapshot${count === 1 ? "" : "s"} from local database.`);
    } catch (err) {
      setError(toAppError(err));
    }
  };

  return (
    <div className="mx-auto max-w-5xl space-y-5 pb-6">
      {/* Header */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between border-b border-ink-800/80 pb-4">
        <div>
          <div className="flex items-center gap-2">
            <HistoryIcon size={18} className="text-brand" />
            <h1 className="text-base font-bold text-white tracking-tight">
              History & Logs
            </h1>
          </div>
        </div>

        <div className="flex items-center gap-2 mt-2 sm:mt-0">
          <div className="flex bg-ink-900/60 rounded-lg p-1 border border-ink-800/60">
            <button
              type="button"
              onClick={() => setActiveTab("bundles")}
              className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                activeTab === "bundles"
                  ? "bg-brand text-white shadow-xs"
                  : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Bundles ({history.length})
            </button>
            <button
              type="button"
              onClick={() => setActiveTab("audit")}
              className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                activeTab === "audit" 
                  ? "bg-brand text-white shadow-xs" 
                  : "text-ink-400 hover:text-ink-200"
              }`}
            >
              Audit ({audit.length})
            </button>
          </div>

          {activeTab === "bundles" && history.length > 0 && (
            <Button variant="danger" size="sm" onClick={handleClearHistory}>
              Clear
            </Button>
          )}
        </div>
      </div>

      {activeTab === "bundles" ? (
        <Panel
          title="Generated Context Snapshots"
          description="Stored in local SQLite. Contents are only retained if 'full_text' retention was enabled in preferences."
        >
          {history.length === 0 ? (
            <div className="py-10 text-center text-xs text-ink-500">
              No bundle history recorded yet for this project.
            </div>
          ) : (
            <div className="divide-y divide-ink-800">
              {history.map((record) => (
                <div key={record.id} className="flex items-center justify-between py-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="mono font-semibold text-xs text-ink-200">
                        sha256:{record.outputHash.slice(0, 16)}…
                      </span>
                      <Chip tone={record.hasText ? "ok" : "neutral"}>
                        {record.hasText ? "Full Text Stored" : "Metadata Only"}
                      </Chip>
                    </div>
                    <p className="mt-1 text-[11px] text-ink-400">
                      {formatNumber(record.fileCount)} files · {formatBytes(record.byteLen)}
                    </p>
                  </div>
                  <span className="mono text-[11px] text-ink-500">ID #{record.id}</span>
                </div>
              ))}
            </div>
          )}
        </Panel>
      ) : (
        <Panel
          title="Audit Trail Log"
          description="Every export, secret check, and bundle compilation performed by LeanAi."
        >
          {audit.length === 0 ? (
            <div className="py-10 text-center text-xs text-ink-500">
              No audit actions logged yet.
            </div>
          ) : (
            <div className="divide-y divide-ink-800">
              {audit.map((entry) => (
                <div key={entry.id} className="flex items-center justify-between py-3">
                  <div>
                    <div className="flex items-center gap-2">
                      <ShieldCheckIcon size={14} className="text-ok" />
                      <span className="text-xs font-semibold text-ink-100">{entry.task}</span>
                      <Chip tone="neutral">{entry.mode}</Chip>
                    </div>
                    <p className="mt-0.5 text-[11px] text-ink-400">
                      Status: <span className="text-ok font-medium">{entry.status}</span> ·{" "}
                      {new Date(entry.startedAtMs).toLocaleString()}
                    </p>
                  </div>
                  <span className="mono text-[11px] text-ink-500">{entry.id}</span>
                </div>
              ))}
            </div>
          )}
        </Panel>
      )}
    </div>
  );
}
