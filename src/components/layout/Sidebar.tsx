import { useEffect, useState } from "react";

import { api } from "../../ipc/client";
import type { SidecarStatus } from "../../ipc/types";
import { useAppStore, type Route } from "../../store/useAppStore";
import {
  AgentsIcon,
  ContextIcon,
  HistoryIcon,
  ModelsIcon,
  OverviewIcon,
  SettingsIcon,
  TasksIcon,
} from "../icons";

interface NavItem {
  route: Route;
  aliases?: Route[];
  label: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  badge?: string | number;
  needsProject?: boolean;
}

export function Sidebar() {
  const { route, setRoute, project, selection } = useAppStore();
  const [sidecarStatus, setSidecarStatus] = useState<SidecarStatus>({ state: "stopped" });

  useEffect(() => {
    api
      .localModelStatus()
      .then(setSidecarStatus)
      .catch(() => undefined);
  }, []);

  const workspaceNav: NavItem[] = [
    {
      route: "overview",
      aliases: ["workspace"],
      label: "Overview",
      icon: OverviewIcon,
    },
    {
      route: "context",
      aliases: ["select", "preview"],
      label: "Context",
      icon: ContextIcon,
      badge: selection.files.size > 0 ? selection.files.size : undefined,
      needsProject: true,
    },
    {
      route: "agents",
      label: "Agents",
      icon: AgentsIcon,
    },
    {
      route: "tasks",
      label: "Tasks",
      icon: TasksIcon,
    },
    {
      route: "history",
      label: "History",
      icon: HistoryIcon,
      needsProject: true,
    },
  ];

  const systemNav: NavItem[] = [
    {
      route: "models",
      label: "Models",
      icon: ModelsIcon,
    },
    {
      route: "settings",
      label: "Settings",
      icon: SettingsIcon,
    },
  ];

  const isNavActive = (item: NavItem) => {
    if (route === item.route) return true;
    if (item.aliases && item.aliases.includes(route)) return true;
    return false;
  };

  const NavButton = ({ item }: { item: NavItem }) => {
    const active = isNavActive(item);
    const disabled = Boolean(item.needsProject && !project);
    const Icon = item.icon;

    return (
      <button
        type="button"
        onClick={() => setRoute(item.route)}
        disabled={disabled}
        aria-current={active ? "page" : undefined}
        title={item.label}
        className={`group relative flex items-center gap-2.5 rounded-md px-2.5 py-1.5 text-xs font-medium transition-all duration-100 ${
          active
            ? "bg-brand/10 text-brand"
            : disabled
              ? "cursor-not-allowed text-ink-700"
              : "text-ink-400 hover:bg-ink-800/60 hover:text-ink-200"
        }`}
      >
        {active && (
          <span className="absolute left-0 top-1/2 -translate-y-1/2 w-0.5 h-4 rounded-r bg-brand" />
        )}
        <Icon
          size={14}
          className={active ? "text-brand" : "text-ink-500 group-hover:text-ink-300 transition-colors"}
        />
        <span>{item.label}</span>
        {item.badge !== undefined && (
          <span
            className={`ml-auto mono rounded px-1 py-px text-[9px] font-semibold ${
              active ? "bg-brand/20 text-brand" : "bg-ink-800 text-ink-500"
            }`}
          >
            {item.badge}
          </span>
        )}
      </button>
    );
  };

  return (
    <aside className="flex w-44 shrink-0 flex-col justify-between border-r border-ink-800/60 bg-ink-950 select-none">
      {/* Nav */}
      <div className="flex flex-col gap-5 p-2 pt-3">
        <nav aria-label="Main navigation" className="flex flex-col gap-0.5">
          {workspaceNav.map((item) => (
            <NavButton key={item.route} item={item} />
          ))}
        </nav>

        <div className="h-px bg-ink-800/60" />

        <nav aria-label="System navigation" className="flex flex-col gap-0.5">
          {systemNav.map((item) => (
            <NavButton key={item.route} item={item} />
          ))}
        </nav>
      </div>

      {/* Footer: sidecar dot */}
      <div className="border-t border-ink-800/60 px-3 py-2.5">
        <button
          type="button"
          onClick={() => setRoute("models")}
          className="flex items-center gap-2 text-[11px] text-ink-500 hover:text-ink-300 transition-colors w-full"
          title={`Sidecar: ${sidecarStatus.state}`}
        >
          <span
            className={`size-1.5 rounded-full shrink-0 ${
              sidecarStatus.state === "ready"
                ? "bg-ok"
                : sidecarStatus.state === "starting"
                  ? "bg-warn"
                  : "bg-ink-700"
            }`}
          />
          <span className="mono text-[10px]">
            {sidecarStatus.state === "ready" ? `sidecar :${sidecarStatus.port}` : sidecarStatus.state}
          </span>
        </button>
      </div>
    </aside>
  );
}
