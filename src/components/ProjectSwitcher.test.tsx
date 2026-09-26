import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ProjectSwitcher } from "./ProjectSwitcher";
import { api } from "../ipc/client";
import { useAppStore } from "../store/useAppStore";
import type { ProjectRecord } from "../ipc/types";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const project = (id: string, displayName: string, canonicalPath: string): ProjectRecord => ({
  id,
  fingerprint: `proj_${id}`,
  canonicalPath,
  displayName,
  lastScanRevision: "git:abc",
  lastOpenedAtMs: 0,
  policyVersion: 1,
});

const current = project("1", "LeanAI", "/Users/dev/LeanAI");
const other = project("2", "acme-web", "/Users/dev/acme-web");

describe("ProjectSwitcher", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(api, "listProjects").mockResolvedValue([current, other]);
    useAppStore.setState({
      project: current,
      route: "overview",
      openProject: vi.fn().mockResolvedValue(undefined),
      closeProject: vi.fn().mockResolvedValue(undefined),
      setRoute: vi.fn(),
    });
  });

  const setup = () => {
    const onCloneRequest = vi.fn();
    render(<ProjectSwitcher onCloneRequest={onCloneRequest} />);
    return { onCloneRequest };
  };

  it("shows the open repository and does not fetch the list until opened", () => {
    setup();
    expect(screen.getByRole("heading", { name: "LeanAI" })).toBeInTheDocument();
    expect(api.listProjects).not.toHaveBeenCalled();
  });

  it("lists the other repositories and marks the current one", async () => {
    const user = userEvent.setup();
    setup();

    await user.click(screen.getByRole("button", { name: /LeanAI/ }));
    const menu = await screen.findByRole("menu", { name: "Switch repository" });

    expect(await screen.findByRole("menuitem", { name: /acme-web/ })).toBeInTheDocument();
    expect(within(menu).getByText("current")).toBeInTheDocument();
  });

  it("switches repository and stays on Overview", async () => {
    const user = userEvent.setup();
    setup();

    await user.click(screen.getByRole("button", { name: /LeanAI/ }));
    await user.click(await screen.findByRole("menuitem", { name: /acme-web/ }));

    const state = useAppStore.getState();
    await waitFor(() => expect(state.openProject).toHaveBeenCalledWith("/Users/dev/acme-web"));
    // `scan()` would otherwise leave the user in the Context Studio.
    await waitFor(() => expect(state.setRoute).toHaveBeenCalledWith("overview"));
  });

  it("offers closing the repository and cloning a new one", async () => {
    const user = userEvent.setup();
    const { onCloneRequest } = setup();

    await user.click(screen.getByRole("button", { name: /LeanAI/ }));
    await user.click(await screen.findByRole("menuitem", { name: "Clone from GitHub…" }));
    expect(onCloneRequest).toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /LeanAI/ }));
    await user.click(await screen.findByRole("menuitem", { name: "Close repository" }));
    expect(useAppStore.getState().closeProject).toHaveBeenCalled();
  });

  it("closes on Escape and returns focus to the trigger", async () => {
    const user = userEvent.setup();
    setup();

    const trigger = screen.getByRole("button", { name: /LeanAI/ });
    await user.click(trigger);
    expect(await screen.findByRole("menu")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
  });

  it("renders nothing when no repository is open", () => {
    useAppStore.setState({ project: null });
    const { container } = render(<ProjectSwitcher onCloneRequest={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });
});
