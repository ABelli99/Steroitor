import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { projectWindows, type ProjectWindow } from "../backend";

/** Finestre Steroitor aperte nello stesso processo, sincronizzate dal backend. */
class Projects {
  windows = $state.raw<ProjectWindow[]>([]);
  readonly current = getCurrentWebviewWindow().label;

  async track() {
    const unlisten = await listen<ProjectWindow[]>("projects-changed", (event) => (this.windows = event.payload));
    this.windows = await projectWindows();
    return unlisten;
  }
}

export const projects = new Projects();

export function folderName(folder: string | null) {
  return folder?.split(/[\\/]/).filter(Boolean).at(-1) ?? "Nessuna cartella";
}

export function folderInitial(folder: string | null) {
  return folder ? (folderName(folder).match(/\p{L}|\p{N}/u)?.[0] ?? "?").toUpperCase() : "?";
}
