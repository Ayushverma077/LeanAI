import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FileTree } from "./FileTree";
import type { FileEntry, Inventory } from "../ipc/types";

function file(path: string, overrides: Partial<FileEntry> = {}): FileEntry {
  return {
    path,
    sizeBytes: 100,
    class: "source_text",
    selectable: true,
    exclusion: null,
    contentHash: "abc",
    modifiedMs: 0,
    ...overrides,
  };
}

const inventory: Inventory = {
  root: "/tmp/project",
  projectFingerprint: "proj_test",
  sourceRevision: "scan:test",
  policyVersion: 1,
  scannedAtMs: 0,
  files: [
    file("README.md"),
    file("src/index.ts"),
    file("src/util/format.ts"),
    file(".env", {
      class: "credential_sensitive",
      selectable: false,
      exclusion: {
        source: "LeanAiPolicy",
        reason: "This path commonly stores credentials.",
        rule: ".env",
      },
    }),
  ],
  issues: [],
  stats: { filesSeen: 4, directoriesSeen: 2, bytesSeen: 400, elapsedMs: 1, truncated: false },
};

function setup(overrides: Partial<Parameters<typeof FileTree>[0]> = {}) {
  const props = {
    inventory,
    selected: new Set<string>(),
    overrides: new Set<string>(),
    search: "",
    onToggleFile: vi.fn(),
    onToggleDirectory: vi.fn(),
    onRequestOverride: vi.fn(),
    ...overrides,
  };
  render(<FileTree {...props} />);
  return props;
}

describe("FileTree", () => {
  it("renders a labelled tree with files and folders", () => {
    setup();
    expect(screen.getByRole("tree", { name: "Project files" })).toBeInTheDocument();
    expect(screen.getByText("README.md")).toBeInTheDocument();
    expect(screen.getByText("src/")).toBeInTheDocument();
  });

  it("blocks a credential-sensitive file and explains why in its accessible name", () => {
    setup();
    const checkbox = screen.getByRole("checkbox", { name: /\.env is excluded/ });
    expect(checkbox).toBeDisabled();
    expect(checkbox).toHaveAccessibleName(/commonly stores credentials/);
  });

  it("routes a blocked file through the override flow instead of selecting it", async () => {
    const user = userEvent.setup();
    const props = setup();
    await user.click(screen.getByRole("button", { name: /^Include \.env anyway, despite/ }));
    expect(props.onRequestOverride).toHaveBeenCalledWith(expect.objectContaining({ path: ".env" }));
    expect(props.onToggleFile).not.toHaveBeenCalled();
  });

  it("selects a file through its checkbox", async () => {
    const user = userEvent.setup();
    const props = setup();
    await user.click(screen.getByRole("checkbox", { name: /Select README\.md/ }));
    expect(props.onToggleFile).toHaveBeenCalledWith("README.md", true);
  });

  it("passes only selectable descendants when a folder is checked", async () => {
    const user = userEvent.setup();
    const props = setup();
    await user.click(screen.getByRole("checkbox", { name: /^Select folder src \(/ }));
    expect(props.onToggleDirectory).toHaveBeenCalledWith("src", true, [
      "src/index.ts",
      "src/util/format.ts",
    ]);
  });

  it("shows a folder as partially selected when only some children are chosen", () => {
    setup({ selected: new Set(["src/index.ts"]) });
    const checkbox = screen.getByRole("checkbox", {
      name: /^Select folder src \(.*currently partial/,
    }) as HTMLInputElement;
    expect(checkbox.indeterminate).toBe(true);
  });

  it("filters to matching paths when searching", () => {
    setup({ search: "format" });
    expect(screen.getByText("format.ts")).toBeInTheDocument();
    expect(screen.queryByText("README.md")).not.toBeInTheDocument();
  });

  it("orders rows depth-first with directories before files", () => {
    setup();
    const labels = screen
      .getAllByRole("treeitem")
      .map((row) => row.textContent?.replace(/[▸▾]/g, "").trim());
    expect(labels?.[0]).toMatch(/^src\//);
    expect(labels?.[1]).toMatch(/^util\//);
    expect(labels?.[2]).toMatch(/^format\.ts/);
    expect(labels?.[3]).toMatch(/^index\.ts/);
  });

  it("is keyboard navigable and toggles with the space key", async () => {
    const user = userEvent.setup();
    const props = setup();
    screen.getByRole("tree").focus();
    // Row order is src/, src/util/, format.ts, index.ts — so three moves down
    // lands on src/index.ts.
    await user.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}");
    await user.keyboard(" ");
    expect(props.onToggleFile).toHaveBeenCalledWith("src/index.ts", true);
  });

  it("toggles a folder with the space key without unlocking blocked files", async () => {
    const user = userEvent.setup();
    const props = setup();
    screen.getByRole("tree").focus();
    await user.keyboard(" ");
    expect(props.onToggleDirectory).toHaveBeenCalledWith("src", true, [
      "src/index.ts",
      "src/util/format.ts",
    ]);
  });

  it("announces each row's depth for screen readers", () => {
    setup();
    const rows = screen.getAllByRole("treeitem");
    const nested = rows.find((row) => within(row).queryByText("format.ts"));
    expect(nested).toHaveAttribute("aria-level", "3");
  });

  it("virtualises rendering for large inventories (10,000+ files) without bloating the DOM", () => {
    const largeFiles: FileEntry[] = [];
    const dirCount = 10;
    const filesPerDir = 1000;
    for (let d = 0; d < dirCount; d++) {
      for (let f = 0; f < filesPerDir; f++) {
        largeFiles.push(file(`pkg_${d}/mod_${f}.ts`));
      }
    }
    const largeInventory: Inventory = {
      ...inventory,
      files: largeFiles,
      stats: { ...inventory.stats, filesSeen: largeFiles.length },
    };

    const startTime = performance.now();
    setup({ inventory: largeInventory });
    const renderDuration = performance.now() - startTime;

    const renderedRows = screen.getAllByRole("treeitem");
    // With 10,000 items, virtual window + overscan should render fewer than 60 DOM rows
    expect(renderedRows.length).toBeLessThan(60);
    expect(renderedRows.length).toBeGreaterThan(0);
    expect(renderDuration).toBeLessThan(1000);
  });

  it("filters large inventories responsively when searching", () => {
    const largeFiles: FileEntry[] = [];
    for (let i = 0; i < 5000; i++) {
      largeFiles.push(file(`pkg/module_${i}.ts`));
    }
    largeFiles.push(file("pkg/needle_target.ts"));

    const largeInventory: Inventory = {
      ...inventory,
      files: largeFiles,
      stats: { ...inventory.stats, filesSeen: largeFiles.length },
    };

    setup({ inventory: largeInventory, search: "needle_target" });
    expect(screen.getByText("needle_target.ts")).toBeInTheDocument();
    expect(screen.queryByText("module_0.ts")).not.toBeInTheDocument();
  });
});
