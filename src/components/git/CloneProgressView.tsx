import type { CloneProgress } from "../../ipc/types";

/**
 * What git is doing right now, with the numbers it reports.
 *
 * Before this, a clone showed a disabled button and nothing else for as long
 * as the download took — ten minutes looked identical to a hang.
 */
export function CloneProgressView({ progress }: { progress: CloneProgress | null }) {
  const percent = progress?.percent ?? 0;
  const details = progress
    ? [
        progress.total > 0
          ? `${progress.current.toLocaleString("en-US")} / ${progress.total.toLocaleString("en-US")}`
          : null,
        progress.transferred,
        progress.speed,
      ]
        .filter(Boolean)
        .join(" · ")
    : "Waiting for the server to respond.";

  return (
    <div className="rounded-lg border border-ink-800 bg-ink-900/40 p-3 text-xs" role="status">
      <div className="flex items-baseline justify-between gap-3">
        <span className="font-medium text-ink-200">
          {progress ? progress.phase : "Connecting…"}
        </span>
        {progress ? <span className="mono text-ink-400">{percent}%</span> : null}
      </div>
      <div
        className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-ink-800"
        role="progressbar"
        aria-label={progress ? progress.phase : "Connecting"}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={progress ? percent : undefined}
      >
        {/* Indeterminate until git reports a percentage: a bar that pretends
            to know how far along it is would be worse than none. */}
        <div
          className={`h-full rounded-full bg-brand transition-[width] duration-300 ${
            progress ? "" : "w-full animate-pulse opacity-40"
          }`}
          style={progress ? { width: `${Math.max(percent, 2)}%` } : undefined}
        />
      </div>
      <p className="mono mt-1.5 text-[11px] text-ink-500">{details}</p>
    </div>
  );
}
