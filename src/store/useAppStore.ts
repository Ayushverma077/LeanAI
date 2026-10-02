import { create } from "zustand";

import { api, toAppError } from "../ipc/client";
import { applyTheme, resolveInitialTheme, type Theme } from "../theme";
import type {
  AppError,
  SelectionRecipe,
  BuildBundleResponse,
  BundleOptions,
  ContextResponse,
  GitState,
  GitAuthStatus,
  GitRemoteStatus,
  GitPushRequest,
  GitPushResponse,
  CloneRepositoryRequest,
  GitHubRepository,
  Inventory,
  PolicyDescription,
  ProjectRecord,
  ScanProgress,
  SelectionSpec,
  Settings,
} from "../ipc/types";

export type Route =
  | "overview"
  | "context"
  | "tasks"
  | "history"
  | "models"
  | "settings"
  | "workspace"
  | "select"
  | "preview";

export interface SelectionState {
  files: Set<string>;
  directories: Set<string>;
  overrides: Set<string>;
  excluded: Set<string>;
}

const emptySelection = (): SelectionState => ({
  files: new Set(),
  directories: new Set(),
  overrides: new Set(),
  excluded: new Set(),
});

export function toSpec(selection: SelectionState): SelectionSpec {
  return {
    files: [...selection.files].sort(),
    directories: [...selection.directories].sort(),
    excluded: [...selection.excluded].sort(),
    overrides: [...selection.overrides].sort(),
  };
}

interface AppStore {
  route: Route;
  theme: Theme;
  project: ProjectRecord | null;
  git: GitState | null;
  remoteStatus: GitRemoteStatus | null;
  gitAuth: GitAuthStatus | null;
  githubRepos: GitHubRepository[];
  loadingGithubRepos: boolean;
  hasAiIgnore: boolean;
  inventory: Inventory | null;
  classCounts: Record<string, number>;
  scanning: boolean;
  /** True while the project map is being built after a project opens. */
  preparingMap: boolean;
  scanProgress: ScanProgress | null;
  selection: SelectionState;
  options: BundleOptions;
  bundle: BuildBundleResponse | null;
  building: boolean;
  context: ContextResponse | null;
  settings: Settings | null;
  policy: PolicyDescription | null;
  error: AppError | null;
  notice: string | null;

  setRoute: (route: Route) => void;
  setError: (error: AppError | null) => void;
  setNotice: (notice: string | null) => void;
  setScanProgress: (progress: ScanProgress | null) => void;

  bootstrap: () => Promise<void>;
  openProject: (path: string) => Promise<void>;
  closeProject: () => Promise<void>;
  /** `stayOnPage`: do not switch pages when the scan finishes (project open). */
  scan: (options?: { stayOnPage?: boolean }) => Promise<void>;
  cancelScan: () => Promise<void>;
  loadRemoteStatus: () => Promise<void>;
  loadGitAuth: () => Promise<void>;
  loadGithubRepos: () => Promise<void>;
  pushBranch: (request?: GitPushRequest) => Promise<GitPushResponse>;
  cloneRepository: (request: CloneRepositoryRequest) => Promise<void>;

  toggleFile: (path: string, selected: boolean) => void;
  toggleDirectory: (directory: string, selected: boolean, paths: string[]) => void;
  addOverride: (path: string) => void;
  selectPaths: (paths: string[], replace: boolean) => void;
  applyRecipe: (recipe: SelectionRecipe) => Promise<void>;
  clearSelection: () => void;

  setOptions: (options: Partial<BundleOptions>) => void;
  buildBundle: () => Promise<void>;

  loadContext: () => Promise<void>;
  /**
   * Generates the project map. Pass `true` only after the user chose to
   * replace an existing PROJECT_CONTEXT.md. Resolves to null on failure.
   */
  generateContext: (replaceExisting?: boolean) => Promise<ContextResponse | null>;
  /**
   * After a project opens: reuse its map if fresh, otherwise build it, then go
   * to Tasks. Moves only if the user is still on `startedOn`.
   */
  prepareMapAndContinue: (startedOn: Route) => Promise<void>;
  /** Regenerate on request, then go to Tasks (unless the file needs a decision). */
  regenerateMap: (replaceExisting?: boolean) => Promise<void>;

