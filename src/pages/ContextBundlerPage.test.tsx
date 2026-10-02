import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BuildBundleResponse, ContextDocument, Inventory } from "../ipc/types";

const buildBundle = vi.fn();
const exportPreflight = vi.fn();
const listPresets = vi.fn();
const generateContext = vi.fn();
vi.mock("../ipc/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/client")>();
  return {
    ...actual,
    api: { ...actual.api, buildBundle, exportPreflight, listPresets, generateContext },
  };
});
vi.mock("@tauri-apps/plugin-clipboard-manager", () => ({ writeText: vi.fn() }));

const { ContextBundlerPage } = await import("./ContextBundlerPage");
const { useAppStore } = await import("../store/useAppStore");

const inventory: Inventory = {
  root: "/tmp/project",
  projectFingerprint: "proj_test",
  sourceRevision: "scan:test",
  policyVersion: 1,
  scannedAtMs: 0,
  files: [
    {
      path: "src/index.ts",
      sizeBytes: 100,
      class: "source_text",
      selectable: true,
      exclusion: null,
      contentHash: "abc",
      modifiedMs: 0,
    },
  ],
  issues: [],
  stats: { filesSeen: 1, directoriesSeen: 1, bytesSeen: 100, elapsedMs: 1, truncated: false },
};

const map: ContextDocument = {
  schemaVersion: 2,
  projectFingerprint: "proj_test",
  projectName: "demo",
  sourceRevision: "scan:test",
  generatedAtMs: 0,
  contentHash: "h",
  sections: [
    {
      key: "overview",
      title: "Overview",
      body: "- **Languages:** typescript 1\n- **Entry points:** `src/index.ts`\n",
      sourceRefs: [{ path: "src/index.ts", contentHash: "abc", lines: null }],
      freshness: "fresh",
      generator: { generator: "deterministic" },
      limitations: ["Entry points are matched by conventional file names."],
      generatedRevision: "scan:test",
      generatedAtMs: 0,
    },
  ],
};

function response(overrides: Partial<BuildBundleResponse> = {}): BuildBundleResponse {
  return {
    resolved: { files: [], rejected: [], totalBytes: 0 },
    preview: "",
    previewTruncated: false,
    projectMapTokens: 42,
    projectMap: map,
    projectMapFile: { state: "current", alert: null, canReplace: false },
    outputHash: "0123456789abcdef0123",
    estimate: { value: 42, kind: "local_estimate" },
    estimateLabel: "estimate · cl100k_base · OpenAI-family",
    contributions: [],
    truncations: [],
    skipped: [],
    byteLen: 180,
    fileCount: 0,
    manifest: {},
    ...overrides,
  } as unknown as BuildBundleResponse;
}

