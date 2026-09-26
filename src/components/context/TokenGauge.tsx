import { Chip, formatNumber } from "../primitives";
import { SparklesIcon, ZapIcon } from "../icons";

interface TokenGaugeProps {
  tokens: number;
  label?: string;
  targetModel?: string;
  contextCap?: number;
  inputPricePer1M?: number;
}

export function TokenGauge({
  tokens,
  label = "Indicative cl100k estimate",
  targetModel = "Claude 3.5 Sonnet",
  contextCap = 200000,
  inputPricePer1M = 3.0,
}: TokenGaugeProps) {
  const ratio = tokens / contextCap;
  const percentage = Math.min(ratio * 100, 100);
  /** A selection that does not fit is the single most important thing to say. */
  const overflows = tokens > contextCap;

  // Status zones: <32k green, 32k-128k amber, >128k red
  const zone =
    tokens === 0 ? "empty" : tokens < 32000 ? "lean" : tokens < 128000 ? "balanced" : "heavy";

  const zoneColors = {
    empty: {
      text: "text-ink-400",
      bar: "bg-ink-600",
      tone: "neutral" as const,
      badge: "Empty",
    },
    lean: {
      text: "text-ok",
      bar: "bg-ok",
      tone: "ok" as const,
      badge: "Comfortable",
    },
    balanced: {
      text: "text-warn",
      bar: "bg-warn",
      tone: "warn" as const,
      badge: "Getting large",
    },
    heavy: {
      text: "text-danger",
      bar: "bg-danger",
      tone: "danger" as const,
      badge: "Too large for most windows",
    },
  };

  const currentZone = zoneColors[zone];
  const estimatedCost = (tokens / 1000000) * inputPricePer1M;

  return (
    <div className="rounded-lg border border-ink-800/80 bg-ink-900/60 p-3.5 shadow-2xs">
      {/* Header with Title and Zone Badge */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <SparklesIcon size={12} className="text-ink-400" />
          <h3 className="text-xs font-semibold text-ink-100">Token budget</h3>
        </div>
        <Chip tone={currentZone.tone} dot>
          {currentZone.badge}
        </Chip>
      </div>

      {/* Main Token Metric */}
      <div className="mt-3 flex items-baseline justify-between">
        <div>
          <div className="flex items-baseline gap-1.5">
            <span className="mono text-xl font-bold tracking-tight text-ink-100">
              {tokens > 0 ? `~${formatNumber(tokens)}` : "0"}
            </span>
            <span className="text-[11px] text-ink-400 font-medium">tokens</span>
          </div>
          <p className="text-[10px] text-ink-500 mt-0.5">{label}</p>
        </div>

        {/* Rough input cost. Deliberately imprecise: it is an estimated token
            count multiplied by an assumed rate, so four decimal places would
            imply accuracy this number does not have (FR-16). */}
        <div className="text-right">
          <div className="flex items-center justify-end gap-1 text-ink-200">
            <ZapIcon size={11} className="text-ink-400" />
            <span className="mono text-xs font-medium">
              {tokens > 0 ? `~$${estimatedCost < 0.01 ? "<0.01" : estimatedCost.toFixed(2)}` : "—"}
            </span>
          </div>
          <p className="text-[10px] text-ink-500">
            input only · ${inputPricePer1M}/1M assumed
          </p>
        </div>
      </div>

      {/* Context Window Capacity Bar */}
      <div className="mt-3.5">
        <div className="flex items-center justify-between text-[10px] text-ink-400 mb-1">
          <span>Target: {targetModel}</span>
          <span className={`mono ${overflows ? "font-semibold text-danger" : ""}`}>
            {overflows
              ? `${formatNumber(tokens - contextCap)} over ${formatNumber(contextCap / 1000)}k`
              : `${tokens > 0 ? percentage.toFixed(1) : 0}% of ${formatNumber(contextCap / 1000)}k`}
          </span>
        </div>
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-ink-800">
          <div
            className={`h-full rounded-full transition-all duration-300 ${currentZone.bar}`}
            style={{ width: `${Math.max(percentage, tokens > 0 ? 3 : 0)}%` }}
          />
        </div>
        {/* Tier markers */}
        <div className="mt-1 flex justify-between text-[9px] text-ink-500 mono">
          <span>0</span>
          <span>32k</span>
          <span>128k</span>
          <span>{formatNumber(contextCap / 1000)}k</span>
        </div>
      </div>

      {/* Compliance Disclaimer (FR-16) */}
      <div className="mt-3 rounded border border-ink-800/80 bg-ink-950/70 p-2 text-[10px] leading-relaxed text-ink-500">
        <span className="text-ink-400 font-medium">Notice: </span>
        Token counts come from the cl100k BPE tokenizer, which is an OpenAI-family count. Other
        providers tokenise differently and bill by their own count, so treat both the tokens and
        the cost as indicative, never as a quote.
      </div>
    </div>
  );
}
