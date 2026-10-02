/**
 * Whether a clone URL may receive the stored GitHub token.
 *
 * Mirrors `is_github_https_url` in `src-tauri/src/commands/git.rs`, which is
 * the check that actually decides. This copy only keeps the modal's label
 * honest about what will be sent. Both are tested against the same cases.
 */
export function isGitHubHttpsUrl(value: string): boolean {
  const trimmed = value.trim();
  if (!trimmed.startsWith("https://")) return false;
  const authority = trimmed.slice("https://".length).split(/[/?#]/)[0] ?? "";
  const hostAndPort = authority.slice(authority.lastIndexOf("@") + 1);
  const host = hostAndPort.split(":")[0] ?? "";
  return host.toLowerCase() === "github.com";
}
