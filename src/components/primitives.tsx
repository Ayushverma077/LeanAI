import type { ReactNode } from "react";

export function Button({
  children,
  onClick,
  variant = "default",
  size = "sm",
  disabled,
  title,
  type = "button",
  className = "",
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: "default" | "primary" | "danger" | "ghost" | "secondary";
  size?: "xs" | "sm" | "md";
  disabled?: boolean;
  title?: string;
  type?: "button" | "submit";
  className?: string;
}) {
  const sizeStyles: Record<string, string> = {
    xs: "px-2 py-1 text-[11px] gap-1",
    sm: "px-2.5 py-1.5 text-xs gap-1.5",
    md: "px-3.5 py-2 text-xs font-medium gap-2",
  };

  const variantStyles: Record<string, string> = {
    primary:
      "bg-brand hover:bg-brand-light text-white font-medium border border-brand/80 shadow-xs active:opacity-90",
    default:
      "bg-ink-850 hover:bg-ink-800 text-ink-200 hover:text-ink-100 border border-ink-750 hover:border-ink-700 shadow-xs",
    secondary:
      "bg-ink-800 hover:bg-ink-750 text-ink-200 hover:text-ink-100 border border-ink-700 shadow-xs",
    danger: "bg-danger/10 hover:bg-danger/20 text-danger border border-danger/30 shadow-xs",
    ghost:
      "bg-transparent hover:bg-ink-800/70 text-ink-400 hover:text-ink-200 border border-transparent",
  };

  return (
    <button
      type={type}
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={`inline-flex items-center justify-center rounded-md font-medium transition-all duration-100 disabled:cursor-not-allowed disabled:opacity-35 disabled:hover:border-inherit disabled:hover:bg-inherit ${sizeStyles[size]} ${variantStyles[variant]} ${className}`}
    >
      {children}
    </button>
  );
}

export function Panel({
  title,
  description,
  badge,
  actions,
  children,
  className = "",
}: {
  title: string;
  description?: ReactNode;
  badge?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section
      className={`rounded-lg border border-ink-800/80 bg-ink-900/60 shadow-2xs ${className}`}
    >
      <header className="flex items-start justify-between gap-4 border-b border-ink-800/60 px-4 py-2.5 bg-ink-950/30">
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h2 className="text-xs font-semibold text-ink-100">{title}</h2>
            {badge ? <div>{badge}</div> : null}
          </div>
          {description ? (
            <div className="mt-0.5 text-[11px] text-ink-400 leading-normal">{description}</div>
          ) : null}
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-1.5">{actions}</div> : null}
      </header>
      <div className="p-4">{children}</div>
    </section>
  );
}

/**
 * A labelled state chip. The label text carries the meaning; the colour only
 * reinforces it (NFR 7.3).
 */
