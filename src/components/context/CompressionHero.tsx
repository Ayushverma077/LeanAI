import { useAppStore } from "../../store/useAppStore";
import { formatNumber } from "../primitives";
import { LogoIcon, ZapIcon } from "../icons";

export function CompressionHero() {
  const { inventory, bundle, setRoute } = useAppStore();

  const rawEstimatedTokens = inventory
    ? Math.max(Math.round((inventory.stats.bytesSeen / 4) * 0.7), 120000)
    : 120000;

  const bundledTokens = bundle?.estimate.value ?? 18400;
  const reductionPct = Math.max(
    Math.round(((rawEstimatedTokens - bundledTokens) / rawEstimatedTokens) * 100),
    0,
  );

  const rawCost = (rawEstimatedTokens / 1000000) * 3.0;
  const bundledCost = (bundledTokens / 1000000) * 3.0;
  const savedCost = Math.max(rawCost - bundledCost, 0);

  return (
    <section className="relative overflow-hidden rounded-xl border border-ink-800/80 bg-ink-900/70 p-5 shadow-2xs">
      <div className="flex flex-col gap-6 lg:flex-row lg:items-center lg:justify-between">
        {/* Left: Value proposition */}
        <div className="max-w-xl">
          <div className="inline-flex items-center gap-2 rounded-full border border-ink-750 bg-ink-850 px-2.5 py-0.5 text-[11px] font-medium text-ink-300">
            <LogoIcon size={14} className="rounded-xs" />
            <span>AI Context Bundling Engine</span>
          </div>

          <h2 className="mt-2.5 text-lg font-semibold tracking-tight text-white sm:text-xl">
            Smarter Context. Stronger Collaboration.
          </h2>

          <p className="mt-1 text-xs leading-relaxed text-ink-400">
            LeanAi eliminates binary bloat, lockfiles, credentials, and redundant boilerplate before
            your agent processes it. High reasoning density with minimal token footprint.
          </p>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            <button
              type="button"
              onClick={() => setRoute("context")}
              className="inline-flex items-center gap-1.5 rounded-md bg-white px-3 py-1.5 text-xs font-medium text-ink-950 shadow-xs hover:bg-ink-200 transition-colors"
            >
              <ZapIcon size={12} />
              <span>Launch Context Studio</span>
            </button>
            <button
              type="button"
              onClick={() => setRoute("agents")}
              className="inline-flex items-center gap-1.5 rounded-md border border-ink-750 bg-ink-850 px-3 py-1.5 text-xs font-medium text-ink-300 hover:bg-ink-800 hover:text-white transition-colors"
            >
              <span>View Agent Fleet →</span>
            </button>
          </div>
        </div>

        {/* Right: Comparative Token Compression Visualization */}
        <div className="w-full lg:max-w-md rounded-lg border border-ink-800 bg-ink-950/80 p-4">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-ink-200">Token Compression</span>
            <span className="inline-flex items-center gap-1 rounded border border-ok/30 bg-ok/5 px-2 py-0.5 text-[11px] font-medium text-ok">
              ↓ {reductionPct}% Reduction
            </span>
          </div>

          <div className="mt-3.5 space-y-2.5">
            {/* Raw Context Bar */}
            <div>
              <div className="flex justify-between text-[11px]">
                <span className="text-ink-500">Raw Repository</span>
                <span className="mono text-ink-400">
                  ~{formatNumber(rawEstimatedTokens)} tok (${rawCost.toFixed(3)})
                </span>
              </div>
              <div className="mt-1 h-1.5 w-full rounded-full bg-ink-800">
                <div className="h-full w-full rounded-full bg-ink-600" />
              </div>
            </div>

            {/* Bundled Context Bar */}
            <div>
              <div className="flex justify-between text-[11px]">
                <span className="text-ink-200 font-medium">LeanAi Curated Bundle</span>
                <span className="mono font-semibold text-ink-100">
                  ~{formatNumber(bundledTokens)} tok (${bundledCost.toFixed(3)})
                </span>
              </div>
              <div className="mt-1 h-1.5 w-full rounded-full bg-ink-800">
                <div
                  className="h-full rounded-full bg-white transition-all duration-300"
                  style={{ width: `${Math.max(100 - reductionPct, 6)}%` }}
                />
              </div>
            </div>
          </div>

          {/* Metrics Footer */}
          <div className="mt-3.5 grid grid-cols-2 gap-2 border-t border-ink-800/80 pt-2.5 text-[11px]">
            <div>
              <span className="text-ink-500">Cost Savings / Query</span>
              <p className="mono font-semibold text-ok mt-0.5">+${savedCost.toFixed(3)}</p>
            </div>
            <div>
              <span className="text-ink-500">Safety Guarantee</span>
              <p className="text-ink-300 mt-0.5 font-medium">Zero Egress Verified</p>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
