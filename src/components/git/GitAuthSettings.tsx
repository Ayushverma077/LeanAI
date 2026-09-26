import { useMemo, useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

import { useAppStore } from "../../store/useAppStore";
import { Button, Chip, Field, Panel } from "../primitives";
import {
  CheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  GithubIcon,
  KeyIcon,
  LockIcon,
  RefreshCwIcon,
  SearchIcon,
  StarIcon,
  TerminalIcon,
} from "../icons";
import { api, toAppError } from "../../ipc/client";
import type { GitHubRepository, SshAuthTestResult } from "../../ipc/types";
import { CloneRepoModal } from "./CloneRepoModal";

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

export function GitAuthSettings() {
  const store = useAppStore();
  const { gitAuth, loadGitAuth, githubRepos, loadingGithubRepos, loadGithubRepos } = store;

  const [tokenInput, setTokenInput] = useState("");
  const [accountLabel, setAccountLabel] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [savingToken, setSavingToken] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [testingSsh, setTestingSsh] = useState(false);
  const [sshResult, setSshResult] = useState<SshAuthTestResult | null>(null);
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const [showUpdateForm, setShowUpdateForm] = useState(false);

  const [repoSearch, setRepoSearch] = useState("");
  const [visibilityFilter, setVisibilityFilter] = useState<"all" | "public" | "private">("all");
  const [selectedRepoForClone, setSelectedRepoForClone] = useState<GitHubRepository | null>(null);
  const [showCloneModal, setShowCloneModal] = useState(false);

  const handleConnectToken = async () => {
    if (!tokenInput.trim()) return;
    setSavingToken(true);
    try {
      await api.configureGithubToken(tokenInput.trim(), accountLabel.trim() || undefined);
      await loadGitAuth();
      setTokenInput("");
      setAccountLabel("");
      setShowUpdateForm(false);
      store.setNotice("GitHub account connected and token saved in OS Keychain.");
    } catch (e) {
      store.setError(toAppError(e));
    } finally {
      setSavingToken(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    try {
      await api.disconnectGithub();
      await loadGitAuth();
      store.setNotice("GitHub credentials removed from OS Keychain.");
    } catch (e) {
      store.setError(toAppError(e));
    } finally {
      setDisconnecting(false);
    }
  };

  const handleTestSsh = async () => {
    setTestingSsh(true);
    setSshResult(null);
    try {
      const result = await api.testGithubSsh();
      setSshResult(result);
    } catch (e) {
      store.setError(toAppError(e));
    } finally {
      setTestingSsh(false);
    }
  };

  const handleCopyKey = async (publicKey: string, filename: string) => {
    try {
      await writeText(publicKey);
    } catch {
      await navigator.clipboard?.writeText(publicKey);
    }
    setCopiedKey(filename);
    setTimeout(() => setCopiedKey(null), 2500);
  };

  const user = gitAuth?.githubUser;
  const isConfigured = gitAuth?.githubTokenConfigured;

  const filteredRepos = useMemo(() => {
    return githubRepos.filter((repo) => {
      if (visibilityFilter === "public" && repo.isPrivate) return false;
      if (visibilityFilter === "private" && !repo.isPrivate) return false;
      if (!repoSearch.trim()) return true;
      const q = repoSearch.toLowerCase().trim();
      return (
        repo.name.toLowerCase().includes(q) ||
        repo.fullName.toLowerCase().includes(q) ||
        (repo.description && repo.description.toLowerCase().includes(q)) ||
        (repo.language && repo.language.toLowerCase().includes(q))
      );
    });
  }, [githubRepos, visibilityFilter, repoSearch]);

  return (
    <>
      {/* GitHub Account & PAT */}
      <Panel
        title="GitHub Integration"
        description="Connect with GitHub via Personal Access Token (PAT) stored strictly in OS Keychain (ADR 0007)."
        actions={
          isConfigured ? (
            <div className="flex items-center gap-2">
              <Button variant="ghost" size="sm" onClick={() => setShowUpdateForm(!showUpdateForm)}>
                {showUpdateForm ? "Cancel" : "Update Token"}
              </Button>
              <Button
                variant="danger"
                size="sm"
                onClick={handleDisconnect}
                disabled={disconnecting}
              >
                {disconnecting ? "Disconnecting…" : "Disconnect"}
              </Button>
            </div>
          ) : undefined
        }
      >
        {isConfigured && !showUpdateForm ? (
          <div className="space-y-3">
            <div className="flex items-center gap-3 rounded-lg border border-ink-800 bg-ink-950 p-3">
              {user?.avatarUrl ? (
                <img
                  src={user.avatarUrl}
                  alt={user.login}
                  className="size-10 rounded-full border border-ink-750"
                />
              ) : (
                <div className="flex size-10 items-center justify-center rounded-full bg-ink-900 border border-ink-750 text-ink-300">
                  <GithubIcon size={20} />
                </div>
              )}
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-xs font-semibold text-ink-100">
                    {user?.name ?? user?.login ?? "GitHub User"}
                  </span>
                  {user?.login && (
                    <span className="mono text-[11px] text-ink-500">@{user.login}</span>
                  )}
                  <Chip tone="ok" dot>
                    Connected in Keychain
                  </Chip>
                </div>
                {user?.htmlUrl && (
                  <a
                    href={user.htmlUrl}
                    target="_blank"
                    rel="noreferrer"
                    className="text-[11px] text-accent hover:underline"
                  >
                    {user.htmlUrl}
                  </a>
                )}
              </div>
            </div>

            {user?.scopes && user.scopes.length > 0 && (
              <div className="space-y-1">
                <div className="text-[11px] font-medium text-ink-400">Token Permissions:</div>
                <div className="flex flex-wrap gap-1">
                  {user.scopes.map((scope) => (
                    <Chip key={scope}>{scope}</Chip>
                  ))}
                </div>
              </div>
            )}

            <p className="text-[11px] text-ink-500">
              Your token is securely stored in your platform keychain and is used for authenticated
              HTTPS clone and push operations.
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            <p className="text-xs text-ink-400">
              Provide a GitHub Personal Access Token (PAT) with{" "}
              <span className="mono text-ink-200">repo</span> scope to authenticate Git operations
              over HTTPS.
            </p>

            <Field label="Personal Access Token" hint="Starts with ghp_ or github_pat_">
              <div className="relative">
                <input
                  type={showToken ? "text" : "password"}
                  value={tokenInput}
                  onChange={(e) => setTokenInput(e.target.value)}
                  placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
                  className="mono w-full rounded border border-ink-800 bg-ink-950 px-3 py-1.5 pr-14 text-xs text-ink-200 focus:border-accent focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() => setShowToken(!showToken)}
                  className="absolute right-2 top-1.5 text-[10px] text-ink-500 hover:text-ink-300"
                >
                  {showToken ? "Hide" : "Show"}
                </button>
              </div>
            </Field>

            <Field label="Account Label (Optional)" hint="e.g. your GitHub username or work email">
              <input
                type="text"
                value={accountLabel}
                onChange={(e) => setAccountLabel(e.target.value)}
                placeholder="e.g. octocat"
                className="w-full rounded border border-ink-800 bg-ink-950 px-3 py-1.5 text-xs text-ink-200 focus:border-accent focus:outline-none"
              />
            </Field>

            <div className="flex items-center justify-between pt-1">
              <a
                href="https://github.com/settings/tokens/new?scopes=repo&description=LeanAI+Desktop"
                target="_blank"
                rel="noreferrer"
                className="text-[11px] text-accent hover:underline"
              >
                Create token on GitHub →
              </a>
              <Button
                variant="primary"
                onClick={handleConnectToken}
                disabled={savingToken || !tokenInput.trim()}
              >
                {savingToken ? "Validating & Saving…" : "Connect GitHub"}
              </Button>
            </div>
          </div>
        )}
      </Panel>

      {/* GitHub Repositories */}
      {isConfigured && (
        <Panel
          title="Your GitHub Repositories"
          description="Browse and clone repositories from your connected GitHub account."
          badge={
            <Chip tone="neutral">
              {githubRepos.length} {githubRepos.length === 1 ? "repository" : "repositories"}
            </Chip>
          }
          actions={
            <Button
              variant="secondary"
              size="sm"
              onClick={() => loadGithubRepos()}
              disabled={loadingGithubRepos}
              className="flex items-center gap-1.5"
            >
              <RefreshCwIcon size={12} className={loadingGithubRepos ? "animate-spin" : ""} />
              <span>{loadingGithubRepos ? "Refreshing…" : "Refresh"}</span>
            </Button>
          }
        >
          <div className="space-y-3">
            {/* Search & Filter Bar */}
            <div className="flex items-center gap-2">
              <div className="relative flex-1">
                <SearchIcon size={13} className="absolute left-2.5 top-2 text-ink-500" />
                <input
                  type="text"
                  value={repoSearch}
                  onChange={(e) => setRepoSearch(e.target.value)}
                  placeholder="Filter repositories by name, language…"
                  className="w-full rounded border border-ink-800 bg-ink-950 pl-8 pr-3 py-1.5 text-xs text-ink-100 placeholder-ink-600 focus:border-accent focus:outline-none"
                />
              </div>
              <div className="flex rounded border border-ink-800 bg-ink-950 p-0.5 text-[11px]">
                <button
                  type="button"
                  onClick={() => setVisibilityFilter("all")}
                  className={`px-2 py-0.5 rounded ${
                    visibilityFilter === "all" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                  }`}
                >
                  All
                </button>
                <button
                  type="button"
                  onClick={() => setVisibilityFilter("public")}
                  className={`px-2 py-0.5 rounded ${
                    visibilityFilter === "public" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                  }`}
                >
                  Public
                </button>
                <button
                  type="button"
                  onClick={() => setVisibilityFilter("private")}
                  className={`px-2 py-0.5 rounded ${
                    visibilityFilter === "private" ? "bg-ink-800 text-ink-100" : "text-ink-400"
                  }`}
                >
                  Private
                </button>
              </div>
            </div>

            {/* List */}
            <div className="max-h-80 overflow-y-auto rounded-lg border border-ink-800 bg-ink-950 divide-y divide-ink-850">
              {loadingGithubRepos && githubRepos.length === 0 ? (
                <div className="flex items-center justify-center py-8 text-xs text-ink-400 gap-2">
                  <RefreshCwIcon size={14} className="animate-spin" />
                  <span>Loading repositories from GitHub…</span>
                </div>
              ) : filteredRepos.length === 0 ? (
                <div className="py-8 text-center text-xs text-ink-500">
                  {githubRepos.length === 0
                    ? "No repositories found for this account. Click 'Refresh' to fetch."
                    : "No repositories match your filter criteria."}
                </div>
              ) : (
                filteredRepos.map((repo) => {
                  const langColor =
                    repo.language && LANGUAGE_COLORS[repo.language]
                      ? LANGUAGE_COLORS[repo.language]
                      : "bg-ink-500";

                  return (
                    <div
                      key={repo.id}
                      className="flex items-center justify-between p-3 transition-colors hover:bg-ink-900/50"
                    >
                      <div className="min-w-0 flex-1 pr-3">
                        <div className="flex items-center gap-2">
                          {repo.isPrivate ? (
                            <LockIcon size={13} className="text-warn shrink-0" />
                          ) : (
                            <GithubIcon size={13} className="text-ink-400 shrink-0" />
                          )}
                          <span className="truncate text-xs font-semibold text-ink-100">
                            {repo.fullName}
                          </span>
                          {repo.isPrivate ? (
                            <Chip tone="warn">Private</Chip>
                          ) : (
                            <Chip tone="neutral">Public</Chip>
                          )}
                          {repo.isFork && <Chip tone="neutral">Fork</Chip>}
                        </div>

                        {repo.description && (
                          <p className="mt-1 truncate text-[11px] text-ink-400">
                            {repo.description}
                          </p>
                        )}

                        <div className="mt-1.5 flex items-center gap-3 text-[10px] text-ink-500">
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
                          <span>Branch: {repo.defaultBranch}</span>
                          <span>Updated {formatRepoDate(repo.updatedAt)}</span>
                        </div>
                      </div>

                      <div className="flex shrink-0 items-center gap-2">
                        {repo.htmlUrl && (
                          <a
                            href={repo.htmlUrl}
                            target="_blank"
                            rel="noreferrer"
                            className="flex size-7 items-center justify-center rounded text-ink-400 hover:bg-ink-800 hover:text-ink-200 transition-colors"
                            title="Open repository in browser"
                          >
                            <ExternalLinkIcon size={13} />
                          </a>
                        )}
                        <Button
                          variant="secondary"
                          size="xs"
                          onClick={() => {
                            setSelectedRepoForClone(repo);
                            setShowCloneModal(true);
                          }}
                        >
                          Clone & Open
                        </Button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </div>
        </Panel>
      )}

      {/* SSH Keys & Agent */}
      <Panel
        title="SSH Keys & Agent"
        description="Inspect local SSH identities for cloning and pushing repositories via SSH (git@github.com)."
        actions={
          <Button
            variant="ghost"
            size="sm"
            onClick={handleTestSsh}
            disabled={testingSsh}
            className="flex items-center gap-1.5"
          >
            <RefreshCwIcon size={12} className={testingSsh ? "animate-spin" : ""} />
            <span>{testingSsh ? "Testing…" : "Test GitHub SSH"}</span>
          </Button>
        }
      >
        <div className="space-y-3">
          {/* SSH Agent Status */}
          <div className="flex items-center justify-between rounded-lg border border-ink-850 bg-ink-950 p-2.5 text-xs">
            <div className="flex items-center gap-2">
              <TerminalIcon size={14} className="text-ink-400" />
              <span className="text-ink-300">SSH Agent:</span>
              <span className="text-ink-100 font-medium">
                {gitAuth?.sshAgentActive ? "Running" : "Inactive"}
              </span>
            </div>
            <Chip tone={gitAuth?.sshAgentActive ? "ok" : "neutral"} dot>
              {gitAuth?.sshAgentActive ? "Active" : "Not Detected"}
            </Chip>
          </div>

          {/* SSH Test Result */}
          {sshResult && (
            <div
              className={`rounded-lg border p-3 text-xs ${
                sshResult.authenticated
                  ? "border-ok/30 bg-ok/10 text-ok-light"
                  : "border-warn/30 bg-warn/10 text-warn-light"
              }`}
            >
              <div className="flex items-center gap-2 font-medium">
                {sshResult.authenticated ? (
                  <>
                    <CheckIcon size={14} />
                    <span>
                      SSH Authenticated with GitHub as{" "}
                      <span className="font-semibold text-ink-100">
                        @{sshResult.username ?? "user"}
                      </span>
                    </span>
                  </>
                ) : (
                  <span>GitHub SSH Connection Notice</span>
                )}
              </div>
              <p className="mt-1 text-[11px] opacity-90">{sshResult.output}</p>
            </div>
          )}

          {/* Discovered SSH Keys */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-[11px] text-ink-400">
              <span className="font-medium">Detected Public Keys in ~/.ssh:</span>
              <span>{gitAuth?.sshKeys?.length ?? 0} found</span>
            </div>

            {gitAuth?.sshKeys && gitAuth.sshKeys.length > 0 ? (
              <div className="space-y-2">
                {gitAuth.sshKeys.map((key) => (
                  <div
                    key={key.filename}
                    className="flex items-center justify-between gap-2 rounded border border-ink-850 bg-ink-950 p-2 text-xs"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <KeyIcon size={12} className="text-accent shrink-0" />
                        <span className="mono font-semibold text-ink-200">{key.filename}</span>
                        <Chip>{key.keyType}</Chip>
                      </div>
                      <p className="mono mt-0.5 truncate text-[10px] text-ink-500">
                        {key.comment || key.publicKey.slice(0, 32) + "…"}
                      </p>
                    </div>

                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => handleCopyKey(key.publicKey, key.filename)}
                      className="shrink-0 flex items-center gap-1"
                    >
                      {copiedKey === key.filename ? (
                        <>
                          <CheckIcon size={12} className="text-ok" />
                          <span className="text-ok">Copied</span>
                        </>
                      ) : (
                        <>
                          <CopyIcon size={12} />
                          <span>Copy Key</span>
                        </>
                      )}
                    </Button>
                  </div>
                ))}
              </div>
            ) : (
              <div className="rounded border border-dashed border-ink-850 p-3 text-center text-xs text-ink-500">
                No SSH public keys found in <span className="mono">~/.ssh/</span>. Generate one
                with:
                <div className="mono mt-1 text-[11px] text-ink-300">
                  ssh-keygen -t ed25519 -C &quot;your_email@example.com&quot;
                </div>
              </div>
            )}
          </div>

          <div className="flex items-center justify-between pt-1 text-[11px]">
            <a
              href="https://github.com/settings/keys"
              target="_blank"
              rel="noreferrer"
              className="text-accent hover:underline"
            >
              Manage SSH Keys on GitHub →
            </a>
            <span className="text-ink-500">Copy your public key above and add it to GitHub</span>
          </div>
        </div>
      </Panel>

      <CloneRepoModal
        open={showCloneModal}
        initialRepo={selectedRepoForClone}
        onClose={() => {
          setShowCloneModal(false);
          setSelectedRepoForClone(null);
        }}
      />
    </>
  );
}
