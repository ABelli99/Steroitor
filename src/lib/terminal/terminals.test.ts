import { beforeEach, describe, expect, it, vi } from "vitest";

const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
let nextId = 0;

vi.mock("@tauri-apps/api/core", () => ({
  invoke: async (command: string, args: Record<string, unknown>) => {
    calls.push({ command, args });
    return command === "terminal_open" ? { id: ++nextId, shell: "PowerShell" } : undefined;
  },
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));

const { terminals } = await import("./terminals.svelte");

describe("terminals.relocate", () => {
  beforeEach(() => {
    for (const session of [...terminals.sessions]) terminals.close(session.id);
    calls.length = 0;
  });

  it("closes every terminal and reopens a single one in the new folder", async () => {
    await terminals.open("C:\\vecchio");
    await terminals.open("C:\\vecchio");
    calls.length = 0;

    await terminals.relocate("C:\\nuovo");

    expect(calls.filter((call) => call.command === "terminal_close")).toHaveLength(2);
    expect(calls.filter((call) => call.command === "terminal_open").map((call) => call.args.cwd)).toEqual(["C:\\nuovo"]);
    expect(terminals.sessions.map((session) => session.title)).toEqual(["PowerShell"]);
  });

  it("does nothing when no terminal is open", async () => {
    await terminals.relocate("C:\\nuovo");
    expect(calls).toEqual([]);
  });

  it("keeps the Git command log visible if it was showing", async () => {
    await terminals.open("C:\\vecchio");
    terminals.activeId = null;
    await terminals.relocate("C:\\nuovo");
    expect(terminals.activeId).toBeNull();
    expect(terminals.sessions).toHaveLength(1);
  });
});

describe("terminals.open", () => {
  beforeEach(() => {
    for (const session of [...terminals.sessions]) terminals.close(session.id);
  });

  it("numbers terminals of the same shell, reusing freed titles", async () => {
    await terminals.open(null);
    const second = await terminals.open(null);
    await terminals.open(null);
    terminals.close(second);
    await terminals.open(null);
    expect(terminals.sessions.map((session) => session.title)).toEqual(["PowerShell", "PowerShell (3)", "PowerShell (2)"]);
  });
});
