import { message } from "@tauri-apps/plugin-dialog";
import { showPanel } from "../ui/panels";
import type { Terminals } from "./terminals.svelte";

const TERMINAL_INPUT = ".terminal:not(.hidden) .xterm-helper-textarea";

/** Nuovo terminale in `cwd`, mostrato nella tab Console con il focus sul prompt. */
export async function openTerminal(terminals: Terminals, cwd: string | null, shell?: string) {
  try {
    await terminals.open(cwd, shell);
  } catch (error) {
    await message(String(error), { title: "Terminale non disponibile", kind: "error" });
    return;
  }
  showPanel("console", TERMINAL_INPUT);
}
