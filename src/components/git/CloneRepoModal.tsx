import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";

import { useAppStore } from "../../store/useAppStore";
import { Button, Chip, Field, Modal } from "../primitives";
import {
  DownloadCloudIcon,
  GithubIcon,
  KeyIcon,
  LockIcon,
  RefreshCwIcon,
  SearchIcon,
  StarIcon,
} from "../icons";
import type { GitHubRepository } from "../../ipc/types";

const LANGUAGE_COLORS: Record<string, string> = {
  TypeScript: "bg-blue-400",
  JavaScript: "bg-yellow-400",
  Rust: "bg-amber-600",
  Python: "bg-emerald-400",
  Go: "bg-cyan-400",
  HTML: "bg-orange-500",
  CSS: "bg-pink-400",
  Java: "bg-red-400",
  "C++": "bg-indigo-400",
  C: "bg-slate-400",
  Shell: "bg-teal-400",
};

function formatRepoDate(isoDate: string): string {
  if (!isoDate) return "";
  try {
    const d = new Date(isoDate);
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  } catch {
    return isoDate;
  }
}

interface CloneRepoModalProps {
  open: boolean;
  onClose: () => void;
  initialRepo?: GitHubRepository | null;
}

export function CloneRepoModal({ open: isOpen, onClose, initialRepo }: CloneRepoModalProps) {
  const { gitAuth, githubRepos, loadingGithubRepos, loadGithubRepos, cloneRepository } =
    useAppStore();

  const isGitHubConfigured = Boolean(gitAuth?.githubTokenConfigured);

  const [activeTab, setActiveTab] = useState<"github" | "custom">("github");
  const [selectedRepo, setSelectedRepo] = useState<GitHubRepository | null>(null);
  const [repoSearch, setRepoSearch] = useState("");
  const [filterVisibility, setFilterVisibility] = useState<"all" | "public" | "private">("all");

  const [url, setUrl] = useState("");
  const [destinationParentDir, setDestinationParentDir] = useState("");
  const [directoryName, setDirectoryName] = useState("");
  const [cloning, setCloning] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Sync tab and initial selection when opened
  useEffect(() => {
    if (!isOpen) return;

    if (initialRepo) {
      setSelectedRepo(initialRepo);
      setDirectoryName(initialRepo.name);
      const preferSsh = gitAuth?.sshAuthenticated && initialRepo.sshUrl;
      setUrl(preferSsh ? initialRepo.sshUrl : initialRepo.cloneUrl);
      setActiveTab("github");
    } else if (isGitHubConfigured) {
      setActiveTab("github");
    } else {
      setActiveTab("custom");
    }

    if (isGitHubConfigured && githubRepos.length === 0 && !loadingGithubRepos) {
      loadGithubRepos().catch(() => undefined);
    }
  }, [
    isOpen,
    initialRepo,
    isGitHubConfigured,
    githubRepos.length,
    loadingGithubRepos,
    loadGithubRepos,
    gitAuth?.sshAuthenticated,
  ]);

  const handleSelectRepo = (repo: GitHubRepository) => {
    setSelectedRepo(repo);
    setDirectoryName(repo.name);
    const preferSsh = gitAuth?.sshAuthenticated && repo.sshUrl;
    setUrl(preferSsh ? repo.sshUrl : repo.cloneUrl);
    setErrorMsg(null);
  };

  const filteredRepos = useMemo(() => {
    return githubRepos.filter((repo) => {
      if (filterVisibility === "public" && repo.isPrivate) return false;
      if (filterVisibility === "private" && !repo.isPrivate) return false;
      if (!repoSearch.trim()) return true;
      const q = repoSearch.toLowerCase().trim();
      return (
        repo.name.toLowerCase().includes(q) ||
        repo.fullName.toLowerCase().includes(q) ||
        (repo.description && repo.description.toLowerCase().includes(q)) ||
        (repo.language && repo.language.toLowerCase().includes(q))
      );
    });
  }, [githubRepos, filterVisibility, repoSearch]);

  if (!isOpen) return null;

  const handleUrlChange = (value: string) => {
    setUrl(value);
    const trimmed = value.trim();
    if (trimmed) {
      const parts = trimmed.replace(/\.git$/, "").split(/[/:]/);
      const last = parts[parts.length - 1];
      if (last && (!directoryName || selectedRepo?.name === directoryName)) {
        setDirectoryName(last);
      }
    }
  };

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select Destination Directory",
    });
    if (typeof selected === "string") {
      setDestinationParentDir(selected);
    }
  };

  const handleClone = async () => {
    if (!url.trim() || !destinationParentDir.trim()) {
      setErrorMsg("Please select or enter a repository and specify the destination directory.");
      return;
    }

    setCloning(true);
    setErrorMsg(null);
    try {
      await cloneRepository({
        url: url.trim(),
        destinationParentDir: destinationParentDir.trim(),
        directoryName: directoryName.trim() || undefined,
      });
      onClose();
    } catch (e: unknown) {
      const msg =
        typeof e === "object" && e && "message" in e
          ? String((e as { message: string }).message)
          : "Failed to clone repository.";
      setErrorMsg(msg);
    } finally {
      setCloning(false);
    }
  };

  const isSsh = url.startsWith("git@") || url.startsWith("ssh://");
  const isHttps = url.startsWith("https://") || url.startsWith("http://");

  return (
    <Modal
      open={isOpen}
      onClose={onClose}
      title="Clone Remote Repository"
      description="Clone a Git repository from GitHub or any remote host and open it in LeanAI Desktop."
      className="max-w-2xl"
    >
      <div className="space-y-4">
        {/* Source Tab Switcher */}
        <div className="flex border-b border-ink-800">
          <button
            type="button"
            onClick={() => setActiveTab("github")}
            className={`flex items-center gap-2 border-b-2 px-3 py-2 text-xs font-medium transition-colors ${
              activeTab === "github"
                ? "border-accent text-ink-100"
                : "border-transparent text-ink-400 hover:text-ink-200"
            }`}
          >
            <GithubIcon size={14} />
            <span>Your GitHub Repositories</span>
            {isGitHubConfigured && githubRepos.length > 0 && (
              <span className="rounded-full bg-ink-800 px-1.5 py-0.2 text-[10px] text-ink-300">
                {githubRepos.length}
              </span>
            )}
          </button>
          <button
            type="button"
            onClick={() => setActiveTab("custom")}
            className={`flex items-center gap-2 border-b-2 px-3 py-2 text-xs font-medium transition-colors ${
              activeTab === "custom"
                ? "border-accent text-ink-100"
                : "border-transparent text-ink-400 hover:text-ink-200"
            }`}
          >
            <span>Custom Git URL</span>
          </button>
        </div>

        {/* Tab 1: GitHub Repositories */}
        {activeTab === "github" && (
          <div className="space-y-3">
            {!isGitHubConfigured ? (
              <div className="rounded-lg border border-ink-800 bg-ink-950/60 p-4 text-center">
                <GithubIcon size={24} className="mx-auto text-ink-500 mb-2" />
                <p className="text-xs font-medium text-ink-200">GitHub account is not connected</p>
                <p className="text-[11px] text-ink-400 mt-1 max-w-md mx-auto">
                  Connect your GitHub Personal Access Token (PAT) in Settings to automatically list
                  and 1-click clone your private and public repositories.
                </p>
                <div className="mt-3">
                  <Button variant="secondary" size="xs" onClick={() => setActiveTab("custom")}>
                    Use Custom URL instead
                  </Button>
                </div>
              </div>
            ) : (
              <>
                {/* Search & Refresh Bar */}
                <div className="flex items-center gap-2">
                  <div className="relative flex-1">
                    <SearchIcon size={13} className="absolute left-2.5 top-2 text-ink-500" />
                    <input
                      type="text"
                      value={repoSearch}
                      onChange={(e) => setRepoSearch(e.target.value)}
                      placeholder="Search your repositories by name, language…"
                      className="w-full rounded border border-ink-800 bg-ink-950 pl-8 pr-3 py-1.5 text-xs text-ink-100 placeholder-ink-600 focus:border-accent focus:outline-none"
                    />
                  </div>
                  <div className="flex rounded border border-ink-800 bg-ink-950 p-0.5 text-[11px]">
                    <button
                      type="button"
                      onClick={() => setFilterVisibility("all")}
                      className={`px-2 py-0.5 rounded ${
                        filterVisibility === "all" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                      }`}
                    >
                      All
                    </button>
                    <button
                      type="button"
                      onClick={() => setFilterVisibility("public")}
                      className={`px-2 py-0.5 rounded ${
                        filterVisibility === "public" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                      }`}
                    >
                      Public
                    </button>
                    <button
                      type="button"
                      onClick={() => setFilterVisibility("private")}
                      className={`px-2 py-0.5 rounded ${
                        filterVisibility === "private" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                      }`}
                    >
                      Private
                    </button>
                  </div>
                  <button
                    type="button"
                    onClick={() => loadGithubRepos()}
                    disabled={loadingGithubRepos}
                    className="flex size-7 items-center justify-center rounded border border-ink-800 bg-ink-950 text-ink-400 hover:text-ink-200 transition-colors disabled:opacity-50"
                    title="Refresh repositories"
                  >
                    <RefreshCwIcon size={12} className={loadingGithubRepos ? "animate-spin" : ""} />
                  </button>
                </div>

                {/* Repositories Scrollable List */}
                <div className="max-h-56 min-h-36 overflow-y-auto rounded-lg border border-ink-800 bg-ink-950/80 divide-y divide-ink-850">
                  {loadingGithubRepos && githubRepos.length === 0 ? (
                    <div className="flex items-center justify-center py-8 text-xs text-ink-400 gap-2">
                      <RefreshCwIcon size={14} className="animate-spin" />
                      <span>Fetching your repositories from GitHub…</span>
                    </div>
                  ) : filteredRepos.length === 0 ? (
                    <div className="py-8 text-center text-xs text-ink-500">
                      {githubRepos.length === 0
                        ? "No repositories found for this account."
                        : "No repositories match your search filter."}
                    </div>
                  ) : (
                    filteredRepos.map((repo) => {
                      const isSelected = selectedRepo?.id === repo.id;
                      const langColor =
                        repo.language && LANGUAGE_COLORS[repo.language]
                          ? LANGUAGE_COLORS[repo.language]
                          : "bg-ink-500";

                      return (
                        <div
                          key={repo.id}
                          onClick={() => handleSelectRepo(repo)}
                          className={`flex items-start justify-between p-2.5 cursor-pointer transition-colors ${
                            isSelected
                              ? "bg-accent/10 border-l-2 border-accent"
                              : "hover:bg-ink-900/60"
                          }`}
                        >
                          <div className="min-w-0 flex-1 pr-2">
                            <div className="flex items-center gap-1.5">
                              {repo.isPrivate ? (
                                <LockIcon size={12} className="text-warn shrink-0" />
                              ) : (
                                <GithubIcon size={12} className="text-ink-400 shrink-0" />
                              )}
                              <span className="truncate text-xs font-medium text-ink-100">
                                {repo.fullName}
                              </span>
                              {repo.isPrivate ? (
                                <Chip tone="warn">Private</Chip>
                              ) : (
                                <Chip tone="neutral">Public</Chip>
                              )}
                            </div>
                            {repo.description && (
                              <p className="mt-0.5 truncate text-[11px] text-ink-400">
                                {repo.description}
                              </p>
                            )}
                            <div className="mt-1 flex items-center gap-3 text-[10px] text-ink-500">
                              {repo.language && (
                                <span className="flex items-center gap-1">
                                  <span className={`size-1.5 rounded-full ${langColor}`} />
                                  <span>{repo.language}</span>
                                </span>
                              )}
                              {repo.stars > 0 && (
                                <span className="flex items-center gap-0.5">
                                  <StarIcon size={10} className="text-amber-400" />
                                  <span>{repo.stars}</span>
                                </span>
                              )}
                              <span>Updated {formatRepoDate(repo.updatedAt)}</span>
                            </div>
                          </div>
                          <div className="shrink-0 pt-0.5">
                            <Button
                              variant={isSelected ? "primary" : "secondary"}
                              size="xs"
                              onClick={() => handleSelectRepo(repo)}
                            >
                              {isSelected ? "Selected" : "Select"}
                            </Button>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </>
            )}
          </div>
        )}

        {/* Tab 2: Custom URL Input */}
        {activeTab === "custom" && (
          <Field
            label="Repository URL"
            hint="Supports SSH (git@github.com:owner/repo.git) or HTTPS (https://github.com/owner/repo.git)"
          >
            <div className="relative">
              <input
                type="text"
                value={url}
                onChange={(e) => handleUrlChange(e.target.value)}
                placeholder="e.g. git@github.com:owner/repository.git"
                className="w-full rounded border border-ink-800 bg-ink-950 px-3 py-1.5 text-xs text-ink-100 font-mono focus:border-accent focus:outline-none"
              />
              {url.trim() && (
                <div className="absolute right-2 top-1.5">
                  <Chip tone={isSsh ? "purple" : isHttps ? "ok" : "neutral"}>
                    {isSsh ? "SSH" : isHttps ? "HTTPS" : "GIT"}
                  </Chip>
                </div>
              )}
            </div>
          </Field>
        )}

        {/* Selected Repo / URL Indicator */}
        {url.trim() && (
          <div className="rounded-lg border border-ink-850 bg-ink-900/40 p-2.5 text-xs flex items-center justify-between">
            <div className="flex items-center gap-2 truncate">
              {isSsh ? (
                <>
                  <KeyIcon size={14} className="text-accent shrink-0" />
                  <span className="text-ink-400">Clone via SSH:</span>
                </>
              ) : (
                <>
                  <GithubIcon size={14} className="text-ok shrink-0" />
                  <span className="text-ink-400">Clone via HTTPS:</span>
                </>
              )}
              <span className="mono text-[11px] text-ink-200 truncate">{url}</span>
            </div>
            <div className="shrink-0 pl-2">
              <span className="text-[10px] text-ink-400">
                {isSsh
                  ? "Local SSH Key"
                  : isGitHubConfigured
                    ? "Authenticated PAT"
                    : "Public Helper"}
              </span>
            </div>
          </div>
        )}

        {/* Destination folder */}
        <Field
          label="Destination Folder"
          hint="Parent folder on your machine where the repository directory will be cloned."
        >
          <div className="flex gap-2">
            <input
              type="text"
              value={destinationParentDir}
              onChange={(e) => setDestinationParentDir(e.target.value)}
              placeholder="/Users/username/Projects"
              className="flex-1 rounded border border-ink-800 bg-ink-950 px-3 py-1.5 text-xs text-ink-100 font-mono focus:border-accent focus:outline-none"
            />
            <Button variant="secondary" onClick={handleBrowse}>
              Browse…
            </Button>
          </div>
        </Field>

        {/* Directory name */}
        <Field label="Folder Name" hint="Directory name for the cloned project repository.">
          <input
            type="text"
            value={directoryName}
            onChange={(e) => setDirectoryName(e.target.value)}
            placeholder="repository-name"
            className="w-full rounded border border-ink-800 bg-ink-950 px-3 py-1.5 text-xs text-ink-100 font-mono focus:border-accent focus:outline-none"
          />
        </Field>

        {errorMsg && (
          <div className="rounded border border-red-500/30 bg-red-500/10 p-2.5 text-xs text-red-400 font-mono whitespace-pre-wrap">
            {errorMsg}
          </div>
        )}

        {/* Actions */}
        <div className="flex items-center justify-between pt-2 border-t border-ink-800/80">
          <Button variant="ghost" onClick={onClose} disabled={cloning}>
            Cancel
          </Button>
          <Button
            variant="primary"
            onClick={handleClone}
            disabled={cloning || !url.trim() || !destinationParentDir.trim()}
            className="flex items-center gap-1.5"
          >
            <DownloadCloudIcon size={14} />
            <span>{cloning ? "Cloning Repository…" : "Clone & Open"}</span>
          </Button>
        </div>
      </div>
    </Modal>
  );
}
