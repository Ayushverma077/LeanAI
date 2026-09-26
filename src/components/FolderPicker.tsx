import { useMemo } from "react";

import type { Inventory } from "../ipc/types";
import { formatBytes, formatNumber } from "./primitives";
import { FolderIcon } from "./icons";

interface FolderRow {
  path: string;
  depth: number;
  /** Selectable files in this directory and everything below it. */
  files: string[];
  bytes: number;
}

/**
 * Flat list of every directory that holds selectable files.
 *
 * The file tree can already tick a folder, but on a large repository you have
 * to expand your way down to find one. This shows every folder at once with
 * its real weight, so "add the whole of `src/api`" is a single click and you
 * can see what it costs before making it.
 *
 * Totals roll up the subtree, mirroring `selection::directory_summaries` in the
 * Rust core.
 */
export function FolderPicker({
  inventory,
  selected,
  search,
  onToggleDirectory,
}: {
  inventory: Inventory;
  selected: Set<string>;
  search: string;
  onToggleDirectory: (directory: string, selected: boolean, paths: string[]) => void;
}) {
  const rows = useMemo(() => {
    const totals = new Map<string, { files: string[]; bytes: number }>();
    for (const entry of inventory.files) {
      if (!entry.selectable) continue;
      const segments = entry.path.split("/");
      for (let depth = 1; depth < segments.length; depth += 1) {
        const directory = segments.slice(0, depth).join("/");
        const slot = totals.get(directory) ?? { files: [], bytes: 0 };
        slot.files.push(entry.path);
        slot.bytes += entry.sizeBytes;
        totals.set(directory, slot);
      }
    }

    const term = search.trim().toLowerCase();
    return [...totals.entries()]
      .filter(([path]) => !term || path.toLowerCase().includes(term))
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map<FolderRow>(([path, slot]) => ({
        path,
        depth: path.split("/").length - 1,
        files: slot.files,
        bytes: slot.bytes,
      }));
  }, [inventory.files, search]);

  if (rows.length === 0) {
    return (
      <p className="px-3 py-8 text-center text-xs text-ink-500">
        {search.trim() ? "No folder matches that search." : "No folders hold selectable files."}
      </p>
    );
  }

  return (
    <div className="h-[60vh] overflow-auto rounded border border-ink-800 bg-ink-950">
      <ul>
        {rows.map((row) => {
          const hits = row.files.reduce((total, path) => total + (selected.has(path) ? 1 : 0), 0);
          const state = hits === 0 ? "none" : hits === row.files.length ? "all" : "some";
          const name = row.path.slice(row.path.lastIndexOf("/") + 1);

          return (
            <li key={row.path}>
              <label
                className="flex cursor-pointer items-center gap-2 py-1 pr-3 text-xs hover:bg-ink-900"
                style={{ paddingLeft: 8 + row.depth * 14 }}
              >
                <input
                  type="checkbox"
                  checked={state === "all"}
                  ref={(element) => {
                    if (element) element.indeterminate = state === "some";
                  }}
                  onChange={(event) =>
                    onToggleDirectory(row.path, event.target.checked, row.files)
                  }
                  aria-label={`Add folder ${row.path} — ${row.files.length} files, ${formatBytes(
                    row.bytes,
                  )}, currently ${state === "all" ? "all selected" : state === "some" ? `${hits} of ${row.files.length} selected` : "not selected"}`}
                  className="size-3.5 shrink-0 accent-brand"
                />
                <FolderIcon size={11} className="shrink-0 text-ink-500" />
                <span className="truncate text-ink-200">{name}/</span>
                <span className="ml-auto shrink-0 text-[11px] text-ink-500">
                  {state === "some" ? `${formatNumber(hits)}/` : ""}
                  {formatNumber(row.files.length)} · {formatBytes(row.bytes)}
                </span>
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
