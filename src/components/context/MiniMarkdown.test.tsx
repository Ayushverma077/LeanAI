import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { MiniMarkdown } from "./MiniMarkdown";

describe("MiniMarkdown", () => {
  it("renders section Markdown as readable text, not raw symbols", () => {
    const { container } = render(
      <MiniMarkdown
        text={[
          "> A collaborative workspace built with",
          "> React and Express.",
          "",
          "- **Languages:** typescript 34 · go 3",
          "- `frontend/` — 47 files",
          "  - `app/` 35 · `lib/` 4",
          "",
          "_Variable names only; LeanAI never reads their values._",
        ].join("\n")}
      />,
    );

    expect(screen.getByText("Languages:").tagName).toBe("STRONG");
    expect(screen.getByText("frontend/").tagName).toBe("CODE");
    expect(container.querySelectorAll("li")).toHaveLength(3);
    expect(container.querySelector("blockquote")).toHaveTextContent(
      "A collaborative workspace built with React and Express.",
    );
    expect(screen.getByText(/never reads their values/).tagName).toBe("P");
    expect(container.textContent).not.toContain("**");
    expect(container.textContent).not.toContain("`");
    expect(container.textContent).not.toMatch(/^>/m);
  });

  it("never turns project text into HTML", () => {
    const { container } = render(<MiniMarkdown text={"- <img src=x onerror=alert(1)>"} />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toContain("<img src=x onerror=alert(1)>");
  });
});
