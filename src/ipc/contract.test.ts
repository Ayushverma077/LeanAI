import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { COMMANDS } from "./client";

/**
 * The frontend and backend agree on the command surface, or the build fails.
 *
 * Without this, renaming a Rust command silently produces a runtime "command
 * not found" in a packaged app, which is exactly the class of stringly-typed
 * IPC failure backlog 1.2 exists to prevent.
 */
describe("IPC contract", () => {
  const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
  const handlerBlock = lib.slice(
    lib.indexOf("tauri::generate_handler!["),
    lib.indexOf("])\n        .run("),
  );
  const rustCommands = [...handlerBlock.matchAll(/commands::\w+::(\w+)/g)].map(
    (match) => match[1] as string,
  );

  it("registers at least one command", () => {
    expect(rustCommands.length).toBeGreaterThan(20);
  });

  it("exposes every registered Rust command to the frontend", () => {
    const missing = rustCommands.filter((command) => !COMMANDS.includes(command as never));
    expect(missing).toEqual([]);
  });

  it("does not declare a command the backend has not registered", () => {
    const extra = COMMANDS.filter((command) => !rustCommands.includes(command));
    expect(extra).toEqual([]);
  });
});