export function Chip({
  tone = "neutral",
  children,
  title,
  dot = false,
  className = "",
}: {
  tone?: "neutral" | "ok" | "warn" | "danger" | "info" | "purple";
  children: ReactNode;
  title?: string;
  dot?: boolean;
  className?: string;
}) {
  const tones = {
    neutral: {
      badge: "border-ink-800 bg-ink-850/80 text-ink-400",
      dot: "bg-ink-500",
    },
    ok: {
      badge: "border-ok/30 bg-ok/5 text-ok",
      dot: "bg-ok",
    },
    warn: {
      badge: "border-warn/30 bg-warn/5 text-warn",
      dot: "bg-warn",
    },
    danger: {
      badge: "border-danger/30 bg-danger/5 text-danger",
      dot: "bg-danger",
    },
    info: {
      badge: "border-ink-750 bg-ink-850/90 text-ink-300",
      dot: "bg-ink-300",
    },
    purple: {
      badge: "border-ink-750 bg-ink-850/90 text-ink-300",
      dot: "bg-ink-300",
    },
  };

  const activeTone = tones[tone] || tones.neutral;

  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1.5 rounded border px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap ${activeTone.badge} ${className}`}
    >
      {dot ? <span className={`size-1.5 rounded-full ${activeTone.dot}`} /> : null}
      {children}
    </span>
  );
}

export function EmptyState({
  icon,
  title,
  body,
  action,
}: {
  icon?: ReactNode;
  title: string;
  body: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-16 text-center">
      {icon ? (
        <div className="mb-1">{icon}</div>
      ) : null}
      <h3 className="text-sm font-semibold text-ink-100">{title}</h3>
      <p className="max-w-sm text-xs leading-relaxed text-ink-400">{body}</p>
      {action ? <div className="mt-2">{action}</div> : null}
    </div>
  );
}

export function StatCard({
  label,
  value,
  subtext,
  badge,
  icon,
}: {
  label: string;
  value: ReactNode;
  subtext?: ReactNode;
  badge?: ReactNode;
  icon?: ReactNode;
}) {
  return (
    <div className="rounded-lg border border-ink-800/80 bg-ink-900/60 p-3 shadow-2xs transition-colors hover:border-ink-700">
      <div className="flex items-center justify-between">
        <p className="text-[11px] text-ink-400 font-medium">{label}</p>
        {icon ? <div className="text-ink-500">{icon}</div> : badge}
      </div>
      <div className="mt-1 text-sm font-semibold text-ink-100 tracking-tight">{value}</div>
      {subtext ? <p className="mt-0.5 text-[10px] text-ink-500">{subtext}</p> : null}
    </div>
  );
}

export function ProgressBar({
  value,
  max,
  color = "accent",
  label,
}: {
  value: number;
  max: number;
  color?: "accent" | "ok" | "warn" | "danger" | "cyan";
  label?: string;
}) {
  const percentage = Math.min(Math.max((value / max) * 100, 0), 100);

  const colors: Record<string, string> = {
    accent: "bg-ink-100",
    ok: "bg-ok",
    warn: "bg-warn",
    danger: "bg-danger",
    cyan: "bg-ink-200",
  };

  return (
    <div className="w-full">
      {label ? (
        <div className="mb-1 flex justify-between text-[11px] text-ink-400">
          <span>{label}</span>
          <span className="mono font-medium text-ink-200">{percentage.toFixed(0)}%</span>
        </div>
      ) : null}
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-ink-800">
        <div
          className={`h-full rounded-full transition-all duration-200 ${colors[color]}`}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: ReactNode;
  children: ReactNode;
}) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs font-medium text-ink-300">{label}</span>
      {children}
      {hint ? <span className="text-[11px] text-ink-500 leading-normal">{hint}</span> : null}
    </label>
  );
}

export function Toggle({
  checked,
  onChange,
  label,
  hint,
}: {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  hint?: string;
}) {
  return (
    <label className="flex cursor-pointer items-start gap-2 py-1">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-0.5 size-3.5 rounded accent-ink-100"
      />
      <span>
        <span className="text-xs text-ink-200">{label}</span>
        {hint ? (
          <span className="block text-[11px] text-ink-500 leading-normal">{hint}</span>
        ) : null}
      </span>
    </label>
  );
}

export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return unit === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[unit]}`;
}

export function formatNumber(value: number): string {
  // Pinned to en-US grouping on purpose: token counts are a technical quantity
  // and locale-specific grouping (2,14,642 under en-IN) reads as a different
  // number to most people looking at a token budget.
  return value.toLocaleString("en-US");
}

export function Modal({
  open,
  onClose,
  title,
  description,
  children,
  className = "",
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  description?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity"
        onClick={onClose}
      />
      <div
        className={`relative z-10 w-full max-w-lg overflow-hidden rounded-xl border border-ink-800 bg-ink-900 shadow-2xl transition-all ${className}`}
        role="dialog"
        aria-modal="true"
      >
        <header className="flex items-start justify-between border-b border-ink-800/80 px-4 py-3 bg-ink-950/40">
          <div>
            <h3 className="text-sm font-semibold text-ink-100">{title}</h3>
            {description && (
              <p className="mt-0.5 text-xs text-ink-400 leading-normal">{description}</p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="text-ink-400 hover:text-ink-200 transition-colors p-1 rounded hover:bg-ink-800"
            title="Close"
          >
            ✕
          </button>
        </header>

        <div className="p-4">{children}</div>
      </div>
    </div>
  );
}
