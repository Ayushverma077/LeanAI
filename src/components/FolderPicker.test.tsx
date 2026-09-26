import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FolderPicker } from "./FolderPicker";
import type { FileEntry, Inventory } from "../ipc/types";

function file(path: string, sizeBytes = 100, selectable = true): FileEntry {
  return {
    path,
    sizeBytes,
    class: selectable ? "source_text" : "binary",
    selectable,
    exclusion: selectable ? null : { source: "LeanAiPolicy", reason: "Binary file type.", rule: null },
    contentHash: "h",
    modifiedMs: 0,
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
    file("src/index.ts", 200),
    file("src/api/handler.ts", 300),
    file("src/api/routes.ts", 400),
    file("assets/logo.png", 5000, false),
  ],
  issues: [],
  stats: { filesSeen: 5, directoriesSeen: 2, bytesSeen: 6000, elapsedMs: 1, truncated: false },
};

function setup(overrides: Partial<Parameters<typeof FolderPicker>[0]> = {}) {
  const props = {
    inventory,
    selected: new Set<string>(),
    search: "",
    onToggleDirectory: vi.fn(),
    ...overrides,
  };
  render(<FolderPicker {...props} />);
  return props;
}

describe("FolderPicker", () => {
  it("lists folders that hold selectable files, with rolled-up totals", () => {
    setup();
    // `src` counts its whole subtree: index.ts + api/handler.ts + api/routes.ts
    expect(screen.getByRole("checkbox", { name: /Add folder src — 3 files/ })).toBeInTheDocument();
    expect(
      screen.getByRole("checkbox", { name: /Add folder src\/api — 2 files/ }),
    ).toBeInTheDocument();
  });

  it("omits folders whose only files are excluded by policy", () => {
    setup();
    expect(screen.queryByRole("checkbox", { name: /Add folder assets/ })).not.toBeInTheDocument();
  });

  it("adds every file beneath a folder in one click", async () => {
    const user = userEvent.setup();
    const props = setup();

    await user.click(screen.getByRole("checkbox", { name: /Add folder src\/api/ }));
    expect(props.onToggleDirectory).toHaveBeenCalledWith("src/api", true, [
      "src/api/handler.ts",
      "src/api/routes.ts",
    ]);
  });

  it("shows a partially selected folder as indeterminate", () => {
    setup({ selected: new Set(["src/api/handler.ts"]) });
    const checkbox = screen.getByRole("checkbox", {
      name: /Add folder src\/api .*currently 1 of 2 selected/,
    }) as HTMLInputElement;
    expect(checkbox.indeterminate).toBe(true);
    expect(checkbox.checked).toBe(false);
  });

  it("marks a fully selected folder as checked", () => {
    setup({ selected: new Set(["src/api/handler.ts", "src/api/routes.ts"]) });
    const checkbox = screen.getByRole("checkbox", {
      name: /Add folder src\/api .*currently all selected/,
    }) as HTMLInputElement;
    expect(checkbox.checked).toBe(true);
  });

  it("filters folders by the shared search term", () => {
    setup({ search: "api" });
    expect(screen.getByRole("checkbox", { name: /Add folder src\/api/ })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /^Add folder src —/ })).not.toBeInTheDocument();
  });

  it("explains an empty result rather than rendering nothing", () => {
    setup({ search: "nothing-matches" });
    expect(screen.getByText("No folder matches that search.")).toBeInTheDocument();
  });
});
