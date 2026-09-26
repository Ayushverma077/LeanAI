import { useAppStore } from "../../store/useAppStore";
import { GitBranchIcon, ShieldCheckIcon } from "../icons";
import { formatNumber } from "../primitives";

export function StatusBar() {
  const { project, git, inventory, selection, bundle, setRoute } = useAppStore();

  const totalFiles = inventory?.stats.filesSeen ?? 0;
  const selectedCount = selection.files.size;
  const tokenEstimate = bundle?.estimate.value ?? 0;

  return (
    <footer className="flex h-5 shrink-0 items-center justify-between border-t border-ink-800/60 bg-ink-950 px-3 text-[10px] text-ink-500 select-none">
      {/* Left */}
      <div className="flex items-center gap-2.5">
        {project ? (
          <>
            {git?.isRepository && (
              <div
                className="flex items-center gap-1 cursor-pointer hover:text-ink-300 transition-colors"
                onClick={() => setRoute("context")}
              >
                <GitBranchIcon size={10} />
                <span className="mono">{git.headRef ?? "detached"}</span>
                <span className={`size-1.5 rounded-full ${git.isDirty ? "bg-warn" : "bg-ok"}`} />
              </div>
            )}
            <span
              className="cursor-pointer hover:text-ink-300 transition-colors"
              onClick={() => setRoute("context")}
            >
              {formatNumber(totalFiles)} files
            </span>
            {selectedCount > 0 && (
              <span
                className="text-brand font-medium cursor-pointer hover:text-brand-light transition-colors"
                onClick={() => setRoute("context")}
              >
                {selectedCount} selected
                {tokenEstimate > 0 && (
                  <span className="ml-1 mono text-ink-500 font-normal">
                    (~{formatNumber(tokenEstimate)} tok)
                  </span>
                )}
              </span>
            )}
          </>
        ) : (
          <span className="text-ink-700">No project</span>
        )}
      </div>

      {/* Right */}
      <div
        className="flex items-center gap-1 cursor-pointer hover:text-ink-300 transition-colors"
        onClick={() => setRoute("settings")}
      >
        <ShieldCheckIcon size={11} className="text-ok" />
        <span>Offline · Zero Egress</span>
      </div>
    </footer>
  );
}