describe("ContextBundlerPage", () => {
  beforeEach(() => {
    buildBundle.mockReset().mockResolvedValue(response());
    exportPreflight.mockReset();
    listPresets.mockReset().mockResolvedValue([]);
    generateContext.mockReset();
    act(() => {
      useAppStore.setState({ inventory, bundle: null, context: null });
      useAppStore.getState().clearSelection();
      useAppStore.getState().setOptions({ includeProjectMap: true });
    });
  });

  it("shows the project map and the files in one place, with one copy", async () => {
    render(<ContextBundlerPage />);

    // The map is rendered as readable text, not raw Markdown.
    expect(await screen.findByText("Languages:")).toHaveProperty("tagName", "STRONG");
    expect(screen.getByRole("region", { name: "Project map" })).toHaveTextContent(
      "PROJECT_CONTEXT.md",
    );
    expect(screen.getByRole("region", { name: "Selected files" })).toHaveTextContent(
      "Copy sends just the project map",
    );
    expect(screen.queryByText(/\*\*/)).toBeNull();

    // With only the map, Copy is available and the summary says what goes out.
    expect(screen.getByRole("button", { name: /copy/i })).toBeEnabled();
    // Plain words: what is sent, how big it is, and whether that is a problem.
    expect(screen.getByRole("heading", { name: "What the AI gets" })).toBeInTheDocument();
    expect(screen.getByText("Small")).toBeInTheDocument();
    expect(screen.getByText(/fits in almost any AI chat/)).toBeInTheDocument();
    // The exact, labelled estimate stays one hover away (ADR 0004).
    expect(screen.getByText("(~42 tokens)")).toHaveAttribute(
      "title",
      expect.stringContaining("estimate · cl100k_base · OpenAI-family"),
    );
    expect(screen.getByRole("button", { name: /copy for ai/i })).toBeEnabled();
    expect(screen.getByRole("region", { name: "Project map" })).toHaveTextContent("Up to date");
    expect(buildBundle).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({ includeProjectMap: true }),
    );
  });

  it("builds once per change instead of looping", async () => {
    render(<ContextBundlerPage />);
    await screen.findByText("Languages:");
    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(buildBundle).toHaveBeenCalledTimes(1);
  });

  it("can leave the map out", async () => {
    render(<ContextBundlerPage />);
    await screen.findByText("Languages:");
    buildBundle.mockResolvedValue(response({ projectMap: null, projectMapTokens: 0 }));

    await userEvent.click(screen.getByLabelText("Include project map"));

    await waitFor(() => expect(screen.getByText(/The project map is off/)).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /copy/i })).toBeDisabled();
  });

  it("alerts when the project already has its own PROJECT_CONTEXT.md, and replaces it only on request", async () => {
    const alert =
      "This project already has a PROJECT_CONTEXT.md that LeanAI did not create. LeanAI left it as it is and keeps its own map inside the app.";
    buildBundle.mockResolvedValue(
      response({ projectMapFile: { state: "foreign", alert, canReplace: true } }),
    );
    generateContext.mockResolvedValue({
      document: map,
      markdown: "",
      freshness: "fresh",
      staleSectionKeys: [],
      file: { state: "current", alert: null, canReplace: false },
    });
    render(<ContextBundlerPage />);

    const banner = await screen.findByRole("alert");
    expect(banner).toHaveTextContent("PROJECT_CONTEXT.md already exists in this project");
    expect(banner).toHaveTextContent("LeanAI did not create");

    // Replacing takes two deliberate clicks.
    await userEvent.click(screen.getByRole("button", { name: "Replace file" }));
    expect(generateContext).not.toHaveBeenCalled();
    buildBundle.mockResolvedValue(response());
    await userEvent.click(screen.getByRole("button", { name: "Yes, replace it" }));
    expect(generateContext).toHaveBeenCalledWith(true);
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(useAppStore.getState().route).toBe("tasks");
  });

  it("opens Tasks once a requested regeneration finishes", async () => {
    generateContext.mockResolvedValue({
      document: map,
      markdown: "",
      freshness: "fresh",
      staleSectionKeys: [],
      file: { state: "current", alert: null, canReplace: false },
    });
    act(() => useAppStore.setState({ route: "context" }));
    render(<ContextBundlerPage />);
    await screen.findByText("Languages:");

    await userEvent.click(screen.getByRole("button", { name: /refresh/i }));

    await waitFor(() => expect(useAppStore.getState().route).toBe("tasks"));
    expect(generateContext).toHaveBeenCalledWith(false);
  });

  it("keeps the user's file when they choose to", async () => {
    buildBundle.mockResolvedValue(
      response({
        projectMapFile: {
          state: "edited",
          alert: "not the one LeanAI last wrote",
          canReplace: true,
        },
      }),
    );
    render(<ContextBundlerPage />);
    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("button", { name: "Keep mine" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(generateContext).not.toHaveBeenCalled();
  });

  it("explains a tracked file without offering to replace it", async () => {
    buildBundle.mockResolvedValue(
      response({
        projectMapFile: {
          state: "tracked_by_git",
          alert: "PROJECT_CONTEXT.md is tracked by Git, so LeanAI will not write it.",
          canReplace: false,
        },
      }),
    );
    render(<ContextBundlerPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "LeanAI can't write PROJECT_CONTEXT.md",
    );
    expect(screen.queryByRole("button", { name: "Replace file" })).toBeNull();
  });

  it("says in words when the copy is getting too big", async () => {
    buildBundle.mockResolvedValue(
      response({ fileCount: 54, estimate: { value: 96_217, kind: "local_estimate" } } as never),
    );
    const { unmount } = render(<ContextBundlerPage />);
    expect(await screen.findByText("Medium")).toBeInTheDocument();
    expect(screen.getByText("(~96k tokens)")).toBeInTheDocument();
    expect(screen.getByText("Project map + 54 files")).toBeInTheDocument();
    unmount();

    buildBundle.mockResolvedValue(
      response({ fileCount: 300, estimate: { value: 250_000, kind: "local_estimate" } } as never),
    );
    render(<ContextBundlerPage />);
    expect(await screen.findByText("Large")).toBeInTheDocument();
    expect(screen.getByText(/Pick fewer files/)).toBeInTheDocument();
  });
});
