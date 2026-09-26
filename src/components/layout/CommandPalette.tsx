import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { useAppStore } from "../../store/useAppStore";
import {
  AgentsIcon,
  ContextIcon,
  HistoryIcon,
  ModelsIcon,
  OverviewIcon,
  RefreshCwIcon,
  SearchIcon,
  SettingsIcon,
  TasksIcon,
} from "../icons";

interface CommandItem {
  id: string;
  title: string;
  category: "Navigation" | "Project" | "Bundle Options";
  shortcut?: string;
  icon: React.ComponentType<{ size?: number; className?: string }>;
  action: () => void;
}

export function CommandPalette({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) {
  const {
    setRoute,
    project,
    inventory,
    scan,
    openProject,
    selectPaths,
    clearSelection,
    options,
    setOptions,
  } = useAppStore();
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  const commands: CommandItem[] = useMemo(() => {
    const list: CommandItem[] = [
      {
        id: "nav-overview",
        title: "Go to Mission Control",
        category: "Navigation",
        shortcut: "G O",
        icon: OverviewIcon,
        action: () => setRoute("overview"),
      },
      {
        id: "nav-context",
        title: "Go to Context Studio",
        category: "Navigation",
        shortcut: "G C",
        icon: ContextIcon,
        action: () => setRoute("context"),
      },
      {
        id: "nav-agents",
        title: "Go to Agent Fleet & Orchestration",
        category: "Navigation",
        shortcut: "G A",
        icon: AgentsIcon,
        action: () => setRoute("agents"),
      },
      {
        id: "nav-tasks",
        title: "Go to Task Workspace",
        category: "Navigation",
        shortcut: "G T",
        icon: TasksIcon,
        action: () => setRoute("tasks"),
      },
      {
        id: "nav-history",
        title: "Go to Bundle History & Audit",
        category: "Navigation",
        shortcut: "G H",
        icon: HistoryIcon,
        action: () => setRoute("history"),
      },
      {
        id: "nav-models",
        title: "Configure Models & Cloud Providers",
        category: "Navigation",
        shortcut: "G M",
        icon: ModelsIcon,
        action: () => setRoute("models"),
      },
      {
        id: "nav-settings",
        title: "Open Preferences & Safety Policy",
        category: "Navigation",
        shortcut: "⌘,",
        icon: SettingsIcon,
        action: () => setRoute("settings"),
      },
      {
        id: "proj-open",
        title: "Open Project Directory…",
        category: "Project",
        shortcut: "⌘O",
        icon: SearchIcon,
        action: async () => {
          const selected = await open({
            directory: true,
            multiple: false,
            title: "Choose a project",
          });
          if (typeof selected === "string") await openProject(selected);
        },
      },
    ];

    if (project) {
      list.push({
        id: "proj-rescan",
        title: "Rescan Project Repository",
        category: "Project",
        shortcut: "⌘R",
        icon: RefreshCwIcon,
        action: () => scan(),
      });
    }

    if (inventory) {
      list.push(
        {
          id: "select-all",
          title: "Select All Selectable Files",
          category: "Project",
          icon: ContextIcon,
          action: () => {
            selectPaths(
              inventory.files.filter((f) => f.selectable).map((f) => f.path),
              true,
            );
            setRoute("context");
          },
        },
        {
          id: "clear-selection",
          title: "Clear All Selected Files",
          category: "Project",
          icon: ContextIcon,
          action: () => clearSelection(),
        },
      );
    }

    list.push(
      {
        id: "toggle-line-numbers",
        title: `Toggle Line Numbers in Bundle (${options.lineNumbers ? "Disable" : "Enable"})`,
        category: "Bundle Options",
        icon: ContextIcon,
        action: () => setOptions({ lineNumbers: !options.lineNumbers }),
      },
      {
        id: "toggle-code-fences",
        title: `Toggle Code Fences (${options.codeFences ? "Disable" : "Enable"})`,
        category: "Bundle Options",
        icon: ContextIcon,
        action: () => setOptions({ codeFences: !options.codeFences }),
      },
      {
        id: "toggle-front-matter",
        title: `Toggle Provenance Front Matter (${options.includeFrontMatter ? "Disable" : "Enable"})`,
        category: "Bundle Options",
        icon: ContextIcon,
        action: () => setOptions({ includeFrontMatter: !options.includeFrontMatter }),
      },
    );

    return list;
  }, [
    project,
    inventory,
    options,
    setRoute,
    openProject,
    scan,
    selectPaths,
    clearSelection,
    setOptions,
  ]);

  const filtered = useMemo(() => {
    if (!query.trim()) return commands;
    const lower = query.toLowerCase();
    return commands.filter(
      (c) => c.title.toLowerCase().includes(lower) || c.category.toLowerCase().includes(lower),
    );
  }, [commands, query]);

  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (isOpen) onClose();
        else onClose(); // parent handles toggle
      }
      if (!isOpen) return;

      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % Math.max(filtered.length, 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + filtered.length) % Math.max(filtered.length, 1));
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (filtered[selectedIndex]) {
          filtered[selectedIndex].action();
          onClose();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose, filtered, selectedIndex]);

  if (!isOpen) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command Palette"
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/75 pt-[14vh] backdrop-blur-xs select-none"
      onClick={onClose}
    >
      <div
        className="w-full max-w-xl overflow-hidden rounded-xl border border-ink-700/80 bg-ink-900 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search Input Bar */}
        <div className="flex items-center gap-3 border-b border-ink-800 px-4 py-2.5">
          <SearchIcon size={14} className="text-ink-400 shrink-0" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Type a command or jump to screen..."
            className="w-full bg-transparent text-xs text-ink-100 placeholder:text-ink-500 focus:outline-hidden"
            autoFocus
          />
          <span className="mono rounded border border-ink-750 bg-ink-850 px-1.5 py-0.5 text-[9px] text-ink-400">
            ESC
          </span>
        </div>

        {/* Command List */}
        <div className="max-h-80 overflow-y-auto p-1.5">
          {filtered.length === 0 ? (
            <div className="py-8 text-center text-xs text-ink-500">No commands match "{query}"</div>
          ) : (
            <ul className="space-y-0.5">
              {filtered.map((item, index) => {
                const isSelected = index === selectedIndex;
                const Icon = item.icon;

                return (
                  <li key={item.id}>
                    <button
                      type="button"
                      onClick={() => {
                        item.action();
                        onClose();
                      }}
                      onMouseEnter={() => setSelectedIndex(index)}
                      className={`flex w-full items-center justify-between rounded px-2.5 py-1.5 text-xs transition-colors ${
                        isSelected
                          ? "bg-ink-800 text-white font-medium"
                          : "text-ink-300 hover:bg-ink-850 hover:text-ink-100"
                      }`}
                    >
                      <div className="flex items-center gap-2">
                        <Icon size={13} className={isSelected ? "text-white" : "text-ink-500"} />
                        <span>{item.title}</span>
                      </div>
                      <div className="flex items-center gap-2">
                        <span className="text-[10px] text-ink-500">{item.category}</span>
                        {item.shortcut ? (
                          <kbd
                            className={`mono rounded px-1.5 py-0.2 text-[9px] ${
                              isSelected
                                ? "border border-ink-700 bg-ink-750 text-white"
                                : "border border-ink-800 bg-ink-950 text-ink-400"
                            }`}
                          >
                            {item.shortcut}
                          </kbd>
                        ) : null}
                      </div>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {/* Footer info */}
        <div className="flex items-center justify-between border-t border-ink-800/80 bg-ink-950/60 px-4 py-2 text-[11px] text-ink-500">
          <span>Navigate with ↑ and ↓</span>
          <span>Press Enter to run</span>
        </div>
      </div>
    </div>
  );
}
