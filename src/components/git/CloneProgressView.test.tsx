import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CloneProgressView } from "./CloneProgressView";

describe("CloneProgressView", () => {
  it("shows an indeterminate state before git reports anything", () => {
    render(<CloneProgressView progress={null} />);
    expect(screen.getByText("Connecting…")).toBeInTheDocument();
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
  });

  it("shows the phase, percentage, amount received and speed", () => {
    render(
      <CloneProgressView
        progress={{
          phase: "Receiving objects",
          percent: 23,
          current: 2345,
          total: 10234,
          transferred: "45.67 MiB",
          speed: "2.31 MiB/s",
        }}
      />,
    );
    expect(screen.getByText("Receiving objects")).toBeInTheDocument();
    expect(screen.getByText("23%")).toBeInTheDocument();
    expect(screen.getByText("2,345 / 10,234 · 45.67 MiB · 2.31 MiB/s")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Receiving objects" })).toHaveAttribute(
      "aria-valuenow",
      "23",
    );
  });

  it("leaves out numbers git did not report", () => {
    render(
      <CloneProgressView
        progress={{
          phase: "Resolving deltas",
          percent: 60,
          current: 6,
          total: 10,
          transferred: null,
          speed: null,
        }}
      />,
    );
    expect(screen.getByText("6 / 10")).toBeInTheDocument();
  });
});
