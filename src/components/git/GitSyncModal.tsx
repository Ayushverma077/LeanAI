import { useState } from "react";

import { useAppStore } from "../../store/useAppStore";
import { Button, Chip, Modal } from "../primitives";
import { GitBranchIcon, RefreshCwIcon, UploadCloudIcon } from "../icons";

interface GitSyncModalProps {
  open: boolean;
  onClose: () => void;
}

export function GitSyncModal({ open, onClose }: GitSyncModalProps) {
  const { git, remoteStatus, pushBranch, loadRemoteStatus } = useAppStore();
  const [pushing, setPushing] = useState(false);
  const [force, setForce] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  if (!open) return null;

  const handlePush = async () => {
    setPushing(true);
    setErrorMsg(null);
    try {
      await pushBranch({ force });
      onClose();
    } catch (e: unknown) {
      const msg =
        typeof e === "object" && e && "message" in e
          ? String((e as { message: string }).message)
          : "Failed to push commits to remote.";
      setErrorMsg(msg);
    } finally {
      setPushing(false);
    }
  };

  const currentBranch = remoteStatus?.currentBranch ?? git?.headRef ?? "main";
  const upstream = remoteStatus?.upstreamBranch;
  const ahead = remoteStatus?.ahead ?? 0;
  const behind = remoteStatus?.behind ?? 0;
  const unpushed = remoteStatus?.unpushedCommits ?? [];
  const primaryRemote = remoteStatus?.remotes[0];

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Git Remote Synchronization"
      description={`Synchronize local changes with ${primaryRemote?.name ?? "origin"} on branch ${currentBranch}.`}
    >
      <div className="space-y-4">
        {/* Remote details card */}
        <div className="rounded-lg border border-ink-800 bg-ink-950/60 p-3 space-y-2">
          <div className="flex items-center justify-between text-xs">
            <div className="flex items-center gap-2 text-ink-200">
              <GitBranchIcon size={14} className="text-accent" />
              <span className="font-semibold">{currentBranch}</span>
              {upstream && (
                <>
                  <span className="text-ink-600">→</span>
                  <span className="text-ink-400 font-mono text-[11px]">{upstream}</span>
                </>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              <Button
                variant="ghost"
                size="sm"
                onClick={() => loadRemoteStatus()}
                title="Refresh remote status"
              >
                <RefreshCwIcon size={12} />
              </Button>
            </div>
          </div>

          {primaryRemote && (
            <div className="flex items-center justify-between text-[11px] text-ink-500 pt-1 border-t border-ink-850">
              <span className="truncate max-w-[280px]" title={primaryRemote.url}>
                {primaryRemote.url}
              </span>
              <Chip>{primaryRemote.protocol.toUpperCase()}</Chip>
            </div>
          )}
        </div>

        {/* Sync status counters */}
        <div className="grid grid-cols-2 gap-2 text-xs">
          <div className="rounded border border-ink-800 bg-ink-900/40 p-2.5 flex items-center justify-between">
            <div>
              <div className="text-ink-500 text-[10px] uppercase font-mono tracking-wider">
                Ahead
              </div>
              <div className="text-sm font-semibold text-ink-100">{ahead} unpushed</div>
            </div>
            <Chip tone={ahead > 0 ? "purple" : "neutral"}>↑ {ahead}</Chip>
          </div>

          <div className="rounded border border-ink-800 bg-ink-900/40 p-2.5 flex items-center justify-between">
            <div>
              <div className="text-ink-500 text-[10px] uppercase font-mono tracking-wider">
                Behind
              </div>
              <div className="text-sm font-semibold text-ink-100">{behind} to pull</div>
            </div>
            <Chip tone={behind > 0 ? "warn" : "neutral"}>↓ {behind}</Chip>
          </div>
        </div>

        {git?.isDirty && (
          <div className="rounded border border-warn/30 bg-warn/10 p-2.5 text-[11px] text-warn-light flex items-center gap-2">
            <span className="size-1.5 rounded-full bg-warn shrink-0" />
            <span>
              Working tree has uncommitted changes. Only committed changes will be pushed.
            </span>
          </div>
        )}

        {/* Unpushed commits list */}
        {unpushed.length > 0 && (
          <div className="space-y-1.5">
            <div className="text-[11px] font-medium text-ink-400">
              Commits to be pushed ({unpushed.length}):
            </div>
            <div className="max-h-40 overflow-y-auto space-y-1 rounded border border-ink-850 bg-ink-950 p-2 text-xs">
              {unpushed.map((c) => (
                <div
                  key={c.id}
                  className="flex items-start justify-between gap-2 border-b border-ink-850/60 pb-1 last:border-0"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-ink-200 text-[11px]">{c.summary}</p>
                    <p className="text-[10px] text-ink-500">
                      {c.author} · {new Date(c.timestampMs).toLocaleString()}
                    </p>
                  </div>
                  <span className="font-mono text-[10px] text-accent shrink-0">
                    {c.id.slice(0, 7)}
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}

        {behind > 0 && (
          <div className="rounded border border-warn/30 bg-warn/10 p-2.5 text-[11px] text-warn-light">
            Remote tracking branch has new commits. Pushing may require pulling or fast-forwarding
            first.
          </div>
        )}

        {/* Force push option */}
        <div className="flex items-center gap-2 pt-1 text-xs text-ink-400">
          <input
            type="checkbox"
            id="forcePushCheck"
            checked={force}
            onChange={(e) => setForce(e.target.checked)}
            className="rounded border-ink-750 bg-ink-950 text-accent focus:ring-0"
          />
          <label htmlFor="forcePushCheck" className="text-[11px] select-none cursor-pointer">
            Force push with lease (--force-with-lease)
          </label>
        </div>

        {errorMsg && (
          <div className="rounded border border-red-500/30 bg-red-500/10 p-2.5 text-xs text-red-400 font-mono whitespace-pre-wrap">
            {errorMsg}
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center justify-end gap-2 pt-2 border-t border-ink-800/80">
          <Button variant="ghost" onClick={onClose} disabled={pushing}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handlePush}
            disabled={pushing || ahead === 0}
            className="flex items-center gap-1.5"
          >
            <UploadCloudIcon size={14} />
            <span>
              {pushing
                ? "Pushing…"
                : `Push ${ahead > 0 ? `${ahead} ` : ""}Commit${ahead === 1 ? "" : "s"}`}
            </span>
          </Button>
        </div>
      </div>
    </Modal>
  );
}
