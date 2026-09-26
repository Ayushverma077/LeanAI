import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { FileEntry, Inventory } from "../ipc/types";
import { Chip, formatBytes } from "./primitives";

const ROW_HEIGHT = 26;
const OVERSCAN = 12;

interface DirectoryNode {
  kind: "directory";
  path: string;
  name: string;
  depth: number;
  children: TreeNode[];
  /** Selectable files at or below this directory. */
  selectablePaths: string[];
}

interface FileNode {
  kind: "file";
  path: string;
  name: string;
  depth: number;
  entry: FileEntry;
}

type TreeNode = DirectoryNode | FileNode;

function buildTree(files: FileEntry[]): TreeNode[] {
  const roots: TreeNode[] = [];
  const directories = new Map<string, DirectoryNode>();

  const ensureDirectory = (path: string, depth: number): DirectoryNode => {
    const existing = directories.get(path);
    if (existing) return existing;
    const name = path.includes("/") ? path.slice(path.lastIndexOf("/") + 1) : path;
    const node: DirectoryNode = {
      kind: "directory",
      path,
      name,
      depth,
      children: [],
      selectablePaths: [],
    };
    directories.set(path, node);
    if (path.includes("/")) {
      const parentPath = path.slice(0, path.lastIndexOf("/"));
      ensureDirectory(parentPath, depth - 1).children.push(node);
    } else {
      roots.push(node);
    }
    return node;
  };

  for (const entry of files) {
    const segments = entry.path.split("/");
    const depth = segments.length - 1;
    const node: FileNode = {
      kind: "file",
      path: entry.path,
      name: segments[segments.length - 1] ?? entry.path,
      depth,
      entry,
    };
    if (depth === 0) {
      roots.push(node);
    } else {
      const parentPath = segments.slice(0, -1).join("/");
      ensureDirectory(parentPath, depth - 1).children.push(node);
      // Record the selectable file against every ancestor.
      for (let index = 1; index <= depth; index += 1) {
        const ancestor = directories.get(segments.slice(0, index).join("/"));
        if (ancestor && entry.selectable) ancestor.selectablePaths.push(entry.path);
      }
    }
  }

  const sortNodes = (nodes: TreeNode[]): TreeNode[] => {
    nodes.sort((a, b) => {
      if (a.kind !== b.kind) return a.kind === "directory" ? -1 : 1;
      return a.name.localeCompare(b.name);
    });
    for (const node of nodes) if (node.kind === "directory") sortNodes(node.children);
    return nodes;
  };
  return sortNodes(roots);
}

export function classChip(entry: FileEntry) {
  switch (entry.class) {
    case "source_text":
      return null;
    case "credential_sensitive":
      return (
        <Chip tone="danger" title={entry.exclusion?.reason} className="shrink-0">
          ⚠ credential
        </Chip>
      );
    case "too_large":
      return (
        <Chip tone="warn" title={entry.exclusion?.reason} className="shrink-0">
          too large
        </Chip>
      );
    case "binary":
      return <Chip title={entry.exclusion?.reason} className="shrink-0">binary</Chip>;
    case "lockfile":
      return <Chip title={entry.exclusion?.reason} className="shrink-0">lockfile</Chip>;
    case "generated":
      return <Chip title={entry.exclusion?.reason} className="shrink-0">generated</Chip>;
    case "hidden_metadata":
      return <Chip title={entry.exclusion?.reason} className="shrink-0">hidden</Chip>;
    case "symlink":
      return (
        <Chip tone="warn" title={entry.exclusion?.reason} className="shrink-0">
          symlink
        </Chip>
      );
    case "unsupported_encoding":
      return (
        <Chip tone="warn" title={entry.exclusion?.reason} className="shrink-0">
          encoding
        </Chip>
      );
    case "unreadable":
      return (
        <Chip tone="danger" title={entry.exclusion?.reason} className="shrink-0">
          unreadable
        </Chip>
      );
  }
}

interface FileTreeProps {
  inventory: Inventory;
  selected: Set<string>;
  overrides: Set<string>;
  search: string;
  onToggleFile: (path: string, selected: boolean) => void;
  onToggleDirectory: (directory: string, selected: boolean, paths: string[]) => void;
  onRequestOverride: (entry: FileEntry) => void;
}

