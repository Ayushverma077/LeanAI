import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ContextResponse, Inventory } from "../ipc/types";

const api = {
  openProject: vi.fn(),
  gitRemoteStatus: vi.fn(),
  scanProject: vi.fn(),
  suggestSelection: vi.fn(),
  loadContext: vi.fn(),
  generateContext: vi.fn(),
};
vi.mock("../ipc/client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../ipc/client")>();
  return { ...actual, api: { ...actual.api, ...api } };
});

const { useAppStore } = await import("./useAppStore");

const inventory = {
  root: "/p",
  projectFingerprint: "proj",
  sourceRevision: "scan:1",
  policyVersion: 1,
  scannedAtMs: 0,
  files: [],
  issues: [],
  stats: { filesSeen: 0, directoriesSeen: 0, bytesSeen: 0, elapsedMs: 0, truncated: false },
} as Inventory;

function context(overrides: Partial<ContextResponse> = {}): ContextResponse {
  return {
    document: { sections: [] },
    markdown: "",
    freshness: "fresh",
    staleSectionKeys: [],
    file: { state: "current", alert: null, canReplace: false },
    ...overrides,
  } as unknown as ContextResponse;
}

describe("opening a project", () => {
  beforeEach(() => {
    Object.values(api).forEach((mock) => mock.mockReset());
    api.openProject.mockResolvedValue({
      project: { id: "p1", displayName: "demo" },
      git: null,
      hasAiIgnore: false,
    });
    api.gitRemoteStatus.mockResolvedValue(null);
    api.scanProject.mockResolvedValue({ inventory, classCounts: {}, selectableBytes: 0 });
    api.suggestSelection.mockResolvedValue({ files: [], description: "", skippedByRecipe: 0 });
    useAppStore.setState({ route: "overview", project: null, inventory: null, context: null });
  });

  it("builds the map when there is none, then opens Tasks", async () => {
    api.loadContext.mockResolvedValue(null);
    api.generateContext.mockResolvedValue(context());

    await useAppStore.getState().openProject("/p");

    expect(api.generateContext).toHaveBeenCalledWith(false);
    expect(useAppStore.getState().route).toBe("tasks");
    expect(useAppStore.getState().preparingMap).toBe(false);
    expect(useAppStore.getState().notice).toContain("Project map ready");
  });

  it("reuses a fresh map without rebuilding it", async () => {
    api.loadContext.mockResolvedValue(context());

    await useAppStore.getState().openProject("/p");

    expect(api.generateContext).not.toHaveBeenCalled();
    expect(useAppStore.getState().route).toBe("tasks");
  });

  it("rebuilds a stale map", async () => {
    api.loadContext.mockResolvedValue(context({ freshness: "stale" }));
    api.generateContext.mockResolvedValue(context());

    await useAppStore.getState().openProject("/p");

    expect(api.generateContext).toHaveBeenCalledTimes(1);
    expect(useAppStore.getState().route).toBe("tasks");
  });

  it("stays on Context when the existing PROJECT_CONTEXT.md needs a decision", async () => {
    api.loadContext.mockResolvedValue(null);
    api.generateContext.mockResolvedValue(
      context({ file: { state: "foreign", alert: "exists", canReplace: true } }),
    );

    await useAppStore.getState().openProject("/p");

    expect(useAppStore.getState().route).toBe("context");
    expect(useAppStore.getState().notice).toContain("Choose whether to replace it");
  });

  it("does not pull the user away from a page they moved to meanwhile", async () => {
    api.loadContext.mockImplementation(async () => {
      useAppStore.getState().setRoute("models");
      return context();
    });

    await useAppStore.getState().openProject("/p");

    expect(useAppStore.getState().route).toBe("models");
  });

  it("falls back to the Context page when the map cannot be built", async () => {
    api.loadContext.mockResolvedValue(null);
    api.generateContext.mockRejectedValue({ code: "io", message: "disk full", retryable: false });

    await useAppStore.getState().openProject("/p");

    expect(useAppStore.getState().route).toBe("context");
    expect(useAppStore.getState().error?.message).toBe("disk full");
  });
});