  saveSettings: (settings: Settings) => Promise<void>;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
}

export const useAppStore = create<AppStore>((set, get) => ({
  route: "overview",
  theme: resolveInitialTheme(),
  project: null,
  git: null,
  remoteStatus: null,
  gitAuth: null,
  githubRepos: [],
  loadingGithubRepos: false,
  hasAiIgnore: false,
  inventory: null,
  classCounts: {},
  scanning: false,
  preparingMap: false,
  scanProgress: null,
  selection: emptySelection(),
  options: {
    headers: true,
    lineNumbers: false,
    codeFences: false,
    fileSizeAnnotations: false,
    includeTree: true,
    includeFrontMatter: false,
    normalizeLineEndings: true,
    maxFileBytes: null,
    includeProjectMap: true,
  },
  bundle: null,
  building: false,
  context: null,
  settings: null,
  policy: null,
  error: null,
  notice: null,

  setRoute: (route) => set({ route }),
  setError: (error) => set({ error }),
  setNotice: (notice) => set({ notice }),
  setScanProgress: (scanProgress) => set({ scanProgress }),

  bootstrap: async () => {
    try {
      const [settings, policy] = await Promise.all([api.getSettings(), api.describePolicy()]);
      set({ settings, policy, options: settings.defaultBundleOptions });
      get()
        .loadGitAuth()
        .catch(() => undefined);
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  openProject: async (path) => {
    const startedOn = get().route;
    try {
      const response = await api.openProject(path);
      set({
        project: response.project,
        git: response.git,
        remoteStatus: null,
        hasAiIgnore: response.hasAiIgnore,
        inventory: null,
        bundle: null,
        context: null,
        selection: emptySelection(),
        error: null,
      });
      get()
        .loadRemoteStatus()
        .catch(() => undefined);
      await get().scan({ stayOnPage: true });
      // Opening a repository should not begin with an empty canvas and a
      // hundred checkboxes. Rescan deliberately does not do this.
      if (get().selection.files.size === 0) {
        await get().applyRecipe("source_only");
      }
      await get().prepareMapAndContinue(startedOn);
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  closeProject: async () => {
    await api.closeProject().catch(() => undefined);
    set({
      project: null,
      git: null,
      remoteStatus: null,
      inventory: null,
      bundle: null,
      context: null,
      selection: emptySelection(),
      route: "overview",
    });
  },

  scan: async (options) => {
    set({ scanning: true, error: null, scanProgress: null });
    try {
      const response = await api.scanProject();
      const currentRoute = get().route;
      // Strictly `true`: `scan` is also a click handler, and an event object
      // must not count as the option.
      const targetRoute =
        options?.stayOnPage !== true &&
        (currentRoute === "overview" || currentRoute === "workspace")
          ? "context"
          : currentRoute;
      set({
        inventory: response.inventory,
        classCounts: response.classCounts,
        scanning: false,
        route: targetRoute,
      });
      // A rescan can invalidate a stored selection; drop paths that vanished.
      const known = new Set(response.inventory.files.map((file) => file.path));
      const selection = get().selection;
      set({
        selection: {
          files: new Set([...selection.files].filter((path) => known.has(path))),
          directories: selection.directories,
          overrides: new Set([...selection.overrides].filter((path) => known.has(path))),
          excluded: new Set([...selection.excluded].filter((path) => known.has(path))),
        },
      });
    } catch (error) {
      const appError = toAppError(error);
      set({
        scanning: false,
        error: appError.code === "cancelled" ? null : appError,
        notice: appError.code === "cancelled" ? "Scan cancelled." : null,
      });
    }
  },

  cancelScan: async () => {
    await api.cancelScan().catch(() => undefined);
  },

  toggleFile: (path, selected) =>
    set((state) => {
      const files = new Set(state.selection.files);
      const excluded = new Set(state.selection.excluded);
      if (selected) {
        files.add(path);
        excluded.delete(path);
      } else {
        files.delete(path);
        // Remember the removal so a selected parent directory does not put it
        // straight back.
        if (state.selection.directories.size > 0) excluded.add(path);
      }
      return { selection: { ...state.selection, files, excluded }, bundle: null };
    }),

  toggleDirectory: (directory, selected, paths) =>
    set((state) => {
      const directories = new Set(state.selection.directories);
      const files = new Set(state.selection.files);
      const excluded = new Set(state.selection.excluded);
      if (selected) {
        directories.add(directory);
        for (const path of paths) {
          files.add(path);
          excluded.delete(path);
        }
      } else {
        directories.delete(directory);
        for (const path of paths) files.delete(path);
      }
      return { selection: { ...state.selection, directories, files, excluded }, bundle: null };
    }),

  addOverride: (path) =>
    set((state) => {
      const overrides = new Set(state.selection.overrides);
      const files = new Set(state.selection.files);
      overrides.add(path);
      files.add(path);
      return { selection: { ...state.selection, overrides, files }, bundle: null };
    }),

  selectPaths: (paths, replace) =>
    set((state) => {
      const files = replace ? new Set<string>() : new Set(state.selection.files);
      for (const path of paths) files.add(path);
      return { selection: { ...state.selection, files }, bundle: null };
    }),

  /**
   * Applies a one-click starting selection. The rule is evaluated in the Rust
   * core against the current scan, so it can only ever propose files the
   * safety policy already allows.
   */
  applyRecipe: async (recipe) => {
    try {
      const suggestion = await api.suggestSelection(recipe);
      get().selectPaths(suggestion.files, true);
      set({
        notice:
          `Selected ${suggestion.files.length} files — ${suggestion.description}` +
          (suggestion.skippedByRecipe > 0
            ? ` ${suggestion.skippedByRecipe} other selectable files were left out.`
            : ""),
        error: null,
      });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  clearSelection: () => set({ selection: emptySelection(), bundle: null }),

  setOptions: (partial) =>
    set((state) => ({ options: { ...state.options, ...partial }, bundle: null })),

  buildBundle: async () => {
    const { selection, options, inventory } = get();
    // With the project map on, there is something to show and copy even
    // before any file is picked, but only once a project is open.
    if (
      !inventory ||
      (selection.files.size === 0 && selection.directories.size === 0 && !options.includeProjectMap)
    ) {
      set({ bundle: null });
      return;
    }
    set({ building: true });
    try {
      const bundle = await api.buildBundle(toSpec(selection), options);
      set({ bundle, building: false, error: null });
    } catch (error) {
      set({ building: false, bundle: null, error: toAppError(error) });
    }
  },

  loadContext: async () => {
    try {
      const context = await api.loadContext();
      set({ context });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  generateContext: async (replaceExisting) => {
    try {
      // Strictly `true`: this is also used as a click handler, whose event
      // argument must never count as consent to replace a file.
      const context = await api.generateContext(replaceExisting === true);
      set({
        context,
        notice:
          context.file.state === "current"
            ? "PROJECT_CONTEXT.md saved in the project folder and excluded from Git."
            : "Project map updated in LeanAI. PROJECT_CONTEXT.md in the project was left unchanged.",
        error: null,
      });
      return context;
    } catch (error) {
      set({ error: toAppError(error) });
      return null;
    }
  },

  prepareMapAndContinue: async (startedOn) => {
    if (!get().inventory) return;
    set({ preparingMap: true });
    let context: ContextResponse | null = null;
    try {
      await get().loadContext();
      context = get().context;
      if (!context || context.freshness === "stale") {
        context = await get().generateContext();
      }
    } finally {
      set({ preparingMap: false });
    }
    // Never pull the user away from a page they moved to while this ran.
    if (get().route !== startedOn) return;
    if (!context) {
      // No map: land where opening a project used to.
      if (startedOn === "overview" || startedOn === "workspace") set({ route: "context" });
      return;
    }
    if (context.file.canReplace) {
      // A decision about the existing PROJECT_CONTEXT.md is waiting there.
      set({
        route: "context",
        notice: "This project already has a PROJECT_CONTEXT.md. Choose whether to replace it.",
      });
      return;
    }
    const name = get().project?.displayName ?? "this project";
    set({
      route: "tasks",
      notice:
        context.file.state === "current"
          ? `Project map ready. Ask LeanAI anything about ${name}.`
          : `Project map ready (kept inside LeanAI; PROJECT_CONTEXT.md in the project was left unchanged). Ask LeanAI anything about ${name}.`,
    });
  },

  regenerateMap: async (replaceExisting) => {
    const context = await get().generateContext(replaceExisting === true);
    if (context && !context.file.canReplace) set({ route: "tasks" });
  },

  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
  },

  toggleTheme: () => {
    const next: Theme = get().theme === "dark" ? "light" : "dark";
    applyTheme(next);
    set({ theme: next });
  },

  saveSettings: async (settings) => {
    try {
      const saved = await api.updateSettings(settings);
      set({ settings: saved, notice: "Settings saved." });
    } catch (error) {
      set({ error: toAppError(error) });
    }
  },

  loadRemoteStatus: async () => {
    try {
      const remoteStatus = await api.gitRemoteStatus();
      set({ remoteStatus });
    } catch {
      set({ remoteStatus: null });
    }
  },

  loadGitAuth: async () => {
    try {
      const gitAuth = await api.getGitAuthStatus();
      set({ gitAuth });
      if (gitAuth.githubTokenConfigured) {
        get()
          .loadGithubRepos()
          .catch(() => undefined);
      } else {
        set({ githubRepos: [] });
      }
    } catch {
      set({ gitAuth: null, githubRepos: [] });
    }
  },

  loadGithubRepos: async () => {
    set({ loadingGithubRepos: true });
    try {
      const githubRepos = await api.listGithubRepositories();
      set({ githubRepos, loadingGithubRepos: false });
    } catch {
      set({ loadingGithubRepos: false });
      // Keep any existing cached repos on error, don't crash
    }
  },

  pushBranch: async (request = {}) => {
    try {
      const res = await api.gitPushBranch(request);
      await get().loadRemoteStatus();
      set({ notice: res.message });
      return res;
    } catch (error) {
      const appErr = toAppError(error);
      set({ error: appErr });
      throw appErr;
    }
  },

  cloneRepository: async (request: CloneRepositoryRequest) => {
    const startedOn = get().route;
    try {
      const response = await api.cloneRemoteRepository(request);
      set({
        project: response.project,
        git: response.git,
        remoteStatus: null,
        hasAiIgnore: response.hasAiIgnore,
        inventory: null,
        bundle: null,
        context: null,
        selection: emptySelection(),
        error: null,
        notice: `Repository cloned into ${response.project.displayName}`,
      });
      get()
        .loadRemoteStatus()
        .catch(() => undefined);
      await get().scan({ stayOnPage: true });
      // Opening a repository should not begin with an empty canvas and a
      // hundred checkboxes. Rescan deliberately does not do this.
      if (get().selection.files.size === 0) {
        await get().applyRecipe("source_only");
      }
      await get().prepareMapAndContinue(startedOn);

      // Set last: `scan()` and the source recipe above both reset errors and
      // notices, and would otherwise wipe this. It is a persistent banner
      // rather than a toast because it describes a broken token that will make
      // the next private clone fail.
      if (response.authNotice) {
        set({
          error: {
            code: "github_token_rejected",
            message: response.authNotice,
            recovery: "Open Settings → Git and reconnect GitHub with a new token.",
            retryable: false,
          },
        });
      }
    } catch (error) {
      const appErr = toAppError(error);
      // Stopping a clone is the user's choice, not a failure worth a red banner.
      set(appErr.code === "clone_cancelled" ? { notice: "Clone stopped." } : { error: appErr });
      throw appErr;
    }
  },
}));
