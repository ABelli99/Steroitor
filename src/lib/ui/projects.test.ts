import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/event", () => ({ listen: async () => () => {} }));
vi.mock("@tauri-apps/api/webviewWindow", () => ({ getCurrentWebviewWindow: () => ({ label: "main" }) }));
vi.mock("../backend", () => ({ projectWindows: async () => [] }));

const { folderInitial, folderName } = await import("./projects.svelte");

describe("folderInitial", () => {
  it("uses the first letter or digit of the folder name", () => {
    expect(folderInitial("C:\\Repos\\steroitor\\")).toBe("S");
    expect(folderInitial("/home/me/.config")).toBe("C");
    expect(folderInitial("C:\\Repos\\2026-notes")).toBe("2");
  });

  it("falls back to a question mark without a folder", () => {
    expect(folderInitial(null)).toBe("?");
    expect(folderName(null)).toBe("Nessuna cartella");
  });
});