/**
 * Virtualised, keyboard-navigable file tree.
 *
 * Only the visible window is rendered, so a monorepo with tens of thousands of
 * files scrolls without the UI stalling (NFR 7.4). Rows are a single ARIA
 * tree, so screen readers announce depth, selection state and the reason a
 * file is not selectable.
 */
export function FileTree({
  inventory,
  selected,
  overrides,
  search,
  onToggleFile,
  onToggleDirectory,
  onRequestOverride,
}: FileTreeProps) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [activeIndex, setActiveIndex] = useState(0);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(480);
  const viewportRef = useRef<HTMLDivElement>(null);

  const tree = useMemo(() => buildTree(inventory.files), [inventory.files]);

  const matches = useMemo(() => {
    const term = search.trim().toLowerCase();
    if (!term) return null;
    return new Set(
      inventory.files
        .filter((file) => file.path.toLowerCase().includes(term))
        .map((file) => file.path),
    );
  }, [inventory.files, search]);

  /** Flattens the tree into the rows that are currently visible. */
  const rows = useMemo(() => {
    const output: TreeNode[] = [];
    const walk = (nodes: TreeNode[]) => {
      for (const node of nodes) {
        if (node.kind === "file") {
          if (!matches || matches.has(node.path)) output.push(node);
          continue;
        }
        const visibleDescendant =
          !matches || node.children.some((child) => hasMatch(child, matches));
        if (!visibleDescendant) continue;
        output.push(node);
        // A search expands the path to its hits; otherwise honour the user's
        // collapse state.
        if (matches || !collapsed.has(node.path)) walk(node.children);
      }
    };
    walk(tree);
    return output;
  }, [tree, collapsed, matches]);

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setViewportHeight(element.clientHeight));
    observer.observe(element);
    setViewportHeight(element.clientHeight);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(rows.length - 1, 0)));
  }, [rows.length]);

  const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const end = Math.min(
    rows.length,
    Math.ceil((scrollTop + viewportHeight) / ROW_HEIGHT) + OVERSCAN,
  );
  const window = rows.slice(start, end);

  const directoryState = useCallback(
    (node: DirectoryNode): "checked" | "partial" | "unchecked" => {
      if (node.selectablePaths.length === 0) return "unchecked";
      let hits = 0;
      for (const path of node.selectablePaths) if (selected.has(path)) hits += 1;
      if (hits === 0) return "unchecked";
      return hits === node.selectablePaths.length ? "checked" : "partial";
    },
    [selected],
  );

  const toggleCollapse = useCallback((path: string) => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    const node = rows[activeIndex];
    if (!node) return;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        setActiveIndex((index) => Math.min(index + 1, rows.length - 1));
        break;
      case "ArrowUp":
        event.preventDefault();
        setActiveIndex((index) => Math.max(index - 1, 0));
        break;
      case "ArrowRight":
        if (node.kind === "directory" && collapsed.has(node.path)) {
          event.preventDefault();
          toggleCollapse(node.path);
        }
        break;
      case "ArrowLeft":
        if (node.kind === "directory" && !collapsed.has(node.path)) {
          event.preventDefault();
          toggleCollapse(node.path);
        }
        break;
      case " ":
      case "Enter": {
        event.preventDefault();
        if (node.kind === "directory") {
          const state = directoryState(node);
          onToggleDirectory(node.path, state !== "checked", node.selectablePaths);
        } else if (node.entry.selectable || overrides.has(node.path)) {
          onToggleFile(node.path, !selected.has(node.path));
        } else {
          onRequestOverride(node.entry);
        }
        break;
      }
      case "Home":
        event.preventDefault();
        setActiveIndex(0);
        break;
      case "End":
        event.preventDefault();
        setActiveIndex(rows.length - 1);
        break;
      default:
        return;
    }
  };

  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return;
    const top = activeIndex * ROW_HEIGHT;
    if (top < element.scrollTop) element.scrollTop = top;
    else if (top + ROW_HEIGHT > element.scrollTop + element.clientHeight) {
      element.scrollTop = top + ROW_HEIGHT - element.clientHeight;
    }
  }, [activeIndex]);

  if (rows.length === 0) {
    return (
      <p className="px-3 py-8 text-center text-xs text-ink-500">
        {matches ? "No file matches that search." : "This project has no scannable files."}
      </p>
    );
  }

  return (
    <div
      ref={viewportRef}
      role="tree"
      aria-label="Project files"
      tabIndex={0}
      onKeyDown={onKeyDown}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
      className="h-[60vh] overflow-auto rounded border border-ink-800 bg-ink-950"
      data-testid="file-tree"
    >
      <div style={{ height: rows.length * ROW_HEIGHT, position: "relative" }}>
        {window.map((node, offset) => {
          const index = start + offset;
          const isActive = index === activeIndex;
          const common = {
            role: "treeitem" as const,
            "aria-level": node.depth + 1,
            "aria-setsize": rows.length,
            "aria-posinset": index + 1,
            style: {
              position: "absolute" as const,
              top: index * ROW_HEIGHT,
              height: ROW_HEIGHT,
              left: 0,
              right: 0,
              paddingLeft: 8 + node.depth * 14,
            },
            className: `flex items-center gap-2 pr-3 text-xs ${
              isActive ? "bg-ink-800" : "hover:bg-ink-900"
            }`,
            onMouseDown: () => setActiveIndex(index),
          };

          if (node.kind === "directory") {
            const state = directoryState(node);
            const isCollapsed = collapsed.has(node.path) && !matches;
            return (
              <div key={node.path} {...common} aria-expanded={!isCollapsed}>
                <button
                  type="button"
                  aria-label={`${isCollapsed ? "Expand" : "Collapse"} ${node.path}`}
                  onClick={() => toggleCollapse(node.path)}
                  className="w-3 shrink-0 text-ink-500"
                  tabIndex={-1}
                >
                  {isCollapsed ? "▸" : "▾"}
                </button>
                <input
                  type="checkbox"
                  checked={state === "checked"}
                  ref={(element) => {
                    if (element) element.indeterminate = state === "partial";
                  }}
                  aria-label={`Select folder ${node.path} (${node.selectablePaths.length} selectable files, currently ${state})`}
                  disabled={node.selectablePaths.length === 0}
                  onChange={(event) =>
                    onToggleDirectory(node.path, event.target.checked, node.selectablePaths)
                  }
                  tabIndex={-1}
                  className="size-3.5 rounded accent-ink-100"
                />
                <span className="truncate font-medium text-ink-300">{node.name}/</span>
                <span className="text-[11px] text-ink-500">{node.selectablePaths.length}</span>
              </div>
            );
          }

          const entry = node.entry;
          const unlocked = entry.selectable || overrides.has(entry.path);
          return (
            <div key={node.path} {...common}>
              <span className="w-3 shrink-0" />
              <input
                type="checkbox"
                checked={selected.has(entry.path)}
                disabled={!unlocked}
                aria-label={
                  unlocked
                    ? `Select ${entry.path}, ${formatBytes(entry.sizeBytes)}`
                    : `${entry.path} is excluded: ${entry.exclusion?.reason ?? entry.class}`
                }
                onChange={(event) => onToggleFile(entry.path, event.target.checked)}
                tabIndex={-1}
                className="size-3.5 rounded accent-ink-100"
              />
              <span className={`truncate ${unlocked ? "text-ink-100" : "text-ink-500"}`}>
                {node.name}
              </span>
              {classChip(entry)}
              {!unlocked &&
              entry.class !== "unreadable" &&
              entry.class !== "unsupported_encoding" ? (
                <button
                  type="button"
                  onClick={() => onRequestOverride(entry)}
                  /* `shrink-0` + `whitespace-nowrap`: without them this wraps onto
                     a second line and overflows the fixed-height virtualised row.
                     `hover:text-white` also disappeared on a light background. */
                  className="shrink-0 whitespace-nowrap text-[11px] text-ink-400 underline decoration-dotted transition-colors hover:text-ink-100"
                  tabIndex={-1}
                  /* The visible word is short to fit the row; the label carries
                     the file and the reason, which is what a screen reader
                     needs to make sense of it. */
                  aria-label={`Include ${entry.path} anyway, despite: ${
                    entry.exclusion?.reason ?? entry.class
                  }`}
                  title={`Include ${entry.path} despite: ${entry.exclusion?.reason ?? entry.class}`}
                >
                  include
                </button>
              ) : null}
              <span className="ml-auto shrink-0 text-[11px] text-ink-500">
                {formatBytes(entry.sizeBytes)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function hasMatch(node: TreeNode, matches: Set<string>): boolean {
  if (node.kind === "file") return matches.has(node.path);
  return node.children.some((child) => hasMatch(child, matches));
}
