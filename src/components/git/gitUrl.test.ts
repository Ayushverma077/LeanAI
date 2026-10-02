import { describe, expect, it } from "vitest";

import { isGitHubHttpsUrl } from "./gitUrl";

// Same cases as `only_https_github_com_may_receive_the_token` in git.rs, so the
// label and the backend cannot drift apart unnoticed.
describe("isGitHubHttpsUrl", () => {
  it.each([
    "https://github.com/calcom/cal.diy",
    "https://github.com/owner/repo.git",
    "https://GitHub.com/owner/repo",
    "https://user@github.com/owner/repo",
    "https://github.com:443/owner/repo",
  ])("accepts %s", (url) => {
    expect(isGitHubHttpsUrl(url)).toBe(true);
  });

  it.each([
    "http://github.com/owner/repo",
    "git@github.com:owner/repo.git",
    "https://gitlab.com/owner/repo",
    "https://bitbucket.org/owner/repo",
    "https://github.com.evil.example/owner/repo",
    "https://evil.example/github.com/owner/repo",
    "https://evil.example?next=github.com",
    "https://github.com@evil.example/owner/repo",
    "https://api.github.com/repos/owner/repo",
    "",
  ])("refuses %s", (url) => {
    expect(isGitHubHttpsUrl(url)).toBe(false);
  });
});
